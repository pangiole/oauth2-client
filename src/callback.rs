//! It spawns an embedded HTTP server on an ephemeral port with the sole intent
//! to provide the `/callback` endpoint at which our OAuth2 Client can "grab" the auth code.

use anyhow::{Result, anyhow};
use axum::{
    extract::{State, Query},
    response::Html,
    Router,
    routing::get,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot::{Sender, Receiver};

use std::sync::Arc;
use std::sync::Mutex;
use openidconnect::CsrfToken;
use serde::Deserialize;
use tokio::sync::oneshot;
// TODO use tokio::sync::Mutex;
use tracing::{info, warn};


// Holds shared thread-safe resources for all Axum route handlers.
#[derive(Clone)]
struct AppState {
    csrf_token: String,
    // Carries the authorization code (Ok) or the failure reason (Err) back to the caller.
    oauth_code_tx: Arc<Mutex<Option<Sender<Result<String>>>>>,
    shutdown_signal_tx: Arc<Mutex<Option<Sender<()>>>>
}

#[derive(Deserialize)]
struct OAuthCallbackParams {
    // The `state` parameter holds the CSRF token we generated and sent with the
    // authorization request; the Authorization Server echoes it back to us.
    state: String,
    // The authorization code is only present on a successful authorization.
    code: Option<String>,
    // On failure (e.g. the user denied consent) the Authorization Server returns
    // an `error` (usually with an `error_description`) instead of a code.
    error: Option<String>,
}


// Send the outcome (authorization code or failure reason) down the channel to
// unblock the caller, then always trigger the graceful shutdown of the ephemeral
// server. This ensures the server stops on *every* terminal outcome, including
// failures such as the user denying consent or a CSRF state mismatch.
// Returns whether the outcome was sent (i.e. the caller was still waiting).
fn complete(
    app_state: AppState,
    outcome: Result<String>
) -> bool {

    let sent = {
        let mut oauth_code_tx_guard = app_state.oauth_code_tx.lock().unwrap();
        if let Some(oauth_code_tx) = oauth_code_tx_guard.take() {
            oauth_code_tx.send(outcome).is_ok()
        }
        else { false }
    };

    let mut shutdown_signal_tx_guard = app_state.shutdown_signal_tx.lock().unwrap();
    if let Some(shutdown_signal_tx) = shutdown_signal_tx_guard.take() {
        let _ = shutdown_signal_tx.send(());
    }

    sent
}


// Handle the incoming HTTP GET requests hitting the '/callback' endpoint
async fn handle_callback(
    Query(params): Query<OAuthCallbackParams>,
    State(app_state): State<AppState>
) -> Html<String> {

    info!("Handling callback");

    // CSRF Guard: Ensure OAuth state returned matches the locally generated CSRF token
    if params.state != app_state.csrf_token {
        let _ = complete(app_state, Err(anyhow!("OAuth state mismatch")));
        return Html("<h1>OAuth State Mismatch Failure! Request Rejected.</h1>".to_string());
    }

    // The Authorization Server may redirect back with an `error` instead of a code
    // (e.g. the user denied consent), in which case there is no code to exchange.
    let Some(oauth_code) = params.code else {
        let error = params.error.as_deref().unwrap_or("unknown_error");
        let message = format!("Authorization request failed: {}", error);
        warn!("{}", message);
        let _ = complete(app_state, Err(anyhow!("{}", message)));
        return Html(format!("<h1>{}</h1>", message));
    };

    let sent = complete(app_state, Ok(oauth_code));
    if sent {
        Html("<h1>Authentication Successful! Return to terminal.</h1>".to_string())
    }
    else {
        Html("<h1>Callback already processed!</h1>".to_string())
    }
}


// The axum server keeps listening until the graceful-shutdown signal is sent
// from `handle_callback` once the OAuth code has been received.
async fn axum_serve(
    tcp_listener: TcpListener,
    app_router: Router<()>,
    shutdown_signal: Receiver<()>
) {

    let result = axum::serve(tcp_listener, app_router)
        .with_graceful_shutdown(async {
            let _ = shutdown_signal.await;
        })
        .await;

    if let Err(err) = result {
        warn!("Callback endpoint error: {}", err);
    }
}


// TODO Show how to use tokio::time::timeout on the oneshot::Receiver so the CLI app cancels gracefully if the user abandons login.

pub async fn spawn_endpoint(
    csrf_token: &CsrfToken
) -> Result<(String, Receiver<Result<String>>)> {

    // Bind a server socket on a port assigned by the underlying operating system
    let local_host = "127.0.0.1";
    let tcp_listener = TcpListener::bind(format!("{}:0", local_host)).await?;
    let local_port = tcp_listener.local_addr()?.port();
    let callback_url = format!("http://{}:{}/callback", local_host, local_port);

    info!("Spawning callback endpoint at {}", callback_url);

    // Set up a Tokio channel for OAuth code transmission (Ok = code, Err = failure)
    let (oauth_code_tx, oauth_code_rx) = oneshot::channel::<Result<String>>();

    // Set up another Tokio channel for the shutdown signal
    let (shutdown_signal_tx, shutdown_signal_rx) = oneshot::channel::<()>();

    // Assemble the application state shared between all Axum routes
    let app_state = AppState {
        csrf_token: csrf_token.secret().clone(),
        oauth_code_tx: Arc::new(Mutex::new(Some(oauth_code_tx))),
        shutdown_signal_tx: Arc::new(Mutex::new(Some(shutdown_signal_tx))),
    };

    let app_router = Router::new()
        // When an HTTP GET request hits the '/callback' endpoint, handle it by grabbing the auth code
        .route("/callback", get(handle_callback))
        .with_state(app_state) // Hand a copy of state to all routes
        ;

    // Run the server in the background so the caller can continue immediately
    tokio::spawn(axum_serve(tcp_listener, app_router, shutdown_signal_rx));

    Ok((callback_url, oauth_code_rx))
}
