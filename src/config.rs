use anyhow::{anyhow, Context, Result};
use std::sync::OnceLock;
use tracing::info;


/// The OAuth2/OIDC provider (Authorization Server) this client is talking to.
///
/// Providers differ in how they request a refresh token: most use the standard
/// `offline_access` scope, whereas Google uses proprietary authorization
/// parameters (`access_type=offline`, `prompt=consent`). Knowing which provider
/// we target lets the client apply the right quirks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssuerType {
    Google,
    GitHub,
    Auth0,
    Keycloak,
    Authentik,
}


impl IssuerType {
    /// The canonical, lower-case identifier (mirrors the accepted `.env` values).
    pub fn as_str(&self) -> &'static str {
        match self {
            IssuerType::Google => "google",
            IssuerType::GitHub => "github",
            IssuerType::Auth0 => "auth0",
            IssuerType::Keycloak => "keycloak",
            IssuerType::Authentik => "authentik",
        }
    }
}


impl std::fmt::Display for IssuerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}


impl std::str::FromStr for IssuerType {
    type Err = anyhow::Error;

    /// Parses the issuer type case-insensitively.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "google" => Ok(IssuerType::Google),
            "github" => Ok(IssuerType::GitHub),
            "auth0" => Ok(IssuerType::Auth0),
            "keycloak" => Ok(IssuerType::Keycloak),
            "authentik" => Ok(IssuerType::Authentik),
            other => Err(anyhow!(
                "unknown OIDC_ISSUER_TYPE value '{other}'; expected one of: \
                 google, github, auth0, keycloak, authentik"
            )),
        }
    }
}


pub struct AppConfig {
    pub issuer_url: String,
    pub issuer_type: IssuerType,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
}


static CONFIG: OnceLock<AppConfig> = OnceLock::new();


/// Returns the process-wide, read-only configuration.
///
/// # Panic
/// Panics if `load` has not been called (or failed) first. This is a
/// programmer-error invariant, not a recoverable runtime error.
pub fn get() -> &'static AppConfig {
    CONFIG.get().expect("AppConfig not initialized; call load() first")
}


/// Load the configuration from the process environment.
///
/// A local `.env` file is loaded as a development convenience, but it is *not*
/// required: the same variables may already be present in the process
/// environment (for example when running inside a Docker container). A missing
/// file is treated as a no-op; only a genuine error (such as a malformed file)
/// fails startup.
///
/// Environment variables take precedence over values in `.env`: `dotenv` only
/// fills in variables that are not already set, keeping the environment as the
/// single source of truth (see <https://12factor.net/config>).
pub fn load() -> Result<()> {
    info!("Loading configuration");

    // A missing `.env` is fine: the variables may already be set in the process
    // environment (e.g. by the shell or a container). Only propagate genuine
    // errors, such as a malformed file.
    if let Err(err) = dotenvy::dotenv()
        && !err.not_found()
    {
        return Err(anyhow::Error::new(err).context("failed to load .env file"));
    }

    // We are using the discovery features of the OIDC protocol
    let issuer_url = dotenvy::var("OIDC_ISSUER_URL")
        .context("OIDC_ISSUER_URL must be set as environment variable")?;

    // Which provider we target, so to apply its specific authorization quirks.
    let issuer_type = dotenvy::var("OIDC_ISSUER_TYPE")
        .context("OIDC_ISSUER_TYPE must be set as environment variable")?
        .parse::<IssuerType>()?;

    // Since this app is an OAuth2 confidential client, we can safely hold both the client
    // identifier and the secret.
    // Note that these have been produced by the Authorization Server upon registering our client.
    // See https://console.cloud.google.com/auth/overview?project=bubbly-mantis-184512

    let client_id = dotenvy::var("OAUTH_CLIENT_ID")
        .context("OAUTH_CLIENT_ID must be set as environment variable")?;

    let client_secret = dotenvy::var("OAUTH_CLIENT_SECRET")
        .context("OAUTH_CLIENT_SECRET must be set as environment variable")?;

    // Space-separated, per the OAuth2 "scope" parameter format.
    let scopes = dotenvy::var("OAUTH_SCOPES")
        .context("OAUTH_SCOPES must be set as environment variable")?
        .split_whitespace()
        .map(String::from)
        .collect::<Vec<String>>();

    let loaded_config = AppConfig {
        issuer_url,
        issuer_type,
        client_id,
        client_secret,
        scopes,
    };

    CONFIG
        .set(loaded_config)
        .map_err(|_| anyhow!("AppConfig is already initialized"))
}