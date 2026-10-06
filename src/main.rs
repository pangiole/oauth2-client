mod config;
mod callback;
mod oidc;
mod store;

use anyhow::Result;
use tracing::{error, info};

#[tokio::main]
async fn main() -> () {
    tracing_subscriber::fmt::init();
    if let Err(e) = run().await {
        error!("{}", e);
    }
    else {
        info!("Completed successfully!");
    }
}


async fn run() -> Result<()> {
    config::load_app_config()?;

    let access_token = oidc::get_access_token().await?;
    info!("access token: {:?}", access_token);
    // TODO Finally use the access_token to access the protected resources

    Ok(())
}


