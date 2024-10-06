use crate::{
    bink::{apply_patch, is_patched, remove_patch},
    github::GitHubRelease,
    plugin::{
        apply_plugin, get_latest_beta_plugin_release, get_latest_plugin_release, remove_plugin,
    },
};
use anyhow::Context;
use iced::{
    theme::Palette,
    widget::{button, column, combo_box, container, row, scrollable, text, Button, Column, Text},
    window::{self, get_latest, icon, resize},
    Color, Length, Size, Task,
};
use log::{debug, error};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};
use tokio::task::spawn_blocking;

/// Title used for created windows
pub const WINDOW_TITLE: &str =
    concat!("Pocket Relay Plugin Installer v", env!("CARGO_PKG_VERSION"));
/// Window icon bytes
pub const ICON_BYTES: &[u8] = include_bytes!("./resources/icon.ico");

/// The window size
const WINDOW_SIZE: Size<f32> = Size::new(500.0, 140.0);
const EXPANDED_WINDOW_SIZE: Size<f32> = Size::new(500.0, 300.0);
const DARK_TEXT: Color = Color::from_rgb(0.4, 0.4, 0.4);
const SPACING: u16 = 10;

/// Initializes the user interface
///
/// ## Arguments
/// * `config` - The client config to use
/// * `client` - The HTTP client to use
pub fn init() {
    iced::application(WINDOW_TITLE, App::update, App::view)
        .window(window::Settings {
            icon: icon::from_file_data(ICON_BYTES, None).ok(),
            size: WINDOW_SIZE,
            resizable: false,

            ..window::Settings::default()
        })
        .theme(|_| iced::Theme::Dark)
        .run_with(|| (Default::default(), plugin_details_task()))
        .expect("failed to start");
}

#[derive(Default)]
struct App {
    state: AppState,

    /// Status for the remote plugin details
    plugin_details_state: PluginDetailsState,
}

pub enum AppState {
    /// Initial state, no game has been picked yet
    Initial(AppStateInitial),

    /// Active state, game has been selected and its
    /// details are known
    Active(AppStateActive),
}

impl Default for AppState {
    fn default() -> Self {
        Self::Initial(Default::default())
    }
}

#[derive(Default)]
pub struct AppStateInitial {
    /// Optionally an error that has occurred when the user is picking a file
    pick_file_error: Option<String>,
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

    /// Current status of adding/removing the patch
    alter_patch_state: AlterPatchState,
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
#[allow(clippy::large_enum_variant)]
enum AppMessage {
    /// Messages related to picking the game
    Game(GameMessage),

    /// Messages related to patching the game
    Patch(PatchMessage),

    /// Messages related to adding/removing the plugin
    Plugin(PluginMessage),

    /// Messages related to loading the plugin details
    PluginDetails(PluginDetailsMessage),
}

#[derive(Debug, Clone)]
enum GameMessage {
    /// Trigger the popup to allow the user to pick the game path
    PickGamePath,
    // Result of picking a game path
    PickedGameResult(Result<Option<GameState>, String>),
    /// Clears the active game path
    ClearGamePath,
}

#[derive(Debug, Clone)]
enum PluginDetailsMessage {
    /// Result of adding the plugin to the game
    Loaded(Result<PluginDetails, String>),
}

/// Current state for the plugin details (Remote state from github)
#[derive(Default)]
#[allow(clippy::large_enum_variant)]
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
pub enum AlterPatchState {
    /// Initial state, patch has not been added or removed yet
    #[default]
    Initial,

    /// Loading state, patch is being applied/removed
    Loading,

    /// Patch was added/removed successfully
    Success,

    /// Failed to add/remove the patch
    Error(String),
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
async fn read_game_state(exe_path: &Path) -> anyhow::Result<GameState> {
    let parent = exe_path.parent().context("missing game folder")?;
    let asi_path = parent.join("ASI");

    let plugin_path = asi_path.join("pocket-relay-plugin.asi");
    let is_patched = is_patched(parent)
        .await
        .context("failed to check game patched state")?;

    let plugin = plugin_path.exists() && plugin_path.is_file();

    Ok(GameState {
        path: parent.to_path_buf(),
        patched: is_patched,
        plugin,
    })
}

/// Obtains the plugin details for the current available releases
async fn get_plugin_details() -> anyhow::Result<PluginDetails> {
    let mut options = Vec::new();

    let release = get_latest_plugin_release().await?;
    let beta_release = get_latest_beta_plugin_release().await?;

    options.push(ReleaseType::Stable(release.clone()));
    if let Some(beta_release) = beta_release {
        options.push(ReleaseType::Beta(beta_release));
    }

    let selected = options
        .first()
        .cloned()
        .context("no release versions found")?;

    let release_type_state = combo_box::State::<ReleaseType>::new(options);

    Ok::<_, anyhow::Error>(PluginDetails {
        release_type_state,
        selected,
    })
}

/// Creates a task that will load and update the plugin details
fn plugin_details_task() -> Task<AppMessage> {
    Task::perform(get_plugin_details(), |result| {
        let result = result.map_err(|err| {
            error!("failed to load plugin details: {err:?}");
            format!("{err:?}")
        });
        AppMessage::PluginDetails(PluginDetailsMessage::Loaded(result))
    })
}

async fn pick_game_state() -> anyhow::Result<Option<GameState>> {
    // Spawn new thread for the native file picker dialog
    let path = spawn_blocking(|| {
        native_dialog::FileDialog::new()
            .add_filter("MassEffect3.exe", &["exe"])
            .set_filename("MassEffect3.exe")
            .set_title("Choose game executable")
            .show_open_single_file()
            .context("failed to pick file")
    })
    .await
    .context("failed to join native thread")?
    .context("failed to pick file")?;

    let path = match path {
        Some(path) => path,
        None => return Ok(None),
    };

    // Read the state from the chosen path
    let game_state = read_game_state(path.as_ref()).await?;
    Ok(Some(game_state))
}

impl App {
    /// View entry point for the app
    fn view(&self) -> iced::Element<'_, AppMessage> {
        match &self.state {
            AppState::Initial(state) => return self.view_initial(state),
            AppState::Active(state) => self.view_active(state),
        }
    }

    /// View for the app when its in the initial state
    fn view_initial<'a>(&'a self, state: &'a AppStateInitial) -> iced::Element<'a, AppMessage> {
        let target_text: Text = text(
            "Please click the button below to choose your game path. \
            When the file picker opens navigate to the folder containing \
            MassEffect3.exe and pick that file",
        )
        .color(DARK_TEXT);

        let pick_button: Button<_> = button("Choose game path")
            .on_press(AppMessage::Game(GameMessage::PickGamePath))
            .padding(10);

        let mut content: Column<_> = column![target_text, pick_button].spacing(10);

        if let Some(err) = &state.pick_file_error {
            content = content
                .push(text(format!("failed to pick file: {err}")).color(Palette::DARK.danger));
        }

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SPACING)
            .into()
    }

    /// View for the app when its in the active state
    fn view_active<'a>(&'a self, state: &'a AppStateActive) -> iced::Element<'a, AppMessage> {
        let back_button: Button<_> = button("Back")
            .on_press(AppMessage::Game(GameMessage::ClearGamePath))
            .padding(10);

        // Section for applying and removing the patch
        let patch_section = Self::view_patch_section(state);

        // Section for applying and removing the plugin
        let plugin_section = Self::view_plugin_section(state, &self.plugin_details_state);

        let content: Column<_> = column![back_button, patch_section, plugin_section].spacing(10);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(SPACING)
            .into()
    }

    /// View for the patch game section
    fn view_patch_section(state: &AppStateActive) -> Column<'_, AppMessage> {
        match (state.patched, &state.alter_patch_state) {
            // Patch is installed, we are in the initial state
            (true, AlterPatchState::Initial) => Self::view_patch_installed(),

            // Patch is not installed, we are in the initial state
            (false, AlterPatchState::Initial) => Self::view_patch_not_installed(),

            // Patch is installed, we are uninstalling
            (true, AlterPatchState::Loading) => Self::view_patch_uninstalling(),

            // Patch is not installed, we are installing
            (false, AlterPatchState::Loading) => Self::view_patch_installing(),

            // Patch was installed
            (true, AlterPatchState::Success) => Self::view_patch_add_success(),

            // Patch was uninstalled
            (false, AlterPatchState::Success) => Self::view_patch_remove_success(),

            // Error occurred while uninstalling
            (true, AlterPatchState::Error(err)) => Self::view_patch_uninstall_error(err),

            // Error occurred while installing
            (false, AlterPatchState::Error(err)) => Self::view_patch_install_error(err),
        }
    }

    fn view_patch_installed() -> Column<'static, AppMessage> {
        let patch_text: Text = text("Your game is patched").color(DARK_TEXT);
        let remove_patch_button: Button<_> = button("Remove Patch")
            .on_press(AppMessage::Patch(PatchMessage::Remove))
            .padding(10);

        column![patch_text, remove_patch_button].spacing(10)
    }

    fn view_patch_not_installed() -> Column<'static, AppMessage> {
        let patch_text: Text =
            text("Your game is not patched, you must apply the patch to use the client plugin.")
                .color(DARK_TEXT);
        let apply_patch_button: Button<_> = button("Apply Patch")
            .on_press(AppMessage::Patch(PatchMessage::Add))
            .padding(10);

        column![patch_text, apply_patch_button].spacing(10)
    }

    fn view_patch_installing() -> Column<'static, AppMessage> {
        let patch_text = text("Installing patch...").color(Palette::DARK.primary);
        column![patch_text].spacing(10)
    }

    fn view_patch_uninstalling() -> Column<'static, AppMessage> {
        let patch_text = text("Uninstalling patch...").color(Palette::DARK.primary);
        column![patch_text].spacing(10)
    }

    fn view_patch_remove_success() -> Column<'static, AppMessage> {
        let patch_text: Text = text("Patch successfully removed.").color(Palette::DARK.success);

        let apply_patch_button: Button<_> = button("Apply Patch")
            .on_press(AppMessage::Patch(PatchMessage::Add))
            .padding(10);

        column![patch_text, apply_patch_button].spacing(10)
    }

    fn view_patch_add_success() -> Column<'static, AppMessage> {
        let patch_text: Text = text("Patch successfully installed.").color(Palette::DARK.success);
        let remove_patch_button: Button<_> = button("Remove Patch")
            .on_press(AppMessage::Patch(PatchMessage::Remove))
            .padding(10);

        column![patch_text, remove_patch_button].spacing(10)
    }

    fn view_patch_install_error(err: &str) -> Column<'static, AppMessage> {
        let patch_text: Text =
            text(format!("failed to add patch: {err}")).color(Palette::DARK.danger);

        let retry_button: Button<_> = button("Retry")
            .on_press(AppMessage::Patch(PatchMessage::Add))
            .padding(10);
        column![patch_text, retry_button].spacing(10)
    }

    fn view_patch_uninstall_error(err: &str) -> Column<'static, AppMessage> {
        let patch_text: Text =
            text(format!("failed to remove patch: {err}")).color(Palette::DARK.danger);

        let retry_button: Button<_> = button("Retry")
            .on_press(AppMessage::Patch(PatchMessage::Remove))
            .padding(10);
        column![patch_text, retry_button].spacing(10)
    }

    /// View for the add plugin section
    fn view_plugin_section<'a>(
        state: &'a AppStateActive,
        plugin_details: &'a PluginDetailsState,
    ) -> Column<'a, AppMessage> {
        match (state.plugin, &state.alter_plugin_state) {
            // Plugin is installed, we are in the initial state
            (true, AlterPluginState::Initial) => Self::view_plugin_installed(),

            // Plugin is not installed, we are in the initial state
            (false, AlterPluginState::Initial) => Self::view_plugin_not_installed(plugin_details),

            // Plugin is installed, we are uninstalling
            (true, AlterPluginState::Loading) => Self::view_plugin_uninstalling(),

            // Plugin is not installed, we are installing
            (false, AlterPluginState::Loading) => Self::view_plugin_installing(),

            // Plugin was installed
            (true, AlterPluginState::Success) => Self::view_plugin_add_success(),

            // Plugin was uninstalled
            (false, AlterPluginState::Success) => Self::view_plugin_remove_success(plugin_details),

            // Error occurred while uninstalling
            (true, AlterPluginState::Error(err)) => Self::view_plugin_uninstall_error(err),

            // Error occurred while installing
            (false, AlterPluginState::Error(err)) => Self::view_plugin_install_error(err),
        }
    }

    fn view_plugin_installed() -> Column<'static, AppMessage> {
        let plugin_text: Text =
            text("You have the Pocket Relay client plugin installed.").color(DARK_TEXT);
        let remove_plugin_button: Button<_> = button("Remove Plugin")
            .on_press(AppMessage::Plugin(PluginMessage::Remove))
            .padding(10);

        column![plugin_text, remove_plugin_button].spacing(10)
    }

    fn view_plugin_not_installed(plugin_details: &PluginDetailsState) -> Column<'_, AppMessage> {
        let plugin_text: Text =
            text("You do not have the Pocket Relay client plugin installed").color(DARK_TEXT);
        let add_plugin = Self::view_add_plugin(plugin_details);
        column![plugin_text, add_plugin].spacing(10)
    }

    fn view_plugin_installing() -> Column<'static, AppMessage> {
        let plugin_text = text("Installing plugin...").color(Palette::DARK.primary);
        column![plugin_text].spacing(10)
    }

    fn view_plugin_uninstalling() -> Column<'static, AppMessage> {
        let plugin_text = text("Uninstalling plugin...").color(Palette::DARK.primary);
        column![plugin_text].spacing(10)
    }

    fn view_plugin_add_success() -> Column<'static, AppMessage> {
        let plugin_text: Text =
            text("Pocket Relay client plugin successfully installed.").color(Palette::DARK.success);
        let remove_plugin_button: Button<_> = button("Remove Plugin")
            .on_press(AppMessage::Plugin(PluginMessage::Remove))
            .padding(10);

        column![plugin_text, remove_plugin_button].spacing(10)
    }

    fn view_plugin_remove_success(plugin_details: &PluginDetailsState) -> Column<'_, AppMessage> {
        let plugin_text: Text =
            text("Pocket Relay client plugin successfully removed.").color(Palette::DARK.success);

        let add_plugin = Self::view_add_plugin(plugin_details);
        column![plugin_text, add_plugin].spacing(10)
    }

    fn view_plugin_install_error(err: &str) -> Column<'static, AppMessage> {
        let patch_text: Text =
            text(format!("failed to install plugin: {err}")).color(Palette::DARK.danger);

        let retry_button: Button<_> = button("Retry")
            .on_press(AppMessage::Plugin(PluginMessage::Add))
            .padding(10);
        column![patch_text, retry_button].spacing(10)
    }

    fn view_plugin_uninstall_error(err: &str) -> Column<'static, AppMessage> {
        let patch_text: Text =
            text(format!("failed to remove plugin: {err}")).color(Palette::DARK.danger);

        let retry_button: Button<_> = button("Retry")
            .on_press(AppMessage::Plugin(PluginMessage::Remove))
            .padding(10);
        column![patch_text, retry_button].spacing(10)
    }

    /// View for the add plugin details and buttons
    fn view_add_plugin(plugin_details: &PluginDetailsState) -> Column<'_, AppMessage> {
        match plugin_details {
            // Still loading the plugin details
            PluginDetailsState::Loading => {
                let plugin_version_text: Text =
                    text("Loading latest plugin version details...").color(DARK_TEXT);
                column![plugin_version_text].spacing(10)
            }
            PluginDetailsState::Error(err) => {
                let plugin_version_text: Text =
                    text(format!("Unable to load latest plugin version: {err}")).color(DARK_TEXT);
                column![plugin_version_text].spacing(10)
            }
            PluginDetailsState::Ready(plugin_details) => {
                let release = match &plugin_details.selected {
                    ReleaseType::Stable(value) => value,
                    ReleaseType::Beta(value) => value,
                };

                let version = &release.tag_name;

                let plugin_version_text: Text = text(format!(
                    "The latest version of the plugin client is {version}"
                ))
                .color(DARK_TEXT);

                let add_plugin_button: Button<_> = button("Add Plugin")
                    .on_press(AppMessage::Plugin(PluginMessage::Add))
                    .padding(10);

                let version_select = combo_box(
                    &plugin_details.release_type_state,
                    "Select version",
                    Some(&plugin_details.selected),
                    |value| AppMessage::Plugin(PluginMessage::SelectType(value)),
                )
                .padding(10);

                let add_row = row![add_plugin_button, version_select].spacing(10);
                column![plugin_version_text, add_row].spacing(10)
            }
        }
    }

    fn update(&mut self, message: AppMessage) -> Task<AppMessage> {
        match message {
            AppMessage::Game(msg) => self.update_game(msg).map(AppMessage::Game),
            AppMessage::Patch(msg) => self.update_patch(msg).map(AppMessage::Patch),
            AppMessage::Plugin(msg) => self.update_plugin(msg).map(AppMessage::Plugin),
            AppMessage::PluginDetails(msg) => self
                .update_plugin_details(msg)
                .map(AppMessage::PluginDetails),
        }
    }

    fn update_game(&mut self, msg: GameMessage) -> Task<GameMessage> {
        match msg {
            GameMessage::PickGamePath => {
                return Task::perform(pick_game_state(), |result| {
                    let result = result.map_err(|err| format!("{err:?}"));
                    GameMessage::PickedGameResult(result)
                });
            }
            GameMessage::PickedGameResult(result) => {
                match result {
                    Ok(state) => {
                        debug!("picked path: {state:?}");

                        if let Some(state) = state {
                            self.state = AppState::Active(AppStateActive {
                                patched: state.patched,
                                plugin: state.plugin,
                                path: state.path,
                                alter_plugin_state: Default::default(),
                                alter_patch_state: Default::default(),
                            });

                            // Resize window to fit next screen
                            return get_latest().and_then(|id| resize(id, EXPANDED_WINDOW_SIZE));
                        } else {
                            self.state = AppState::default()
                        }
                    }
                    Err(err) => {
                        error!("failed to pick game path: {err}");

                        if let AppState::Initial(state) = &mut self.state {
                            state.pick_file_error = Some(err);
                        }
                    }
                }
            }
            GameMessage::ClearGamePath => {
                self.state = AppState::default();

                // Resize window to fit main screen
                return get_latest().and_then(|id| resize(id, WINDOW_SIZE));
            }
        }

        Task::none()
    }

    fn update_patch(&mut self, msg: PatchMessage) -> Task<PatchMessage> {
        let state = match &mut self.state {
            AppState::Active(state) => state,
            _ => panic!("app reached invalid state, expecting 'Active' state"),
        };

        match msg {
            PatchMessage::Add => {
                state.alter_patch_state = AlterPatchState::Loading;

                return Task::perform(apply_patch(state.path.to_path_buf()), |result| {
                    let result = result.map_err(|err| format!("{err:?}"));
                    PatchMessage::Added(result)
                });
            }
            PatchMessage::Remove => {
                state.alter_patch_state = AlterPatchState::Loading;

                return Task::perform(remove_patch(state.path.to_path_buf()), |result| {
                    let result = result.map_err(|err| format!("{err:?}"));
                    PatchMessage::Removed(result)
                });
            }
            PatchMessage::Added(result) => {
                if let Err(err) = result {
                    error!("failed to apply patch: {err}");
                    state.alter_patch_state = AlterPatchState::Error(err);
                } else {
                    state.alter_patch_state = AlterPatchState::Success;
                    state.patched = true;
                }
            }
            PatchMessage::Removed(result) => {
                if let Err(err) = result {
                    error!("failed to remove patch: {err}");
                    state.alter_patch_state = AlterPatchState::Error(err);
                } else {
                    state.alter_patch_state = AlterPatchState::Success;
                    state.patched = false;
                }
            }
        }

        Task::none()
    }

    fn update_plugin(&mut self, msg: PluginMessage) -> Task<PluginMessage> {
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

                return Task::perform(apply_plugin(path, release), |result| {
                    let result = result.map_err(|err| format!("{err:?}"));
                    PluginMessage::Added(result)
                });
            }
            PluginMessage::Remove => {
                let path = state.path.to_path_buf();

                state.alter_plugin_state = AlterPluginState::Loading;

                return Task::perform(remove_plugin(path), |result| {
                    let result = result.map_err(|err| format!("{err:?}"));
                    PluginMessage::Removed(result)
                });
            }
            PluginMessage::Added(result) => {
                if let Err(err) = result {
                    error!("failed to add plugin: {err:?}");
                    state.alter_plugin_state = AlterPluginState::Error(err);
                } else {
                    state.alter_plugin_state = AlterPluginState::Success;
                    state.plugin = true;
                }
            }
            PluginMessage::Removed(result) => {
                if let Err(err) = result {
                    error!("failed to remove plugin: {err:?}");
                    state.alter_plugin_state = AlterPluginState::Error(err);
                } else {
                    state.alter_plugin_state = AlterPluginState::Success;
                    state.plugin = false;
                }
            }
            PluginMessage::SelectType(release_type) => {
                if let PluginDetailsState::Ready(plugin_details) = &mut self.plugin_details_state {
                    plugin_details.selected = release_type;
                }
            }
        }

        Task::none()
    }

    fn update_plugin_details(&mut self, msg: PluginDetailsMessage) -> Task<PluginDetailsMessage> {
        match msg {
            PluginDetailsMessage::Loaded(result) => {
                self.plugin_details_state = match result {
                    Ok(value) => PluginDetailsState::Ready(value),
                    Err(err) => PluginDetailsState::Error(err),
                }
            }
        }

        Task::none()
    }
}
