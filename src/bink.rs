//! Module for helpers related to patching the game using the binkw32 DLL

use std::path::{Path, PathBuf};

use anyhow::Context;
use log::debug;
use sha256::try_async_digest;

/// Unpatched binkw32.dll
const BINK_UNPATCHED: &[u8] = include_bytes!("./resources/binkw23.dll");
/// Patched binkw32.dll
const BINK_PATCHED: &[u8] = include_bytes!("./resources/binkw32.dll");

/// Hash of the official binkw32.dll file, used to check if the game has already
/// been patched (SHA256)
const OFFICIAL_BINKW32_HASH: &str =
    "a4ddcf8d78eac388cbc85155ef37a251a77f50de79d0b975ab9bb65bd0375698";

/// Checks if the binkw32.dll at the provided game path is already patched
pub async fn is_patched(game_path: &Path) -> anyhow::Result<bool> {
    let binkw32_path = game_path.join("binkw32.dll");

    // Obtain the sha256 hash of the binkw32.dll
    let digest = try_async_digest(binkw32_path)
        .await
        .context("failed to get binkw32.dll hash")?;

    let is_patched = digest != OFFICIAL_BINKW32_HASH;

    debug!("binkw32 hash is: (unofficial: {is_patched}) {digest}");

    Ok(is_patched)
}

/// Writes an unpatched version of the binkw32.dll to binkw23.dll and
/// overwrites the binkw32.dll with a patched version
pub async fn apply_patch(game_path: PathBuf) -> anyhow::Result<()> {
    let binkw32_path = game_path.join("binkw32.dll");
    let binkw23_path = game_path.join("binkw23.dll");

    tokio::fs::write(binkw32_path, BINK_PATCHED)
        .await
        .context("failed to write patch")?;
    tokio::fs::write(binkw23_path, BINK_UNPATCHED)
        .await
        .context("failed to write unpatched")?;

    Ok(())
}

/// Writes an unpatched version of the binkw32.dll and removes
/// the old binkw23.dll
pub async fn remove_patch(game_path: PathBuf) -> anyhow::Result<()> {
    let binkw32_path = game_path.join("binkw32.dll");
    let binkw23_path = game_path.join("binkw23.dll");

    tokio::fs::write(binkw32_path, BINK_UNPATCHED)
        .await
        .context("failed to write unpatched")?;
    if binkw23_path.exists() {
        tokio::fs::remove_file(binkw23_path)
            .await
            .context("failed to remove patched")?;
    }
    Ok(())
}
