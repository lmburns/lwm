//! Configuration options

use crate::{
    core::{AutomaticScheme, ChildPolarity, PointerAction, StateTransition, Tightness},
    geometry::Padding,
    utils::{deserialize_absolute_path, deserialize_shellexpand},
    x::input::{Button, ModMask},
};
use anyhow::{Context, Result};
use colored::Colorize;
use directories::{BaseDirs, ProjectDirs, UserDirs};
use format_serde_error::SerdeError;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use serde::{de, Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::HashMap,
    env,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
};
use which::which;

/// Configuration file name
const CONFIG_FILE: &str = "lwm.yml";

/// Default shell to run commands within
pub(crate) static SHELL: Lazy<PathBuf> = Lazy::new(|| {
    PathBuf::from(env::var("LWM_SHELL").unwrap_or_else(|_| {
        env::var("SHELL").unwrap_or_else(|_| {
            if let Ok(bash) = which("bash") {
                bash.to_string_lossy().to_string()
            } else if let Ok(dash) = which("dash") {
                dash.to_string_lossy().to_string()
            } else {
                String::from("/bin/sh")
            }
        })
    }))
});

// pub(crate) static SHELL: Lazy<String> =
//     Lazy::new(|| env::var("SHELL").unwrap_or_else(|_|
// String::from("/bin/bash")));

// =============== GlobalSettings ================= [[[

/// Global configuration settings
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct GlobalSettings {
    /// The shell to use for running commands
    #[serde(deserialize_with = "deserialize_absolute_path")]
    pub(crate) shell: Option<PathBuf>,

    /// The file to write the PID to
    #[serde(alias = "pid-file", deserialize_with = "deserialize_shellexpand")]
    pub(crate) pid_file: Option<PathBuf>,

    /// Whether logs should be written to a file
    #[serde(alias = "log-to-file")]
    pub(crate) log_to_file: bool,

    /// The directory to write the log to
    #[serde(alias = "log-dir", deserialize_with = "deserialize_shellexpand")]
    pub(crate) log_dir: Option<PathBuf>,

    /// The delay in which keys begin to repeat
    #[serde(alias = "autorepeat-delay")]
    pub(crate) autorepeat_delay: Option<u16>,

    /// The speed in which keys repeat after the delay
    #[serde(alias = "autorepeat-interval")]
    pub(crate) autorepeat_interval: Option<u16>,

    // ====================== Window Manager Specific ======================
    /// Name of the desktops
    pub(crate) desktops: Vec<String>,

    // NOTE: Default: ""
    /// Absolute path to the command used to retrieve rule consequences
    #[serde(alias = "external-rules-cmd")]
    pub(crate) external_rules_cmd: Option<String>,

    /// Prefix prepended to each of the status lines
    #[serde(alias = "status-prefix")]
    pub(crate) status_prefix: String,

    /// Color of the border of an unfocused window
    #[serde(alias = "normal-border-color")]
    pub(crate) normal_border_color: String,

    /// Color of the border of a focused window of an unfocused monitor
    #[serde(alias = "active-border-color")]
    pub(crate) active_border_color: String,

    /// Color of the border of a focused window of a focused monitor
    #[serde(alias = "focused-border-color")]
    pub(crate) focused_border_color: String,

    /// Color of the area when preselection takes place
    #[serde(alias = "presel-feedback-color")]
    pub(crate) presel_feedback_color: String,

    /// Top, bottom, left, right padding of windows
    pub(crate) padding: Padding,

    /// Top, bottom, left, right padding of windows in monocle mode
    #[serde(alias = "monocle-padding")]
    pub(crate) monocle_padding: Padding,

    /// Gap between active windows
    #[serde(alias = "window-gap")]
    pub(crate) window_gap: usize,

    /// Size of the border around the window
    #[serde(alias = "border-width")]
    pub(crate) border_width: u32,

    /// Ratio of window splits
    #[serde(alias = "split-ratio")]
    pub(crate) split_ratio: f32,

    /// Window that child is attached to when adding in automatic mode
    #[serde(alias = "initial-polarity")]
    pub(crate) initial_polarity: Option<ChildPolarity>,

    /// Insertion scheme used when the insertion point is in automatic mode
    #[serde(alias = "automatic-scheme")]
    pub(crate) automatic_scheme: AutomaticScheme,

    /// Adjust brother when unlinking node from tree in accordance with
    /// `automatic` scheme
    #[serde(alias = "removal-adjustment")]
    pub(crate) removal_adjustment: bool,

    /// [`Tightnesss`] of the algorithm used
    #[serde(alias = "directional-focus-tightness")]
    pub(crate) directional_focus_tightness: Tightness,

    /// Keyboard modifier used for moving or resizing windows
    #[serde(alias = "pointer-modifier")]
    pub(crate) pointer_modifier: ModMask,

    /// Minimum interval between two motion notify events (milliseconds)
    #[serde(alias = "pointer-motion-interval")]
    pub(crate) pointer_motion_interval: u32,

    /// Action performed when pressing [`ModMask`] + [`Button`]
    #[serde(alias = "pointer-actions")]
    pub(crate) pointer_actions: Option<PointerActions>,

    /// Handle next `mapping_events_count` mapping notify events
    /// A negative value implies that every event needs to be handled
    #[serde(alias = "mapping-events-count")]
    pub(crate) mapping_events_count: i8,

    /// Draw the preselection feedback area
    #[serde(alias = "presel-feedback")]
    pub(crate) presel_feedback: bool,

    /// Remove borders of tiled windows (monocle desktop layout)
    #[serde(alias = "borderless-monocle")]
    pub(crate) borderless_monocle: bool,

    /// Remove gaps of tiled windows (monocle desktop layout)
    #[serde(alias = "gapless-monocle")]
    pub(crate) gapless_monocle: bool,

    /// Set desktop layout to monocle if there’s only one tiled window in tree
    #[serde(alias = "single-monocle")]
    pub(crate) single_monocle: bool,

    /// XXX: Not in configuration
    #[serde(alias = "borderless-singleton")]
    pub(crate) borderless_singleton: bool,

    /// Focus the window under the pointer
    #[serde(alias = "focus_follows_pointer")]
    pub(crate) focus_follows_pointer: bool,

    /// When focusing a window, put the pointer at its center
    #[serde(alias = "pointer-follows-focus")]
    pub(crate) pointer_follows_focus: bool,

    /// When focusing a monitor, put the pointer at its center
    #[serde(alias = "pointer-follows-monitor")]
    pub(crate) pointer_follows_monitor: bool,

    /// Button used for focusing a window (or a monitor)
    #[serde(alias = "click-to-focus")]
    pub(crate) click_to_focus: Button,

    /// Don’t replay the click that makes a window focused if `click_to_focus`
    /// isn’t none
    #[serde(alias = "swallow-first-click")]
    pub(crate) swallow_first_click: bool,

    /// Ignore EWMH focus requests coming from applications
    #[serde(alias = "ignore-ewmh-focus")]
    pub(crate) ignore_ewmh_focus: bool,

    /// Ignore strut hinting from clients requesting to reserve space
    #[serde(alias = "ignore-ewmh-struts")]
    pub(crate) ignore_ewmh_struts: bool,

    /// Block the fullscreen state transitions that originate from an EWMH
    /// request
    #[serde(alias = "ignore-ewmh-fullscreen")]
    pub(crate) ignore_ewmh_fullscreen: StateTransition,

    /// Center pseudo tiled windows into their tiling rectangles
    #[serde(alias = "center-pseudotiled")]
    pub(crate) center_pseudotiled: bool,

    /// Apply ICCCM window size hints
    #[serde(alias = "honor-size-hints")]
    pub(crate) honor_size_hints: bool,

    /// Consider disabled monitors as disconnected
    #[serde(alias = "remove-disabled-monitors")]
    pub(crate) remove_disabled_monitors: bool,

    /// Remove unplugged monitors
    #[serde(alias = "remove-unplugged-monitors")]
    pub(crate) remove_unplugged_monitors: bool,

    /// Merge overlapping monitors (the bigger remains)
    #[serde(alias = "remove-unplugged-monitors")]
    pub(crate) merge_overlapping_monitors: bool,
} // ]]] === Global Settings ===

// =============== Pointer Actions ================ [[[

/// Three [`PointerAction`]s
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub(crate) struct PointerActions {
    /// Action (1) performed when pressing `pointer_modifier` + [`Button`]
    #[serde(alias = "pointer-action1")]
    pub(crate) pointer_action1: Option<PointerAction>,

    /// Action (2) performed when pressing `pointer_modifier` + [`Button`]
    #[serde(alias = "pointer-action2")]
    pub(crate) pointer_action2: Option<PointerAction>,

    /// Action (3) performed when pressing `pointer_modifier` + [`Button`]
    #[serde(alias = "pointer-action3")]
    pub(crate) pointer_action3: Option<PointerAction>,
} // ]]] === Pointer Actions ===

impl PointerActions {
    /// Create a new [`PointerActions`]
    pub(crate) const fn new(a1: PointerAction, a2: PointerAction, a3: PointerAction) -> Self {
        Self {
            pointer_action1: Some(a1),
            pointer_action2: Some(a2),
            pointer_action3: Some(a3),
        }
    }
}

// =================== Config ===================== [[[

/// Configuration file to parse.
///
/// accident, the first one will be the one that is used
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct Config {
    /// Global settings
    #[serde(flatten)]
    pub(crate) global: GlobalSettings,

    /// The mappings of keys to shell commands
    pub(crate) bindings: Option<IndexMap<String, String>>,
}

impl Config {
    /// Create the default configuration file
    pub(crate) fn create_default<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            log::debug!("Creating configuration path: {}", path.display());
            fs::create_dir_all(path).context("unable to create configuration directory")?;
        }

        let path = path.join(CONFIG_FILE);
        log::debug!("{}: {}", "Configuration path".bright_blue(), path.display());

        if !path.is_file() {
            let initialization = include_str!("../example/lwm.yml");

            let mut config_file: fs::File = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&path)
                .with_context(|| format!("could not create lwm config: '{}'", path.display()))?;

            config_file
                .write_all(initialization.as_bytes())
                .with_context(|| format!("could not create lwm config: '{}'", path.display()))?;
            config_file.flush()?;
        }

        Self::load(path)
    }

    // NOTE: SerdeError doesn't always point out correct error

    /// Load the configuration file from a given path
    pub(crate) fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // let file = fs::read(&path).context("failed to read config file")?;
        // serde_yaml::from_slice(&file).context("failed to deserialize config file")

        let file = fs::read_to_string(&path).context("failed to read config file")?;
        let res = serde_yaml::from_str(&file).map_err(|e| SerdeError::new(file, e))?;

        Ok(res)
    }

    /// Load the default configuration file
    pub(crate) fn load_default() -> Result<Self> {
        let path = PROJECT_DIRS.config_dir();
        log::debug!("loading default config: {}", path.display());
        Self::create_default(path)
    }
} // ]]] === Config ===

// ================ Project Dirs ================== [[[

/// Get the base [`LwmDirs`]
pub(crate) static PROJECT_DIRS: Lazy<LwmDirs> =
    Lazy::new(|| LwmDirs::new().expect("failed to get `LwmDirs`"));

/// Get the project directories relevant to [`lwm`]
#[derive(Debug, Clone)]
pub(crate) struct LwmDirs {
    /// User's `$HOME` directory
    home_dir:   PathBuf,
    /// User's `$XDG_CACHE_HOME/lwm` directory
    cache_dir:  PathBuf,
    /// User's `$XDG_CONFIG_HOME/lwm` directory
    config_dir: PathBuf,
    /// User's `$XDG_DATA_HOME/lwm` directory
    data_dir:   PathBuf,
}

impl LwmDirs {
    /// Create a new [`LwmDirs`]
    fn new() -> Option<Self> {
        Some(Self {
            home_dir:   Self::get_home_dir()?,
            cache_dir:  Self::get_cache_dir()?,
            config_dir: Self::get_config_dir()?,
            data_dir:   Self::get_data_dir()?,
        })
    }

    /// Wrapper function that makes it easier to get directories
    fn get_dir(env_var: &str, var: &str, join: &str, dirf: &Path) -> Option<PathBuf> {
        env::var_os(env_var).map(PathBuf::from).map_or_else(
            || {
                env::var_os(var)
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .or_else(|| BaseDirs::new().map(|p| p.home_dir().join(join)))
                    .map(|p| p.join(env!("CARGO_PKG_NAME")))
            },
            |v| {
                // Custom env var is set
                if v.is_absolute() {
                    Some(v)
                } else {
                    BaseDirs::new()
                        .map(|p| p.home_dir().join(join))
                        .map(|p| p.join(env!("CARGO_PKG_NAME")))
                }
            },
        )
    }

    /// Get the `home` directory
    fn get_home_dir() -> Option<PathBuf> {
        BaseDirs::new().map(|p| p.home_dir().to_path_buf())
    }

    // ================== Config Dirs ===================== [[[

    /// Get the `cache` directory
    fn get_cache_dir() -> Option<PathBuf> {
        Self::get_dir(
            "LWM_CACHE_DIR",
            "XDG_CACHE_HOME",
            ".cache",
            get_project_dirs().cache_dir(),
        )
    }

    /// Get the `config` directory
    fn get_config_dir() -> Option<PathBuf> {
        Self::get_dir(
            "LWM_CONFIG_DIR",
            "XDG_CONFIG_HOME",
            ".config",
            get_project_dirs().config_dir(),
        )
    }

    /// Get the `data` directory
    fn get_data_dir() -> Option<PathBuf> {
        Self::get_dir(
            "LWM_DATA_DIR",
            "XDG_DATA_HOME",
            ".local/share",
            get_project_dirs().data_dir(),
        )
    }

    // ]]] === Config Dirs ===

    // ================== Public Funcs ==================== [[[

    /// Get cache directory
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get configuration directory
    #[must_use]
    pub(crate) fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Get local data directory
    #[must_use]
    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Get cache directory
    #[must_use]
    pub(crate) fn home_dir(&self) -> &Path {
        &self.home_dir
    }
    // ]]] === Public Funcs ===
}

/// Get all user project directories
pub(crate) fn get_project_dirs() -> ProjectDirs {
    log::trace!("determining project default folders");
    ProjectDirs::from("com", "lmburns", "lwm")
        .expect("could not detect user home directory to place program files")
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            shell:               Some(SHELL.to_path_buf()),
            pid_file:            None,
            log_to_file:         true,
            log_dir:             None,
            autorepeat_delay:    None,
            autorepeat_interval: None,

            desktops:              vec!["1", "2", "3", "4", "5"]
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            external_rules_cmd:    None,
            status_prefix:         String::from("W"),
            normal_border_color:   String::from("#4C566A"),
            active_border_color:   String::from("#1E1E1E"),
            focused_border_color:  String::from("#A98698"),
            presel_feedback_color: String::from("#4C96A8"),

            padding:                     Padding::new(0, 0, 0, 0),
            monocle_padding:             Padding::new(0, 0, 0, 0),
            window_gap:                  6_usize,
            border_width:                1_u32,
            split_ratio:                 0.5_f32,
            initial_polarity:            None,
            automatic_scheme:            AutomaticScheme::LongestSide,
            removal_adjustment:          true,
            directional_focus_tightness: Tightness::High,

            pointer_modifier:        ModMask::Mod4,
            pointer_motion_interval: 17_u32,
            pointer_actions:         Some(PointerActions::new(
                PointerAction::Move,
                PointerAction::ResizeSide,
                PointerAction::ResizeCorner,
            )),
            mapping_events_count:    1_i8,

            presel_feedback:      true,
            borderless_monocle:   false,
            gapless_monocle:      false,
            single_monocle:       false,
            borderless_singleton: false,

            focus_follows_pointer:   false,
            pointer_follows_focus:   false,
            pointer_follows_monitor: false,
            click_to_focus:          Button::Left,
            swallow_first_click:     false,
            ignore_ewmh_focus:       false,
            ignore_ewmh_struts:      false,
            ignore_ewmh_fullscreen:  StateTransition::Enter,

            center_pseudotiled: true,
            honor_size_hints:   false,

            remove_disabled_monitors:   false,
            remove_unplugged_monitors:  false,
            merge_overlapping_monitors: false,
        }
    }
}

// NOTE: Does a custom implementation of `Clone` do anything?
impl Clone for GlobalSettings {
    fn clone(&self) -> Self {
        Self {
            shell: self.shell.clone(),
            pid_file: self.pid_file.clone(),
            log_to_file: self.log_to_file,
            log_dir: self.log_dir.clone(),
            autorepeat_delay: self.autorepeat_delay,
            autorepeat_interval: self.autorepeat_interval,
            desktops: self.desktops.clone(),
            external_rules_cmd: self.external_rules_cmd.clone(),
            status_prefix: self.status_prefix.clone(),
            normal_border_color: self.normal_border_color.clone(),
            active_border_color: self.active_border_color.clone(),
            focused_border_color: self.focused_border_color.clone(),
            presel_feedback_color: self.presel_feedback_color.clone(),
            padding: self.padding,
            monocle_padding: self.monocle_padding,
            window_gap: self.window_gap,
            border_width: self.border_width,
            split_ratio: self.split_ratio,
            initial_polarity: self.initial_polarity,
            automatic_scheme: self.automatic_scheme,
            removal_adjustment: self.removal_adjustment,
            directional_focus_tightness: self.directional_focus_tightness,
            pointer_modifier: self.pointer_modifier,
            pointer_motion_interval: self.pointer_motion_interval,
            pointer_actions: self.pointer_actions.clone(),
            mapping_events_count: self.mapping_events_count,
            presel_feedback: self.presel_feedback,
            borderless_monocle: self.borderless_monocle,
            gapless_monocle: self.gapless_monocle,
            single_monocle: self.single_monocle,
            borderless_singleton: self.borderless_singleton,
            focus_follows_pointer: self.focus_follows_pointer,
            pointer_follows_focus: self.pointer_follows_focus,
            pointer_follows_monitor: self.pointer_follows_monitor,
            click_to_focus: self.click_to_focus,
            swallow_first_click: self.swallow_first_click,
            ignore_ewmh_focus: self.ignore_ewmh_focus,
            ignore_ewmh_struts: self.ignore_ewmh_struts,
            ignore_ewmh_fullscreen: self.ignore_ewmh_fullscreen,
            center_pseudotiled: self.center_pseudotiled,
            honor_size_hints: self.honor_size_hints,
            remove_disabled_monitors: self.remove_disabled_monitors,
            remove_unplugged_monitors: self.remove_unplugged_monitors,
            merge_overlapping_monitors: self.merge_overlapping_monitors,
        }
    }
}
