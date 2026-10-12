# oauth2-client

[![version](https://img.shields.io/badge/version-0.1.0-blue)](https://github.com/pangiole/oauth2-client)
[![Last commit](https://img.shields.io/github/last-commit/pangiole/oauth2-client)](https://github.com/pangiole/oauth2-client/commits/main)
[![made with Rust](https://img.shields.io/badge/made%20with-Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![license: WTFPL](https://img.shields.io/github/license/pangiole/oauth2-client)](https://github.com/pangiole/oauth2-client/blob/main/LICENSE)

A small OAuth2 / OpenID Connect (OIDC) **client** written in Rust with the
[Tokio](https://tokio.rs) asynchronous runtime. It runs the OAuth2 *Authorization
Code* flow against a real identity provider, keeps the refresh token on disk, and
reuses it across runs — so you only sign in through the browser once.

> 📖 This repository ships a companion **mdBook** that walks through the
> implementation step by step. Read it at
> <https://pangiole.github.io/oauth2-client/>.

## What it does

- Discovers the provider's endpoints from its
  `/.well-known/openid-configuration` document.
- Opens the provider's *authorize* URL in the browser.
- Runs an ephemeral local HTTP server to receive the authorization code.
- Exchanges the code for tokens, validates the ID token, and stores the refresh
  token (at `OAUTH_REFRESH_TOKEN_PATH`, default `.refresh-token`, git-ignored)
  for the next run.

Supported identity providers (selected via `OIDC_ISSUER_TYPE`):
`google`, `github`, `auth0`, `keycloak`, `linkedin`.

## Build

```sh
cargo build
```

## Run

Configuration is read from the process environment; a local `.env` file is only
a convenience to bootstrap it (the environment stays the single source of truth).

```sh
OIDC_ISSUER_TYPE=Google
OIDC_ISSUER_URL=https://accounts.google.com
OAUTH_CLIENT_ID=<your-client-id>
OAUTH_CLIENT_SECRET=<your-client-secret>
OAUTH_SCOPES=<space-separated-scopes>
OAUTH_REFRESH_TOKEN_PATH=<absolute-or-relative-path>
```

`OAUTH_REFRESH_TOKEN_PATH` is optional and defaults to `.refresh-token`. It may
be either an absolute or a relative path; any missing intermediate directories
are created automatically, and the run aborts with a clear error if the path
cannot be written (for example, due to filesystem permissions).

```sh
cargo run
```

The first run opens your browser to authenticate; later runs reuse the stored
refresh token.

### Local Keycloak (optional)

A development Keycloak is available via Docker Compose:

```sh
docker compose up   # http://localhost:8080, admin/admin
```

then point the client at the imported `master` realm:

```sh
OIDC_ISSUER_TYPE=keycloak
OIDC_ISSUER_URL=http://localhost:8080/realms/master
```

## Documentation

The step-by-step guide is an mdBook under [`docs/`](docs/):

```sh
mdbook serve docs   # live preview at http://localhost:3000
mdbook build docs   # static HTML in docs/book/
```

It is published through the main site — <https://pangiole.github.io/oauth2-client/> —
rather than to GitHub Pages here. On every push to `main`, the
[`.github/workflows/notify-site.yml`](.github/workflows/notify-site.yml) workflow
notifies the `pangiole.github.io` repository, which bumps this repo's gitlink and
rebuilds the book into the site.
