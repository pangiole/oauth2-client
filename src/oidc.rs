use crate::callback::{self, CallbackOutcome};
use crate::config::{self, IssuerType};
use crate::store;
use anyhow::Result;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AccessToken, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, RedirectUrl, RefreshToken, Scope,
    TokenResponse, reqwest,
};
use tracing::info;


/// The concrete client type returned by `CoreClient::from_provider_metadata`.
///
/// `openidconnect` encodes which endpoints are available at the type level (the
/// "type-state" pattern). After discovery the authorization endpoint is set, the
/// token and userinfo endpoints are optional, and the remaining ones are unset.
/// Naming this concrete type lets both flows share a single discovered client.
/// The redirect URI is *not* part of the type state, so it is attached later,
/// when `run_authorization_code_flow_front_channel` learns the callback URL.
type DiscoveredOidcClient = CoreClient<
    EndpointSet,      // HasAuthUrl
    EndpointNotSet,   // HasDeviceAuthUrl
    EndpointNotSet,   // HasIntrospectionUrl
    EndpointNotSet,   // HasRevocationUrl
    EndpointMaybeSet, // HasTokenUrl
    EndpointMaybeSet, // HasUserInfoUrl
>;


/// Discover the OIDC provider metadata and build the client, so that both the
/// Refresh Token and the Authorization Code flows share the same HTTP client and
/// OAuth client instead of each constructing their own.
async fn discover(
) -> Result<(reqwest::Client, DiscoveredOidcClient)> {

    // Access the global configuration
    let cfg = config::get();
    info!("Discovering OIDC provider metadata for provider '{}'", cfg.issuer_type);

    let http_client = reqwest::Client::new();
    let issuer_url = IssuerUrl::new(cfg.issuer_url.clone())?;
    let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client).await?;

    // Since this app is an OAuth2 confidential client, we send the client secret
    // along with the client identifier produced by the Authorization Server.
    let oidc_client = CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(cfg.client_id.clone()),
        Some(ClientSecret::new(cfg.client_secret.clone())),
    );

    Ok((http_client, oidc_client))
}


/// Attempt the Refresh Token flow using a token persisted by a previous run.
///
/// Returns an error (so the caller can fall back to the Authorization Code flow)
/// when no refresh token is stored or when the Authorization Server rejects it.
async fn run_refresh_token_flow(
    http_client: &reqwest::Client,
    oidc_client: &DiscoveredOidcClient
) -> Result<AccessToken> {

    info!("Attempting the Refresh Token flow");

    let Some(refresh_token) = store::load_refresh_token().await? else {
        anyhow::bail!("no refresh token stored");
    };

    let token_response = oidc_client
        // `exchange_refresh_token` returns a `Result<RefreshTokenRequest, ConfigurationError>`
        // (the token endpoint URL may be missing), so unwrap it before requesting.
        .exchange_refresh_token(&RefreshToken::new(refresh_token))?
        .request_async(http_client)
        .await?;

    // Some providers rotate the refresh token on each use; persist the new one.
    if let Some(new_refresh_token) = token_response.refresh_token() {
        store::save_refresh_token(new_refresh_token.secret()).await?;
    }

    Ok(token_response.access_token().clone())
}


/// Everything the front-channel leg hands over to the back-channel leg.
///
/// `code` is the authorization code captured by our ephemeral `/callback` endpoint;
/// `nonce` is expected to be echoed back withing the `id_token` through the
/// back-channel leg.
///
struct FrontChannelOutcome {
    code: AuthorizationCode,
    nonce: Nonce,
}


/// Run the **front-channel** leg of the Authorization Code flow: hand the user
/// off to the OAuth Server *through the browser* and collect the authorization
/// `code` it redirects back to our ephemeral `/callback` endpoint.
///
/// The redirect URI is attached to `oidc_client` in place, so the caller's client
/// carries it afterwards (the back-channel leg then echoes it back to the OAuth
/// Server).
async fn run_authorization_code_flow_front_channel(
    oidc_client: &mut DiscoveredOidcClient
) -> Result<FrontChannelOutcome> {

    // Access the global configuration
    let cfg = config::get();

    // Spawn the ephemeral `/callback` endpoint that will receive the `code`
    let (callback_url, callback_rx) =
        callback::spawn_endpoint().await?;

    // Attach the callback URL as the `redirect_uri` the OAuth Server will send
    // the authorization `code` back to.
    //
    // `set_redirect_uri` consumes the client and returns the updated one, so clone
    // ours, set the URI, and assign the result back through the `&mut` reference.
    *oidc_client = oidc_client
        .clone()
        .set_redirect_uri(
            RedirectUrl::new(callback_url)?
        );

    // The "state" parameter is our CSRF token: an opaque value generated by our
    // Client Application that must be a cryptographically random sequence of
    // characters. The OAuth Server echoes it back unchanged on the redirect
    // location, along with the authorization "code" (see RFC 6749 Section 10.12).
    let state = CsrfToken::new_random();

    // TODO Explain why we use a nonce
    let nonce = Nonce::new_random();

    // Generate the authorize URL including the above state and nonce protections
    let mut request = oidc_client
        // TODO .set_pkce_challenge(pkce_challenge)
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            || state,
            || nonce
        )
        .add_scopes(cfg.scopes.iter().cloned().map(Scope::new));

    // The `openid` scope is added automatically by the underlying library
    // (see CoreClient::authorize_url); here we only append the scopes
    // configured by this client application.

    match cfg.issuer_type {
        // Google is the outlier: `access_type=offline` asks for a refresh token
        // and `prompt=consent` forces the consent screen so a refresh token is
        // (re)issued on every authorization. Google does not use the standard
        // `offline_access` scope.
        IssuerType::Google => {
            request = request
                .add_extra_param("access_type", "offline")
                .add_extra_param("prompt", "consent");
        }
        // All other supported providers request a refresh token via the standard
        // `offline_access` scope.
        _ => {
            request = request.add_scope(Scope::new("offline_access".to_string()));
        }
    }

    // Generate the authorize URL with all parameters
    let (authorize_url, state, nonce) = request.url();
    info!("Opening your default browser with the authorize URL {}", authorize_url);
    // TODO webbrowser::open(authorize_url.as_str())?;

    // Await the callback outcome (the authorization code plus the echoed `state`)
    // sent by the Authorization Server to our ephemeral `/callback` endpoint.
    //
    // Note that the outer `?` handles an eventually dropped one-shot channel (RecvError);
    // while the inner `?` propagates the authorization failure reported by the callback
    // endpoint (e.g. the user denied consent).
    let CallbackOutcome { code, state: echoed_state } = callback_rx.await??;
    info!("OAuth code is: {}", code);

    // CSRF guard: the OAuth Server must echo back the exact "state" value we
    // generated; a mismatch means this callback was not the response to our request
    // (e.g. an Account Linking attack), so we reject it before exchanging the code.
    if &echoed_state != state.secret() {
        anyhow::bail!("OAuth state mismatch: possible CSRF attack");
    }

    Ok(FrontChannelOutcome {
        code: AuthorizationCode::new(code),
        nonce,
    })
}


/// Run the **back-channel** leg of the Authorization Code flow: exchange the
/// authorization `code` for tokens by talking to the OAuth Server *directly*,
/// bypassing the browser. Going direct is what lets us present the
/// `client_secret` and keep the tokens out of the browser.
async fn run_authorization_code_flow_back_channel(
    http_client: &reqwest::Client,
    oidc_client: &DiscoveredOidcClient,
    code: AuthorizationCode,
    nonce: Nonce,
) -> Result<AccessToken> {

    // Exchange the authorization code with the access, id and refresh tokens
    let token_response =
        oidc_client
            // `exchange_code` returns a `Result<CodeTokenRequest, ConfigurationError>`
            // (the token endpoint URL may be missing), so unwrap it before requesting.
            .exchange_code(code)?
            // TODO Optionally .set_pkce_verifier(pkce_verifier)
            .request_async(http_client)
            .await?;


    // Validate the ID token
    if let Some(id_token) = token_response.id_token() {
        let verifier = oidc_client.id_token_verifier();
        let claims = id_token.claims(&verifier, &nonce)?;
        info!("ID token verified; subject = {}", claims.subject().as_str());
    }

    // Persist the refresh token so the next run can skip the browser round-trip.
    if let Some(refresh_token) = token_response.refresh_token() {
        store::save_refresh_token(refresh_token.secret()).await?;
    }

    Ok(token_response.access_token().clone())
}


///
/// Get an access token by attempting the following OAuth2 flows in order:
///
///  1. refresh token
///  2. authorization code
///
pub async fn get_access_token(
) -> Result<AccessToken> {
    info!("Getting an OAuth2 access token ...");

    let (http_client, mut oidc_client) = discover().await?;

    match run_refresh_token_flow(&http_client, &oidc_client).await {
        Ok(access_token) => {
            info!("Access token obtained via the Refresh Token flow");
            Ok(access_token)
        }
        Err(err) => {
            info!("Refresh Token flow unavailable ({}); falling back to the Authorization Code flow", err);
            // Front-channel: obtain the authorization `code` through the browser (steps 1-6).
            // It attaches the `/callback` redirect URI to `oidc_client` in place.
            let front = run_authorization_code_flow_front_channel(&mut oidc_client).await?;

            // Back-channel: exchange the code for tokens directly with the OAuth Server (steps 7-8).
            run_authorization_code_flow_back_channel(&http_client, &oidc_client, front.code, front.nonce).await
        }
    }
}


