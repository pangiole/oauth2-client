{{#include ../warning.md}}

# Parameters
The Authorization Code Grant's front-channel carries only a handful of 
parameters, and each one exists for one of two reasons: to *orchestrate* the
flow (routing the request and its answer) or to *secure* it (binding the answer
to the session that started it). The sequence diagram above shows only the
orchestration parameters for clarity; the two security parameters below travel
on the same front-channel request.

## Core protocol orchestration
These parameters are bout who is asking, and where the answer goes:

* `response_type=code`  
  tells the Authorization Server to return an authorization code instead of 
  a token.


* `client_id`  
  identifies which OAuth2 Client application is asking.


* `redirect_uri`  
  the callback URL the Authorization Server redirects the user (and the `code`)
  back to after consent.


* `scope=openid`  
  declares the OpenID Connect intent: authenticate the user and return an 
  identity token plus an access token for the `/userinfo` endpoint.


### Scopes
...

## Vulnerability resolutions
These additional parameters bind the answer back to the  request and serve 
the purpose to resolve vulnerabilities and mitigate security attacks (as 
better explained in the next section):

* `state`  
  an opaque, unguessable value the client generates and stores before sending 
  the authorization request; the Authorization Server echoes it back verbatim on
  the callback, and the client rejects the callback if the two differ. This 
  single check defeats the account-linking attack below, which is why `state` is
  commonly referred to simply as the *CSRF token*.


* `nonce`  
  an OpenID Connect addition: a one-time value the client sends with the 
  authorization request and the Authorization Server binds into the returned 
  `id_token`. The client compares it when validating the ID token, so a token
  replayed from an unrelated flow is rejected.
