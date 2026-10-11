# Channels
The single most important architectural detail of the Authorization Code flow is
the *channel* a message travels over, and it dictates what may be carried on each
leg.

## Front-channel
The first six steps all travel *through the user's browser*, as plain HTTP 
redirects. The browser is a passive intermediary: it forwards the authorization
request to the OAuth Server, and later forwards the authorization response 
(carrying the `code`) back to our `/callback` endpoint. Everything on this 
channel is visible in the browser's address bar and history, so only *public* 
parameters (such as `response_type`, `client_id`, `redirect_uri`, 
`scope=openid`, `state`, `nonce`, and later the `code` itself) may ever appear
here — never a secret.

```rust
use anyhow::Result;
use openidconnect::{AuthorizationCode, RedirectUrl};
use openidconnect::core::CoreAuthenticationFlow;
use tokio::sync::oneshot::Receiver;

// type DiscoveredOidcClient = ...
// type CallbackOutcome = ...
// type FrontChannelOutcome = ...

async fn run_authorization_code_flow_front_channel(
    oidc_client: &mut DiscoveredOidcClient
) -> Result<FrontChannelOutcome> {

    // Spawn the ephemeral `/callback` endpoint that will receive the `code`
    let (callback_url, callback_rx): (String, Receiver<Result<CallbackOutcome>>) = 
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
    
    // Generate the authorize URL with all parameters
    let (authorize_url, _, _) = oidc_client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
        )
        // The `openid` scope is added automatically by the underlying library
        // .add_scopes(...)
        .url();

    info!("Opening the browser to the authorize URL {}", authorize_url);
    // TODO webbrowser::open(authorize_url.as_str())?;

    // Await the callback outcome (mainly the auth code).
    // Note that the outer `?` handles an eventually dropped one-shot channel (RecvError); 
    // while the inner `?` propagates the authorization failure reported by the callback 
    // endpoint (e.g. the user denied consent).
    let code = callback_rx.await??;
    info!("OAuth code is: {}", code);

    Ok(FrontChannelOutcome {
        code: AuthorizationCode::new(code),
    })
}
```

## Back-channel
Steps 7 through 10 go *directly* between the Client App and the OAuth Server,
bypassing the browser entirely. The token exchange happens here. Because the 
channel is not exposed publicly, the client can safely present its 
`client_secret`, and neither the `access_token` nor the `id_token` ever transits
the user's browser.

Splitting the flow this way is what lets the client hand the user a short-lived
`code` through the front-channel, while turning that code into tokens only after
it has authenticated itself on the back-channel.
