use crate::{
    bink::{apply_patch, is_patched, remove_patch},
    github::GitHubRelease,
    plugin::{apply_plugin, get_latest_plugin_release, remove_plugin},
    ui::{show_error, show_info, ICON_BYTES, WINDOW_TITLE},
};
use anyhow::Context;
use iced::{
    executor,
    theme::Palette,
    widget::{button, column, combo_box, container, row, text, Button, Column, Text},
    window::{self, icon, resize},
    Application, Color, Command, Length, Settings, Size, Theme,
};
use log::{debug, error};
use std::{
    default,
    fmt::{Display, Write},
    path::{Path, PathBuf},
};
use tokio::task::spawn_blocking;

/// The window size
const WINDOW_SIZE: (u32, u32) = (500, 140);
const EXPANDED_WINDOW_SIZE: (u32, u32) = (500, 300);

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
    state: AppState,

    /// Status for the remote plugin details
    plugin_details_state: PluginDetailsState,
}

#[derive(Default)]
pub enum AppState {
    /// Initial state, no game has been picked yet
    #[default]
    Initial,

    /// Active state, game has been selected and its
    /// details are known
    Active(AppStateActive),
}

pub struct AppStateActive {
    /// Whether the game is patched
    patched: bool,

    /// Whether the plugin is installed
    plugin: bool,

    /// Selected game folder path
    path: PathBuf,

    /// Current status of adding/removing a plugin
    alter_plugin_state: AlterPluginState,
}

#[derive(Debug, Clone)]
enum PatchMessage {
    /// Applies the patch to the game
    Add,
    /// Remove the patch from the game
    Remove,

    /// Result of applying the patch to the game
    Added(Result<(), String>),
    /// Result of removing the patch from the game
    Removed(Result<(), String>),
}

#[derive(Debug, Clone)]
enum PluginMessage {
    /// Adds the plugin to the game
    Add,
    /// Remove the plugin from the game
    Remove,
    /// Select a different plugin version type
    SelectType(ReleaseType),

    /// Result of adding the plugin to the game
    Added(Result<(), String>),
    /// Result of removing the plugin from the game
    Removed(Result<(), String>),
}

#[derive(Debug, Clone)]
enum AppMessage {
    /// Trigger the popup to allow the user to pick the game path
    PickGamePath,
    /// Result of the user picking the game path
    PickedGamePath(Option<GameState>),

    /// Messages related to patching the game
    Patch(PatchMessage),

    /// Messages related to adding/removing the plugin
    Plugin(PluginMessage),

    /// Messages related to loading the plugin details
    PluginDetails(PluginDetailsMessage),
}

#[derive(Debug, Clone)]
enum PluginDetailsMessage {
    /// Result of adding the plugin to the game
    Loaded(Result<PluginDetails, String>),
}

/// Current state for the plugin details (Remote state from github)
#[derive(Default)]
pub enum PluginDetailsState {
    /// Loading details about the plugin
    #[default]
    Loading,

    /// Failed to load details about the plugin
    Error(String),

    /// Ready
    Ready(PluginDetails),
}

#[derive(Debug, Clone)]
pub struct PluginDetails {
    /// State for the release type combobox
    release_type_state: combo_box::State<ReleaseType>,
    /// Selected release type
    selected: ReleaseType,
}

/// Current state for the plugin add process
#[derive(Default)]
pub enum AlterPluginState {
    /// Initial state, plugin has not been added yet
    #[default]
    Initial,

    /// Loading state, plugin asset is being downloaded
    Loading,

    /// Plugin was added successfully
    Success,

    /// Failed to add the plugin
    Error(String),
}

#[derive(Debug, Clone)]
struct GameState {
    patched: bool,
    plugin: bool,
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum ReleaseType {
    Stable(GitHubRelease),
    Beta(GitHubRelease),
}

impl Display for ReleaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReleaseType::Stable(release) => write!(f, "Stable ({})", release.tag_name),
            ReleaseType::Beta(release) => write!(f, "Beta ({})", release.tag_name),
        }
    }
}

/// Reads the current patch and plugin state from the provided
/// game path
fn read_game_state(exe_path: &Path) -> anyhow::Result<GameState> {
    let parent = exe_path.parent().context("missing game folder")?;
    let asi_path = parent.join("ASI");

    let plugin_path = asi_path.join("pocket-relay-plugin.asi");
    let is_patched = is_patched(parent).context("failed to check game patched state")?;

    let plugin = plugin_path.exists() && plugin_path.is_file();

    Ok(GameState {
        path: parent.to_path_buf(),
        patched: is_patched,
        plugin,
    })
}

const DARK_TEXT: Color = Color::from_rgb(0.4, 0.4, 0.4);
const SPACING: u16 = 10;

impl App {
    fn update_patch(&mut self, msg: PatchMessage) -> Command<PatchMessage> {
        let state = match &mut self.state {
            AppState::Active(state) => state,
            _ => panic!("app reached invalid state, expecting 'Active' state"),
        };

        match msg {
            PatchMessage::Add => {
                let path = state.path.to_path_buf();
                return Command::perform(async move { apply_patch(&path).await }, |result| {
                    let result = result.map_err(|err| {
                        error!("failed to apply patch: {err:?}");
                        format!("{err:?}")
                    });

                    PatchMessage::Added(result)
                });
            }
            PatchMessage::Remove => {
                let path = state.path.to_path_buf();
                return Command::perform(async move { remove_patch(&path).await }, |result| {
                    let result = result.map_err(|err| {
                        error!("failed to remove patch: {err:?}");
                        format!("{err:?}")
                    });

                    PatchMessage::Removed(result)
                });
            }
            PatchMessage::Added(result) => {
                if let Err(err) = result {
                    show_error("Failed to apply patch", &err);
                } else {
                    state.patched = true;
                    show_info("Applied patch", "Successfully applied patch");
                }
            }
            PatchMessage::Removed(result) => {
                if let Err(err) = result {
                    show_error("Failed to remove patch", &err);
                } else {
                    state.patched = false;
                    show_info("Removed patch", "Successfully removed patch");
                }
            }
        }

        Command::none()
    }

    fn update_plugin(&mut self, msg: PluginMessage) -> Command<PluginMessage> {
        let state = match &mut self.state {
            AppState::Active(state) => state,
            _ => panic!("app reached invalid state, expecting 'Active' state"),
        };

        match msg {
            PluginMessage::Add => {
                let release = match &self.plugin_details_state {
                    PluginDetailsState::Ready(details) => &details.selected,
                    _ => panic!("invalid plugin details state, expecting 'Ready' state"),
                };

                let release = match release {
                    ReleaseType::Stable(value) => value.clone(),
                    ReleaseType::Beta(value) => value.clone(),
                };

                let path = state.path.to_path_buf();

                state.alter_plugin_state = AlterPluginState::Loading;

                return Command::perform(
                    async move { apply_plugin(&path, &release).await },
                    |result| {
                        let result = result.map_err(|err| {
                            error!("failed to add plugin: {err:?}");
                            format!("{err:?}")
                        });

                        PluginMessage::Added(result)
                    },
                );
            }
            PluginMessage::Remove => {
                let path = state.path.to_path_buf();

                state.alter_plugin_state = AlterPluginState::Initial;

                return Command::perform(async move { remove_plugin(&path).await }, |result| {
                    let result = result.map_err(|err| {
                        error!("failed to remove plugin: {err:?}");
                        format!("{err:?}")
                    });
                    PluginMessage::Removed(result)
                });
            }
            PluginMessage::Added(result) => {
                if let Err(err) = result {
                    show_error("Failed to apply plugin", &err);
                    state.alter_plugin_state = AlterPluginState::Error(err);
                } else {
                    show_info("Applied plugin", "Successfully applied plugin");
                    state.alter_plugin_state = AlterPluginState::Success;
                    state.plugin = true;
                }
            }
            PluginMessage::Removed(result) => {
                if let Err(err) = result {
                    show_error("Failed to remove plugin", &err);
                } else {
                    state.plugin = false;
                    show_info("Removed plugin", "Successfully removed plugin");
                }
            }
            PluginMessage::SelectType(release_type) => match &mut self.plugin_details_state {
                PluginDetailsState::Ready(plugin_details) => {
                    plugin_details.selected = release_type;
                }
                _ => {}
            },
        }

        Command::none()
    }

    fn update_plugin_details(
        &mut self,
        msg: PluginDetailsMessage,
    ) -> Command<PluginDetailsMessage> {
        match msg {
            PluginDetailsMessage::Loaded(result) => match result {
                Ok(value) => {
                    self.plugin_details_state = PluginDetailsState::Ready(value);
                }
                Err(err) => {
                    show_error("Failed to load plugin details", &err);

                    self.plugin_details_state = PluginDetailsState::Error(err);
                }
            },
        }

        Command::none()
    }

    fn render_patch_section(state: &AppStateActive) -> Column<'_, AppMessage> {
        // Game is already patched
        if state.patched {
            let patch_text: Text = text("Your game is patched").style(DARK_TEXT);
            let remove_patch_button: Button<_> = button("Remove Patch")
                .on_press(AppMessage::Patch(PatchMessage::Remove))
                .padding(10);

            return column![patch_text, remove_patch_button].spacing(10);
        }

        let patch_text: Text =
            text("Your game is not patched, you must apply the patch to use the client plugin.")
                .style(DARK_TEXT);
        let apply_patch_button: Button<_> = button("Apply Patch")
            .on_press(AppMessage::Patch(PatchMessage::Add))
            .padding(10);

        column![patch_text, apply_patch_button].spacing(10)
    }

    fn render_plugin_section<'a>(
        state: &'a AppStateActive,
        plugin_details: &'a PluginDetailsState,
    ) -> Column<'a, AppMessage> {
        match (state.plugin, &state.alter_plugin_state) {
            // Plugin is installed
            (true, AlterPluginState::Initial) => {
                let plugin_text: Text =
                    text("You have the Pocket Relay client plugin installed.").style(DARK_TEXT);
                let remove_plugin_button: Button<_> = button("Remove Plugin")
                    .on_press(AppMessage::Plugin(PluginMessage::Remove))
                    .padding(10);

                return column![plugin_text, remove_plugin_button].spacing(10);
            }

            // Plugin is not installed, we are in the initial state
            (false, AlterPluginState::Initial) => {
                match plugin_details {
                    // Still loading the plugin details
                    PluginDetailsState::Loading => {
                        let plugin_text: Text =
                            text("You do not have the Pocket Relay client plugin installed")
                                .style(DARK_TEXT);
                        let plugin_version_text: Text =
                            text("Loading latest plugin version details...").style(DARK_TEXT);
                        column![plugin_text, plugin_version_text].spacing(10)
                    }
                    PluginDetailsState::Error(_) => {
                        let plugin_text: Text =
                            text("You do not have the Pocket Relay client plugin installed")
                                .style(DARK_TEXT);
                        let plugin_version_text: Text =
                            text("Unable to load latest plugin version").style(DARK_TEXT);
                        column![plugin_text, plugin_version_text].spacing(10)
                    }
                    PluginDetailsState::Ready(plugin_details) => {
                        let plugin_text: Text =
                            text("You do not have the Pocket Relay client plugin installed")
                                .style(DARK_TEXT);

                        let release = match &plugin_details.selected {
                            ReleaseType::Stable(value) => value,
                            ReleaseType::Beta(value) => value,
                        };

                        let version = &release.tag_name;

                        let plugin_version_text: Text = text(format!(
                            "The latest version of the plugin client is {version}"
                        ))
                        .style(DARK_TEXT);

                        let add_plugin_button: Button<_> = button("Add Plugin")
                            .on_press(AppMessage::Plugin(PluginMessage::Add))
                            .padding(10);

                        let version_select = combo_box(
                            &plugin_details.release_type_state,
                            "Select version",
                            Some(&plugin_details.selected),
                            |value| AppMessage::Plugin(PluginMessage::SelectType(value)),
                        );

                        column![
                            plugin_text,
                            plugin_version_text,
                            row![add_plugin_button, version_select].spacing(10)
                        ]
                        .spacing(10)
                    }
                }
            }

            // Plugin is not installed, we are installing
            (false, AlterPluginState::Loading) => {
                let plugin_text = text("Installing plugin...").style(Palette::DARK.primary);
                column![plugin_text].spacing(10)
            }

            // Plugin is installed, we are uninstalling
            (true, AlterPluginState::Loading) => {
                let plugin_text = text("Uninstalling plugin...").style(Palette::DARK.primary);
                column![plugin_text].spacing(10)
            }

            // Plugin was uninstalled
            (false, AlterPluginState::Success) => {
                let plugin_text: Text = text("Pocket Relay client plugin successfully removed.")
                    .style(Palette::DARK.success);
                let add_plugin_button: Button<_> = button("Add Plugin")
                    .on_press(AppMessage::Plugin(PluginMessage::Add))
                    .padding(10);

                column![plugin_text, add_plugin_button].spacing(10)
            }

            // Plugin was installed
            (true, AlterPluginState::Success) => {
                let plugin_text: Text = text("Pocket Relay client plugin successfully installed.")
                    .style(Palette::DARK.success);
                let remove_plugin_button: Button<_> = button("Remove Plugin")
                    .on_press(AppMessage::Plugin(PluginMessage::Remove))
                    .padding(10);

                column![plugin_text, remove_plugin_button].spacing(10)
            }

            // Error occurred
            (plugin, AlterPluginState::Error(_)) => {
                let (message, action) = match plugin {
                    true => ("failed to remove plugin".to_string(), PluginMessage::Remove),
                    false => ("failed to install plugin".to_string(), PluginMessage::Add),
                };

                let text: Text = text(message).style(Palette::DARK.danger);

                let add_plugin_button: Button<_> = button("Retry")
                    .on_press(AppMessage::Plugin(action))
                    .padding(10);
                column![text, add_plugin_button].spacing(10)
            }
        }
    }
}

impl Application for App {
    type Message = AppMessage;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            App {
                state: Default::default(),
                plugin_details_state: Default::default(),
            },
            Command::perform(
                async move {
                    let release = get_latest_plugin_release().await?;
                    let release_type_state = combo_box::State::<ReleaseType>::new(vec![
                        ReleaseType::Stable(release.clone()),
                        ReleaseType::Beta(release.clone()),
                    ]);

                    Ok::<_, anyhow::Error>(PluginDetails {
                        release_type_state,
                        selected: ReleaseType::Stable(release.clone()),
                    })
                },
                |result| {
                    let result = result.map_err(|err| {
                        error!("failed to remove plugin: {err:?}");
                        format!("{err:?}")
                    });
                    AppMessage::PluginDetails(PluginDetailsMessage::Loaded(result))
                },
            ),
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
                    self.state = AppState::Active(AppStateActive {
                        patched: state.patched,
                        plugin: state.plugin,
                        path: state.path,
                        alter_plugin_state: Default::default(),
                    });

                    // Resize window to fit next screen
                    return resize(Size::new(EXPANDED_WINDOW_SIZE.0, EXPANDED_WINDOW_SIZE.1));
                } else {
                    self.state = AppState::Initial
                }
            }
            AppMessage::Patch(msg) => return self.update_patch(msg).map(AppMessage::Patch),
            AppMessage::Plugin(msg) => return self.update_plugin(msg).map(AppMessage::Plugin),
            AppMessage::PluginDetails(msg) => {
                return self
                    .update_plugin_details(msg)
                    .map(AppMessage::PluginDetails)
            }
        }

        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        let state: &AppStateActive = match &self.state {
            AppState::Initial => {
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
            AppState::Active(active) => active,
        };

        // Section for applying and removing the patch
        let patch_section = Self::render_patch_section(state);

        // Section for applying and removing the plugin
        let plugin_section = Self::render_plugin_section(state, &self.plugin_details_state);

        let content: Column<_> = column![patch_section, plugin_section].spacing(10);

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
