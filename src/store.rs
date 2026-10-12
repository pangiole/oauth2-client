use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;

use crate::config;

/// Resolve the configured path where the refresh token is persisted.
///
/// The location comes from the `OAUTH_REFRESH_TOKEN_PATH` environment variable
/// (see [`crate::config::AppConfig::refresh_token_path`]) and may be either an
/// absolute or a relative path.
fn refresh_token_path() -> PathBuf {
    config::get().refresh_token_path.clone()
}

/// Load the persisted refresh token, if any.
///
/// Returns `None` when no token has been stored yet (for example on the very
/// first run, before the Authorization Code flow has completed).
pub async fn load_refresh_token() -> Result<Option<String>> {
    let path = refresh_token_path();

    if !fs::try_exists(&path)
        .await
        .with_context(|| format!("failed to check refresh token file {}", path.display()))?
    {
        return Ok(None);
    }

    let token = fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read refresh token file {}", path.display()))?;

    let token = token.trim().to_string();
    if token.is_empty() {
        Ok(None)
    } else {
        Ok(Some(token))
    }
}

/// Persist the refresh token so it can be reused on subsequent runs.
///
/// Missing intermediate directories are created on demand, so the configured
/// path can point deep into the filesystem. Any filesystem failure — notably a
/// permission-denied write — is reported with the offending path and bubbles up
/// to abort the run.
pub async fn save_refresh_token(refresh_token: &str) -> Result<()> {
    let path = refresh_token_path();

    // Create all missing parent components. A bare filename such as
    // `.refresh-token` has an empty parent, which `create_dir_all` rejects, so
    // skip it and let the write target the current working directory.
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).await.with_context(|| {
            format!(
                "failed to create directory {} for refresh token",
                parent.display()
            )
        })?;
    }

    fs::write(&path, refresh_token)
        .await
        .with_context(|| format!("failed to write refresh token to {}", path.display()))?;

    Ok(())
}
