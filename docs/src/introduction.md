{{#include warning.md}}

# Walkthrough an OAuth2 Client in Rust

This book explains how to implement an OAuth2 / OIDC (OpenID Connect) Client 
application in Rust language using the [Tokio](https://tokio.rs/) asynchronous 
runtime and library stack. 

By adopting the best Rust idioms and coding practices, we demonstrate how to run 
through the OAuth2 **Authorization Code Grant** 
(see [RFC 6749, §4.1](https://www.rfc-editor.org/info/rfc6749/#section-4.1)) 
and how to execute the **Refreshing an Access Token** operation 
(see [RFC 6749, §6](https://www.rfc-editor.org/info/rfc6749/#section-6)). We 
demonstrate how to make our OAuth2 Client application interact with some of the most 
popular OAuth2 Authorization Servers, such as Google Accounts, GitHub, LinkedIn, 
Keycloak, just to name a few of those built with IdP - Identity Provider 
capabilities.

The full Rust source code of this OAuth2 Client application is open source and 
publicly available on GitHub at <https://github.com/pangiole/oauth2-client>.


> [!NOTE]
> This book explains how to:
>
> * discover the identity provider's endpoints from its 
>   `/.well-known/openid-configuration` document.
>
>
> * generate a CSRF `state` and an OIDC `nonce`, then open the
>   provider authorize URL in the browser.
>
>
> * start an ephemeral HTTP server on `127.0.0.1:0` to 
>   receive the authorization code on a `/callback` endpoint.
> 
> 
> * exchange the returned auth code for tokens, validate the 
>   ID token, and persist the refresh token to disk.
> 
> 
> * on subsequent runs, exchange the stored refresh token for 
>   a new access token and fall back to the browser flow only 
>   when that fails.

 
