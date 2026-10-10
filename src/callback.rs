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
use serde::Deserialize;
use tokio::sync::oneshot;
// TODO use tokio::sync::Mutex;
use tracing::{info, warn};


// The successful outcome of the `/callback` endpoint: the authorization `code`
// plus the `state` the Authorization Server echoed back. Handing both to the
// caller lets it verify the echoed `state` against the CSRF token it generated.
pub struct CallbackOutcome {
    pub code: String,
    pub state: String,
}

// Holds shared thread-safe resources for the Axum route handlers.
#[derive(Clone)]
struct CallbackState {
    // Carries the callback outcome (Ok) or the failure reason (Err) back to the caller.
    callback_tx: Arc<Mutex<Option<Sender<Result<CallbackOutcome>>>>>,
    shutdown_signal_tx: Arc<Mutex<Option<Sender<()>>>>
}

#[derive(Deserialize)]
struct CallbackParams {
    // The `state` parameter holds the CSRF token we generated and sent with the
    // authorization request; the Authorization Server echoes it back to us. We
    // forward it so the caller can verify it against the value it generated.
    state: String,
    // The authorization code is only present on a successful authorization.
    code: Option<String>,
    // On failure (e.g. the user denied consent) the Authorization Server returns
    // an `error` (usually with an `error_description`) instead of a code.
    error: Option<String>,
}


// Send the callback outcome (the code and echoed state) or the failure reason
// down the channel to unblock the caller, then always trigger the graceful
// shutdown of the ephemeral server. This ensures the server stops on *every*
// terminal outcome, including failures such as the user denying consent.
// Returns whether the outcome was sent (i.e. the caller was still waiting).
fn complete(app_state: CallbackState, outcome: Result<CallbackOutcome>) -> bool {
    let sent = {
        let mut callback_tx_guard = app_state.callback_tx.lock().unwrap();
        if let Some(callback_tx) = callback_tx_guard.take() {
            callback_tx.send(outcome).is_ok()
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
async fn handle_callback(Query(params): Query<CallbackParams>, State(app_state): State<CallbackState>) -> Html<String> {
    info!("Handling callback");

    // The Authorization Server may redirect back with an `error` instead of a code
    // (e.g. the user denied consent), in which case there is no code to exchange.
    let Some(code) = params.code else {
        let error = params.error.as_deref().unwrap_or("unknown_error");
        let message = format!("Authorization request failed: {}", error);
        warn!("{}", message);
        let _ = complete(app_state, Err(anyhow!("{}", message)));
        return Html(format!("<h1>{}</h1>", message));
    };

    // Forward both the authorization code and the echoed `state` to the caller;
    // verifying the state against the CSRF token is up to the caller.
    let sent = complete(app_state, Ok(CallbackOutcome { code, state: params.state }));
    if sent {
        Html("<h1>Authentication Successful! Return to terminal.</h1>".to_string())
    }
    else {
        Html("<h1>Callback already processed!</h1>".to_string())
    }
}


// The axum server keeps listening until the graceful-shutdown signal is sent
// from `handle_callback` once the OAuth code has been received.
async fn axum_serve(tcp_listener: TcpListener, app_router: Router<()>, shutdown_signal: Receiver<()>) {
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

pub async fn spawn_endpoint() -> Result<(String, Receiver<Result<CallbackOutcome>>)> {

    // Bind a server socket on a port assigned by the underlying operating system
    let local_host = "127.0.0.1";
    let tcp_listener = TcpListener::bind(format!("{}:0", local_host)).await?;
    let local_port = tcp_listener.local_addr()?.port();
    let callback_url = format!("http://{}:{}/callback", local_host, local_port);

    info!("Spawning callback endpoint at {}", callback_url);

    // Set up a Tokio channel for the callback outcome (Ok = code + echoed state, Err = failure)
    let (callback_tx, callback_rx) = oneshot::channel::<Result<CallbackOutcome>>();

    // Set up another Tokio channel for the shutdown signal
    let (shutdown_signal_tx, shutdown_signal_rx) = oneshot::channel::<()>();

    // Assemble the application state shared between all Axum routes
    let callback_state = CallbackState {
        callback_tx: Arc::new(Mutex::new(Some(callback_tx))),
        shutdown_signal_tx: Arc::new(Mutex::new(Some(shutdown_signal_tx))),
    };

    let app_router = Router::new()
        // When an HTTP GET request hits the '/callback' endpoint, handle it by grabbing the auth code
        .route("/callback", get(handle_callback))
        .with_state(callback_state) // Hand a copy of state
        ;

    // Run the server in the background so the caller can continue immediately
    tokio::spawn(axum_serve(tcp_listener, app_router, shutdown_signal_rx));

    Ok((callback_url, callback_rx))
}
