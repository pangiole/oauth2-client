use anyhow::{Context, Result};
use tokio::fs;

const REFRESH_TOKEN_FILE: &str = ".refresh_token";

/// Load the persisted refresh token, if any.
///
/// Returns `None` when no token has been stored yet (for example on the very
/// first run, before the Authorization Code flow has completed).
pub async fn load_refresh_token() -> Result<Option<String>> {
    let path = std::path::Path::new(REFRESH_TOKEN_FILE);

    if !fs::try_exists(path)
        .await
        .context("failed to check refresh token file")?
    {
        return Ok(None);
    }

    let token = fs::read_to_string(path)
        .await
        .context("failed to read refresh token file")?;

    let token = token.trim().to_string();
    if token.is_empty() {
        Ok(None)
    } else {
        Ok(Some(token))
    }
}

/// Persist the refresh token so it can be reused on subsequent runs.
pub async fn save_refresh_token(refresh_token: &str) -> Result<()> {
    fs::write(REFRESH_TOKEN_FILE, refresh_token)
        .await
        .context("failed to write refresh token file")?;

    Ok(())
}
