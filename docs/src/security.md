# Vulnerabilities
The Authorization Code flow is not secure by itself: the front-channel is a
series of plain browser navigations, and the callback is a public URL that
*anyone* can make a browser visit. Parameters such as `state` and `nonce` were
added to protect from the following attacks.

[//]: # (TODO Explain it as CRSF attack and then clarify the difference 
between Account Linking versus Account Takeover)

## Account Linking Attack
The account-linking attack exploits the fact that the callback request carrying
the `code` (step 6) has no built-in way to prove it belongs to the session that
*initiated* the flow. The attacker abuses this between steps 5 and 6:

1. The attacker starts an authorization request of their own and authenticates
   as *themselves*. The Authorization Server answers with a redirect carrying a
   `code` bound to the attacker's account, such as
   `http://localhost:67852/callback?code=<ATTACKER_CODE>`.
2. Instead of letting their own browser follow that redirect, the attacker
   captures it and tricks the *victim* into navigating to it — via a link, an
   image tag, or a page the victim visits.
3. The victim's Client App receives a perfectly valid `code` and exchanges it
   for tokens — but those tokens identify the *attacker*, not the victim.
4. The victim's session is now silently bound to the attacker's identity. If
   the victim then acts on the app — uploading data, or linking their own
   account — those actions land on the attacker's account, which the attacker
   can later sign in to and read.

The root cause is that a `code` is an opaque value the client cannot 
distinguish: it cannot tell whether the code came from a request it issued 
itself, or whether someone else injected it into the flow.

The defense is to make the callback *provable* as the response to the client's
own request. Before redirecting to the Authorization Server, the client
generates a random, unguessable `state`, keeps it, and sends it along on the
authorization request (step 1). The Authorization Server must echo it back
unchanged on the callback (steps 5–6), and the client rejects the callback
whenever the two values differ.

The attack now collapses: an attacker can no longer replay a `code` from a flow
*they* started, because that code comes back with the attacker's `state` — or
with none at all — and will never match the `state` the victim's client
generated. Only the client that started the flow knows its own `state`, so only
a legitimate round-trip survives the check.

This is exactly a **Cross-Site Request Forgery (CSRF)** defense (see
[RFC 6749 §10.12](https://www.rfc-editor.org/info/rfc6749/#section-10.12)):
the attacker forges a request — the callback navigation — that the victim's
browser executes in the victim's session, and `state` is the unpredictable
secret that must be echoed back to prove the request was genuinely initiated by
that session. That is why `state` is so often called the *CSRF token* — and in
our Rust implementation it literally is one:

## ID Token Replay Attack
...


The following Rust snippet demonstrates how to mitigate both the CRSF attack and
the ID Token Reply attack.

```rust
use openidconnect::{CsrfToken, Nonce};
use openidconnect::core::CoreAuthenticationFlow;

// type DiscoveredOidcClient = ...

async fn run_authorization_code_flow(oidc_client: DiscoveredOidcClient) -> Result<AccessToken> {
    
    // Spawn the ephemeral `/callback` endpoint
    let (callback_url, callback_rx) = callback::spawn_endpoint().await?;

    // Set the callback URL 
    let oidc_client = oidc_client
        .set_redirect_uri(
            RedirectUrl::new(callback_url)?
        );

    // Generate the unguessable state and nonce
    let state = CsrfToken::new_random();
    let nonce = Nonce::new_random();
    
    // And then generate the authorize URL including them
    let (authorize_url, _, _) = oidc_client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            || state,
            || nonce
        )
        .url();
    
    // Open the browser with the "authorize URL"
    // ...

    // Await the callback outcome so to receive both the auth `code` 
    // plus the echoed `state`
    let CallbackOutcome { code, state: echoed_state } = callback_rx.await??;

    // Check if they match
    if &echoed_state != state.secret() {
        anyhow::bail!("OAuth state mismatch: possible CSRF attack!");
    }
    
    // TODO 
}
```

