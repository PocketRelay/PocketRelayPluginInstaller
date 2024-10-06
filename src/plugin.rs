use std::path::Path;

use anyhow::Context;
use log::debug;

use crate::github::{download_latest_release, get_latest_release, get_releases, GitHubRelease};

/// Client user agent created from the name and version
pub const USER_AGENT: &str = concat!("PocketRelayPluginInstaller/v", env!("CARGO_PKG_VERSION"));

/// The GitHub repository to use for releases
pub const GITHUB_REPOSITORY: &str = "PocketRelay/PocketRelayClientPlugin";
/// GitHub asset name for the plugin file
pub const ASSET_NAME: &str = "pocket-relay-plugin.asi";

/// Determines the latest release version of the plugin
pub async fn get_latest_plugin_release() -> anyhow::Result<GitHubRelease> {
    let http_client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build http client")?;

    let latest_release = get_latest_release(&http_client, GITHUB_REPOSITORY)
        .await
        .context("failed finding latest plugin client version")?;

    Ok(latest_release)
}
/// Determines the latest release version of the plugin
pub async fn get_latest_beta_plugin_release() -> anyhow::Result<Option<GitHubRelease>> {
    let http_client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build http client")?;

    let mut releases = get_releases(&http_client, GITHUB_REPOSITORY)
        .await
        .context("failed finding latest plugin client version")?;
    releases.retain(|value| value.prerelease);

    releases.sort_by(|a, b| a.published_at.cmp(&b.published_at).reverse());

    debug!("{:?}", releases);

    Ok(releases.first().cloned())
}

pub async fn apply_plugin(game_path: &Path, release: &GitHubRelease) -> anyhow::Result<()> {
    let asi_path = game_path.join("ASI");
    let plugin_path = asi_path.join("pocket-relay-plugin.asi");

    let http_client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build http client")?;

    // Find the asset for the plugin file
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == ASSET_NAME)
        .context("missing plugin asset file")?;

    // Download the asset
    let bytes = download_latest_release(&http_client, asset)
        .await
        .context("failed to download client plugin")?;

    // Save the plugin to the plugins directory
    tokio::fs::write(plugin_path, bytes)
        .await
        .context("saving plugin file")?;

    debug!("applied plugin");

    Ok(())
}

pub async fn remove_plugin(game_path: &Path) -> anyhow::Result<()> {
    let asi_path = game_path.join("ASI");
    let plugin_path = asi_path.join("pocket-relay-plugin.asi");
    tokio::fs::remove_file(plugin_path).await?;
    Ok(())
}
