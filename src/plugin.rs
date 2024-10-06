//! Module for helpers related to finding plugin releases and applying/removing the plugin
//! from the game

use crate::github::{download_latest_release, get_latest_release, get_releases, GitHubRelease};
use anyhow::Context;
use log::debug;
use std::path::PathBuf;

/// Client user agent created from the name and version
pub const USER_AGENT: &str = concat!("PocketRelayPluginInstaller/v", env!("CARGO_PKG_VERSION"));

/// The GitHub repository to use for releases
pub const GITHUB_REPOSITORY: &str = "PocketRelay/PocketRelayClientPlugin";
/// GitHub asset name for the plugin file
pub const ASSET_NAME: &str = "pocket-relay-plugin.asi";

/// Name of the plugin directory
pub const PLUGIN_DIR: &str = "ASI";

/// Name of the plugin file
pub const PLUGIN_NAME: &str = "pocket-relay-plugin.asi";

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

/// Finds the latest beta release of the plugin by searching for the newest
/// release marked as a prerelease
pub async fn get_latest_beta_plugin_release() -> anyhow::Result<Option<GitHubRelease>> {
    let http_client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build http client")?;

    // Request the list of releases
    let mut releases = get_releases(&http_client, GITHUB_REPOSITORY)
        .await
        .context("failed finding latest plugin client version")?;

    // Retain only the prerelease's
    releases.retain(|value| value.prerelease);

    // Sort on the published_at descending
    releases.sort_by(|a, b| a.published_at.cmp(&b.published_at).reverse());

    Ok(releases.first().cloned())
}

/// Applies the plugin from the provided `release`, downloads the plugin and saves
/// it to the plugin directory
pub async fn apply_plugin(game_path: PathBuf, release: GitHubRelease) -> anyhow::Result<()> {
    let asi_path = game_path.join(PLUGIN_DIR);
    let plugin_path = asi_path.join(PLUGIN_NAME);

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

/// Removes the plugin from the game directory
pub async fn remove_plugin(game_path: PathBuf) -> anyhow::Result<()> {
    let asi_path = game_path.join(PLUGIN_DIR);
    let plugin_path = asi_path.join(PLUGIN_NAME);
    tokio::fs::remove_file(plugin_path).await?;
    Ok(())
}
