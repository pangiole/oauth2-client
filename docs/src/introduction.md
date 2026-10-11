# How-to implement OAuth2 clients in Rust
This small book explains how to implement an OAuth2 / OIDC (OpenID Connect) 
client application in Rust language using the Tokio asynchronous runtime. 

By adopting common Rust idioms and patterns, we demonstrate how to run both 
the OAuth2 Authorization Code and Refresh Token flows by interacting with 
popular Authorization Servers having IdP - Identity Provider capabilities, such 
as Google Account, GitHub, LinkedIn, Keycloack etc.


> **TL;DR**  
> * Discover the provider's endpoints from its 
>   `/.well-known/openid-configuration` document.
>
>
> * Generate a CSRF `state` and an OIDC `nonce`, then open the
>   provider authorize URL in the browser.
>
>
> * Start an ephemeral HTTP server on `127.0.0.1:0` to 
>   receive the authorization code on a `/callback` endpoint.
> 
> 
> * Exchange the returned auth code for tokens, validate the 
>   ID token, and persist the refresh token to disk.
> 
> 
> * On subsequent runs, exchange the stored refresh token for 
>   a new access token and fall back to the browser flow only 
>   when that fails.
