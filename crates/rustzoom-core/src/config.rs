//! User settings and their persistence as TOML.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::zoom::ZoomMode;

pub const DEFAULT_FACTOR: f32 = 3.0;
pub const MIN_FACTOR: f32 = 1.5;
pub const MAX_FACTOR: f32 = 10.0;
pub const MAX_LEVELS: usize = 5;
pub const MIN_DIAMETER: f32 = 0.1;
pub const MAX_DIAMETER: f32 = 0.9;
pub const MAX_BORDER: f32 = 10.0;

const DEFAULT_HOTKEY: Hotkey = Hotkey::Mouse(MouseButton::X1);
const DEFAULT_DIAMETER: f32 = 0.4;
const DEFAULT_BORDER: f32 = 2.0;

/// Mouse buttons that can be bound as hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    /// Never bindable (the GUI needs it), but reported by input.
    Left,
    Right,
    Middle,
    /// "Mouse 4", usually the rear side button.
    X1,
    /// "Mouse 5", usually the front side button.
    X2,
}

/// A bindable input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hotkey {
    /// Windows virtual-key code (`VK_*`).
    Key(u16),
    Mouse(MouseButton),
}

impl Hotkey {
    /// Whether this input may be used as a hotkey at all.
    pub fn is_bindable(self) -> bool {
        self != Self::Mouse(MouseButton::Left)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "de")]
    German,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Zoom on/off. When off the hotkey does nothing.
    pub enabled: bool,
    /// UI language; `None` follows the Windows UI language.
    pub language: Option<Language>,
    pub hotkey: Hotkey,
    pub mode: ZoomMode,
    /// Selectable magnification factors, the first one is active at start.
    pub levels: Vec<f32>,
    /// Switches to the next level.
    pub cycle_hotkey: Option<Hotkey>,
    /// Scope diameter relative to the monitor height.
    pub scope_diameter: f32,
    /// Width of the scope border in pixels.
    pub border_width: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            language: None,
            hotkey: DEFAULT_HOTKEY,
            mode: ZoomMode::Hold,
            levels: vec![DEFAULT_FACTOR],
            cycle_hotkey: None,
            scope_diameter: DEFAULT_DIAMETER,
            border_width: DEFAULT_BORDER,
        }
    }
}

impl Config {
    /// Brings every value into its valid range, so the rest of the app never sees nonsense,
    /// whatever was in the file.
    pub fn sanitized(mut self) -> Self {
        self.levels.retain(|f| f.is_finite());
        self.levels.truncate(MAX_LEVELS);
        for level in &mut self.levels {
            *level = level.clamp(MIN_FACTOR, MAX_FACTOR);
        }
        if self.levels.is_empty() {
            self.levels = vec![DEFAULT_FACTOR];
        }
        self.scope_diameter =
            finite_or(self.scope_diameter, DEFAULT_DIAMETER).clamp(MIN_DIAMETER, MAX_DIAMETER);
        self.border_width = finite_or(self.border_width, DEFAULT_BORDER).clamp(0.0, MAX_BORDER);
        if !self.hotkey.is_bindable() {
            self.hotkey = DEFAULT_HOTKEY;
        }
        if self
            .cycle_hotkey
            .is_some_and(|key| !key.is_bindable() || key == self.hotkey)
        {
            self.cycle_hotkey = None;
        }
        self
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

/// Result of [`load`]: always a usable config, plus a message if the file had problems.
#[derive(Debug)]
pub struct Loaded {
    pub config: Config,
    pub warning: Option<String>,
}

/// Reads the config from `path`. A missing file gives the defaults. A file that can't be parsed is
/// moved to `<path>.bak` (so the user's edits aren't lost) and the defaults are used.
pub fn load(path: &Path) -> Loaded {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Loaded {
                config: Config::default(),
                warning: None,
            };
        }
        Err(err) => {
            return Loaded {
                config: Config::default(),
                warning: Some(format!("cannot read {}: {err}", path.display())),
            };
        }
    };

    match toml::from_str::<Config>(&text) {
        Ok(config) => Loaded {
            config: config.sanitized(),
            warning: None,
        },
        Err(err) => {
            let backup = sibling(path, "bak");
            let moved = fs::rename(path, &backup)
                .map(|()| format!(", moved to {}", backup.display()))
                .unwrap_or_default();
            Loaded {
                config: Config::default(),
                warning: Some(format!("invalid {}{moved}: {err}", path.display())),
            }
        }
    }
}

/// Writes the config atomically (temp file + rename), creating the directory if needed.
pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(config).map_err(io::Error::other)?;
    let temp = sibling(path, "tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, path)
}

/// `config.toml` → `config.toml.<suffix>`.
fn sibling(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom() -> Config {
        Config {
            enabled: false,
            language: Some(Language::German),
            hotkey: Hotkey::Key(0x12),
            mode: ZoomMode::Toggle,
            levels: vec![2.0, 4.0, 8.0],
            cycle_hotkey: Some(Hotkey::Mouse(MouseButton::X2)),
            scope_diameter: 0.6,
            border_width: 0.0,
        }
    }

    #[test]
    fn defaults_match_mvp() {
        let config = Config::default();
        assert!(config.enabled);
        assert_eq!(config.language, None);
        assert_eq!(config.hotkey, Hotkey::Mouse(MouseButton::X1));
        assert_eq!(config.mode, ZoomMode::Hold);
        assert_eq!(config.levels, vec![3.0]);
        assert_eq!(config.cycle_hotkey, None);
        assert_eq!(config.scope_diameter, 0.4);
        assert_eq!(config.border_width, 2.0);
    }

    #[test]
    fn defaults_are_already_sane() {
        assert_eq!(Config::default().sanitized(), Config::default());
        assert_eq!(custom().sanitized(), custom());
    }

    #[test]
    fn toml_roundtrip() {
        for config in [Config::default(), custom()] {
            let text = toml::to_string_pretty(&config).unwrap();
            assert_eq!(toml::from_str::<Config>(&text).unwrap(), config, "{text}");
        }
    }

    #[test]
    fn toml_format_is_readable() {
        let config: Config = toml::from_str(
            r#"
            hotkey = { key = 18 }
            cycle_hotkey = { mouse = "x2" }
            mode = "toggle"
            language = "de"
            "#,
        )
        .unwrap();
        assert_eq!(config.hotkey, Hotkey::Key(18));
        assert_eq!(config.cycle_hotkey, Some(Hotkey::Mouse(MouseButton::X2)));
        assert_eq!(config.mode, ZoomMode::Toggle);
        assert_eq!(config.language, Some(Language::German));
    }

    #[test]
    fn missing_fields_use_defaults() {
        let config: Config = toml::from_str("scope_diameter = 0.6").unwrap();
        assert_eq!(
            config,
            Config {
                scope_diameter: 0.6,
                ..Config::default()
            }
        );
    }

    #[test]
    fn sanitized_clamps_everything() {
        let config = Config {
            levels: vec![0.5, f32::NAN, 2.0, 50.0, 3.0, 4.0, 5.0, 6.0],
            scope_diameter: 2.0,
            border_width: -1.0,
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.levels, vec![1.5, 2.0, 10.0, 3.0, 4.0]);
        assert_eq!(config.scope_diameter, MAX_DIAMETER);
        assert_eq!(config.border_width, 0.0);

        let config = Config {
            levels: vec![f32::INFINITY],
            scope_diameter: f32::NAN,
            border_width: f32::NAN,
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.levels, vec![DEFAULT_FACTOR]);
        assert_eq!(config.scope_diameter, DEFAULT_DIAMETER);
        assert_eq!(config.border_width, DEFAULT_BORDER);

        let config = Config {
            levels: Vec::new(),
            scope_diameter: 0.0,
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.levels, vec![DEFAULT_FACTOR]);
        assert_eq!(config.scope_diameter, MIN_DIAMETER);
    }

    #[test]
    fn sanitized_rejects_bad_hotkeys() {
        let left = Hotkey::Mouse(MouseButton::Left);
        let config = Config {
            hotkey: left,
            cycle_hotkey: Some(left),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.hotkey, DEFAULT_HOTKEY);
        assert_eq!(config.cycle_hotkey, None);

        let config = Config {
            cycle_hotkey: Some(DEFAULT_HOTKEY),
            ..Config::default()
        }
        .sanitized();
        assert_eq!(config.cycle_hotkey, None, "cycle key equal to zoom key");
    }

    #[test]
    fn load_missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load(&dir.path().join("config.toml"));
        assert_eq!(loaded.config, Config::default());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn load_garbage_gives_defaults_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "this is = = not toml").unwrap();

        let loaded = load(&path);
        assert_eq!(loaded.config, Config::default());
        assert!(loaded.warning.is_some());
        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("config.toml.bak")).unwrap(),
            "this is = = not toml"
        );
    }

    #[test]
    fn load_sanitizes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "scope_diameter = 5.0").unwrap();
        assert_eq!(load(&path).config.scope_diameter, MAX_DIAMETER);
    }

    #[test]
    fn save_then_load_roundtrip_without_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("config.toml");
        save(&path, &custom()).unwrap();
        save(&path, &custom()).unwrap(); // overwriting works too

        let loaded = load(&path);
        assert_eq!(loaded.config, custom());
        assert!(loaded.warning.is_none());
        let files: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(files, vec!["config.toml"]);
    }
}
