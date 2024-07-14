use crate::{
    github::{download_latest_release, get_latest_release},
    ui::{show_error, show_info, ICON_BYTES, WINDOW_TITLE},
};
use anyhow::Context;
use iced::{
    executor,
    theme::Palette,
    widget::{button, column, container, text, Button, Column, Text},
    window::{self, icon, resize},
    Application, Color, Command, Length, Settings, Size, Theme,
};
use log::{debug, error};
use sha256::try_digest;
use std::path::{Path, PathBuf};
use tokio::task::spawn_blocking;

/// The window size
const WINDOW_SIZE: (u32, u32) = (500, 140);
const EXPANDED_WINDOW_SIZE: (u32, u32) = (500, 200);

/// Initializes the user interface
///
/// ## Arguments
/// * `config` - The client config to use
/// * `client` - The HTTP client to use
pub fn init() {
    App::run(Settings {
        window: window::Settings {
            icon: icon::from_file_data(ICON_BYTES, None).ok(),
            size: WINDOW_SIZE,
            resizable: false,

            ..window::Settings::default()
        },
        flags: (),
        ..Settings::default()
    })
    .unwrap();
}

struct App {
    game_state: Option<GameState>,
    plugin_status: AddPluginStatus,
}

#[derive(Debug, Clone)]
enum AppMessage {
    PickGamePath,
    PickedGamePath(Option<GameState>),

    RemovePatch,
    ApplyPatch,

    RemovedPatch(Result<(), String>),
    AppliedPatch(Result<(), String>),

    AddPlugin,
    RemovePlugin,

    AddedPlugin(Result<(), String>),
    RemovedPlugin(Result<(), String>),
}

enum AddPluginStatus {
    None,
    Added,
    Downloading,
}

#[derive(Debug, Clone)]
struct GameState {
    patch: bool,
    plugin: bool,
    path: PathBuf,
}

/// Unpatched binkw32.dll
const BINK_UNPATCHED: &[u8] = include_bytes!("./resources/binkw23.dll");
/// Patched binkw32.dll
const BINK_PATCHED: &[u8] = include_bytes!("./resources/binkw32.dll");

/// Hash of the official binkw32.dll file
const OFFICIAL_BINKW32_HASH: &str =
    "a4ddcf8d78eac388cbc85155ef37a251a77f50de79d0b975ab9bb65bd0375698";

/// Reads the current patch and plugin state from the provided
/// game path
fn read_game_state(exe_path: &Path) -> anyhow::Result<GameState> {
    let parent = exe_path.parent().context("missing game folder")?;
    let binkw32_path = parent.join("binkw32.dll");
    let asi_path = parent.join("ASI");

    let plugin_path = asi_path.join("pocket-relay-plugin.asi");

    let digest = try_digest(binkw32_path).context("failed to get binkw32.dll hash")?;
    let patch = digest != OFFICIAL_BINKW32_HASH;

    let plugin = plugin_path.exists() && plugin_path.is_file();

    debug!("binkw32 hash is: (unofficial: {patch}) {digest}");

    Ok(GameState {
        path: exe_path.to_path_buf(),
        patch,
        plugin,
    })
}

/// Writes an unpatched version of the binkw32.dll to binkw23.dll and
/// overwrites the binkw32.dll with a patched version
async fn apply_patch(exe_path: &Path) -> anyhow::Result<()> {
    let parent = exe_path.parent().context("missing game folder")?;
    let binkw32_path = parent.join("binkw32.dll");
    let binkw23_path = parent.join("binkw23.dll");

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
async fn remove_patch(exe_path: &Path) -> anyhow::Result<()> {
    let parent = exe_path.parent().context("missing game folder")?;
    let binkw32_path = parent.join("binkw32.dll");
    let binkw23_path = parent.join("binkw23.dll");

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

/// Client user agent created from the name and version
pub const USER_AGENT: &str = concat!("PocketRelayPluginInstaller/v", env!("CARGO_PKG_VERSION"));

async fn apply_plugin(exe_path: &Path) -> anyhow::Result<()> {
    /// The GitHub repository to use for releases
    pub const GITHUB_REPOSITORY: &str = "PocketRelay/PocketRelayClientPlugin";
    /// GitHub asset name for the plugin file
    pub const ASSET_NAME: &str = "pocket-relay-plugin.asi";

    let parent = exe_path.parent().context("missing game folder")?;
    let asi_path = parent.join("ASI");
    let plugin_path = asi_path.join("pocket-relay-plugin.asi");

    let http_client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build http client")?;
    let latest_release = get_latest_release(&http_client, GITHUB_REPOSITORY)
        .await
        .context("failed finding latest plugin client version")?;

    let asset = latest_release
        .assets
        .iter()
        .find(|asset| asset.name == ASSET_NAME)
        .context("missing plugin asset file")?;

    let bytes = download_latest_release(&http_client, asset)
        .await
        .context("failed to download client plugin")?;

    tokio::fs::write(plugin_path, bytes)
        .await
        .context("saving plugin file")?;

    Ok(())
}

async fn remove_plugin(exe_path: &Path) -> anyhow::Result<()> {
    let parent = exe_path.parent().context("missing game folder")?;
    let asi_path = parent.join("ASI");
    let plugin_path = asi_path.join("pocket-relay-plugin.asi");
    tokio::fs::remove_file(plugin_path).await?;
    Ok(())
}

impl Application for App {
    type Message = AppMessage;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            App {
                game_state: None,
                plugin_status: AddPluginStatus::None,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        WINDOW_TITLE.to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            AppMessage::PickGamePath => {
                return Command::perform(
                    async move {
                        spawn_blocking(|| {
                            let path = native_dialog::FileDialog::new()
                                .add_filter("MassEffect3.exe", &["exe"])
                                .set_filename("MassEffect3.exe")
                                .set_title("Choose game executable")
                                .show_open_single_file()
                                .context("failed to pick file")?;
                            let path = match path {
                                Some(path) => path,
                                None => return anyhow::Ok(None),
                            };

                            let game_state = read_game_state(path.as_ref())?;
                            Ok(Some(game_state))
                        })
                        .await
                        .context("failed to join native thread")?
                        .context("failed to pick file")
                    },
                    |result| match result {
                        Ok(value) => AppMessage::PickedGamePath(value),
                        Err(err) => {
                            let error_message =
                                format!("error occurred while picking file: {err:?}");
                            show_error("Failed to pick file", &error_message);
                            AppMessage::PickedGamePath(None)
                        }
                    },
                );
            }
            AppMessage::PickedGamePath(state) => {
                debug!("picked path: {state:?}");
                if let Some(state) = state {
                    self.game_state = Some(state);

                    // Resize window to fit next screen
                    return resize(Size::new(EXPANDED_WINDOW_SIZE.0, EXPANDED_WINDOW_SIZE.1));
                }
            }
            AppMessage::RemovePatch => {
                if let Some(state) = self.game_state.clone() {
                    return Command::perform(
                        async move {
                            remove_patch(&state.path).await.map_err(|err| {
                                error!("{err:?}");
                                format!("{err:?}")
                            })
                        },
                        AppMessage::RemovedPatch,
                    );
                }
            }
            AppMessage::ApplyPatch => {
                if let Some(state) = self.game_state.clone() {
                    return Command::perform(
                        async move {
                            apply_patch(&state.path).await.map_err(|err| {
                                error!("{err:?}");
                                format!("{err:?}")
                            })
                        },
                        AppMessage::AppliedPatch,
                    );
                }
            }
            AppMessage::RemovedPatch(result) => {
                if let Err(err) = result {
                    show_error("Failed to remove patch", &err);
                } else {
                    show_info("Removed patch", "Successfully removed patch");

                    if let Some(game) = self.game_state.as_mut() {
                        game.patch = false;
                    }
                }
            }
            AppMessage::AppliedPatch(result) => {
                if let Err(err) = result {
                    show_error("Failed to apply patch", &err);
                } else {
                    show_info("Applied patch", "Successfully applied patch");

                    if let Some(game) = self.game_state.as_mut() {
                        game.patch = true;
                    }
                }
            }
            AppMessage::AddPlugin => {
                self.plugin_status = AddPluginStatus::Downloading;

                if let Some(state) = self.game_state.clone() {
                    return Command::perform(
                        async move {
                            apply_plugin(&state.path).await.map_err(|err| {
                                error!("{err:?}");
                                format!("{err:?}")
                            })
                        },
                        AppMessage::AddedPlugin,
                    );
                }
            }
            AppMessage::RemovePlugin => {
                self.plugin_status = AddPluginStatus::None;

                if let Some(state) = self.game_state.clone() {
                    return Command::perform(
                        async move {
                            remove_plugin(&state.path).await.map_err(|err| {
                                error!("{err:?}");
                                err.to_string()
                            })
                        },
                        AppMessage::RemovedPlugin,
                    );
                }
            }
            AppMessage::AddedPlugin(result) => {
                if let Err(err) = result {
                    show_error("Failed to apply plugin", &err);
                    self.plugin_status = AddPluginStatus::None;
                } else {
                    show_info("Applied plugin", "Successfully applied plugin");
                    self.plugin_status = AddPluginStatus::Added;

                    if let Some(game) = self.game_state.as_mut() {
                        game.plugin = true;
                    }
                }
            }
            AppMessage::RemovedPlugin(result) => {
                if let Err(err) = result {
                    show_error("Failed to remove plugin", &err);
                } else {
                    show_info("Removed plugin", "Successfully removed plugin");

                    if let Some(game) = self.game_state.as_mut() {
                        game.plugin = false;
                    }
                }
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        const DARK_TEXT: Color = Color::from_rgb(0.4, 0.4, 0.4);
        const SPACING: u16 = 10;

        let game_state = match self.game_state.as_ref() {
            Some(value) => value,
            None => {
                let target_text: Text = text(
                    "Please click the button below to choose your game path. \
                    When the file picker opens navigate to the folder containing \
                    MassEffect3.exe and pick that file",
                )
                .style(DARK_TEXT);

                let pick_button: Button<_> = button("Choose game path")
                    .on_press(AppMessage::PickGamePath)
                    .padding(10);

                let content: Column<_> = column![target_text, pick_button].spacing(10);

                return container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(SPACING)
                    .into();
            }
        };

        // Section for applying and removing the patch
        let patch_section = if game_state.patch {
            let patch_text: Text = text("Your game is patched").style(DARK_TEXT);
            let remove_patch_button: Button<_> = button("Remove Patch")
                .on_press(AppMessage::RemovePatch)
                .padding(10);

            column![patch_text, remove_patch_button].spacing(10)
        } else {
            let patch_text: Text = text(
                "Your game is not patched, you must apply the patch to use the client plugin.",
            )
            .style(DARK_TEXT);
            let apply_patch_button: Button<_> = button("Apply Patch")
                .on_press(AppMessage::ApplyPatch)
                .padding(10);

            column![patch_text, apply_patch_button].spacing(10)
        };

        // Section for applying and removing the plugin
        let plugin_section = if game_state.plugin {
            let plugin_text: Text =
                text("You have the Pocket Relay client plugin installed.").style(DARK_TEXT);
            let remove_plugin_button: Button<_> = button("Remove Plugin")
                .on_press(AppMessage::RemovePlugin)
                .padding(10);

            column![plugin_text, remove_plugin_button].spacing(10)
        } else {
            let plugin_text: Text =
                text("You do not have the Pocket Relay client plugin installed").style(DARK_TEXT);
            let add_plugin_button: Button<_> = button("Add Plugin")
                .on_press(AppMessage::AddPlugin)
                .padding(10);
            column![plugin_text, add_plugin_button].spacing(10)
        };

        // Plugin status text
        let status_text: Text = match &self.plugin_status {
            AddPluginStatus::None => text("Idle..").style(DARK_TEXT),
            AddPluginStatus::Added => text("Plugin applied.").style(Palette::DARK.success),
            AddPluginStatus::Downloading => text("Downloading...").style(Palette::DARK.success),
        };

        let content: Column<_> = column![patch_section, plugin_section, status_text].spacing(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SPACING)
            .into()
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}
