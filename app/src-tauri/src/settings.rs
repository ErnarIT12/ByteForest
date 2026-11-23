//! Small persistent user settings stored as JSON in `%APPDATA%\ByteForest\settings.json`.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// User settings. Missing fields fall back to their defaults.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    /// Restart as administrator at every start.
    pub always_elevated: bool,
    /// Names, wildcards and absolute paths left out of every scan; see
    /// [`bf_core::ExclusionSet::from_patterns`] for the syntax.
    pub exclusions: Vec<String>,
    /// How many automatic snapshots to keep (`0` turns them off).
    pub auto_snapshot_keep: u8,
}

/// Automatic snapshots kept unless the user changes it.
pub const DEFAULT_AUTO_SNAPSHOT_KEEP: u8 = 10;
/// Most automatic snapshots that can be kept.
pub const MAX_AUTO_SNAPSHOT_KEEP: u8 = 50;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            always_elevated: false,
            exclusions: Vec::new(),
            auto_snapshot_keep: DEFAULT_AUTO_SNAPSHOT_KEEP,
        }
    }
}

impl Settings {
    /// The settings with `exclusions` trimmed, deduplicated and capped and the snapshot
    /// count limited to `0..=50`.
    pub fn sanitized(mut self) -> Self {
        self.exclusions = bf_core::normalize_patterns(&self.exclusions);
        self.auto_snapshot_keep = self.auto_snapshot_keep.min(MAX_AUTO_SNAPSHOT_KEEP);
        self
    }
}

/// Reads settings from `path`; a missing or corrupt file gives the defaults.
pub fn load(path: &Path) -> Settings {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                tracing::warn!("cannot read settings {}: {e}", path.display());
            }
            return Settings::default();
        }
    };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        tracing::warn!("corrupt settings {}, using defaults: {e}", path.display());
        Settings::default()
    })
}

/// Writes settings to `path`, creating the parent folder if needed. The data goes to a
/// temporary file first and is renamed over `path`, so a crash cannot leave a half-written file.
pub fn save(path: &Path, s: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(s).map_err(io::Error::other)?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// `%APPDATA%\ByteForest\settings.json`, or `None` when `APPDATA` is unset or empty.
/// There is deliberately no fallback to the current folder: an elevated process often
/// starts in `C:\Windows\System32`.
pub fn settings_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA");
    #[cfg(target_os = "macos")]
    let base = std::env::home_dir().map(|p| p.join("Library/Application Support").into_os_string());
    path_in(base)
}

/// Settings file below the given `APPDATA` value.
fn path_in(appdata: Option<std::ffi::OsString>) -> Option<PathBuf> {
    let base = PathBuf::from(appdata.filter(|v| !v.is_empty())?);
    Some(base.join("ByteForest").join("settings.json"))
}

/// Loads the settings from the default path, or the defaults when there is none.
pub fn load_default_path() -> Settings {
    settings_path().map_or_else(Settings::default, |p| load(&p))
}

/// Saves the settings to the default path. Fails when `APPDATA` is not set.
pub fn save_default_path(s: &Settings) -> io::Result<()> {
    match settings_path() {
        Some(p) => save(&p, s),
        None => {
            tracing::warn!("the user settings folder is not available");
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the user settings folder is not available",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Temp folder under `BF_TEST_TMP` when set, else the system temp folder.
    fn temp_dir() -> tempfile::TempDir {
        match std::env::var_os("BF_TEST_TMP") {
            Some(base) => tempfile::tempdir_in(base).unwrap(),
            None => tempfile::tempdir().unwrap(),
        }
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = temp_dir();
        assert_eq!(load(&dir.path().join("nope.json")), Settings::default());
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{garbage").unwrap();
        assert_eq!(load(&path), Settings::default());
    }

    #[test]
    fn empty_object_uses_field_defaults() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(load(&path), Settings::default());
    }

    #[test]
    #[cfg(windows)]
    fn no_appdata_means_no_settings_path() {
        assert_eq!(path_in(None), None);
        assert_eq!(path_in(Some(std::ffi::OsString::new())), None);
        assert_eq!(
            path_in(Some(r"C:\Users\x\AppData\Roaming".into())),
            Some(PathBuf::from(
                r"C:\Users\x\AppData\Roaming\ByteForest\settings.json"
            ))
        );
    }

    #[test]
    fn old_file_without_exclusions_loads_with_none() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"always_elevated": true}"#).unwrap();
        let s = load(&path);
        assert!(s.always_elevated);
        assert!(s.exclusions.is_empty());
    }

    #[test]
    fn sanitized_trims_dedupes_and_caps_exclusions() {
        let s = Settings {
            always_elevated: false,
            exclusions: vec![" a ".into(), "a".into(), String::new(), "x".repeat(300)],
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.exclusions, vec!["a".to_string()]);
        let many = Settings {
            always_elevated: false,
            exclusions: (0..400).map(|i| format!("p{i}")).collect(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(many.exclusions.len(), 200);
    }

    #[test]
    fn save_then_load_round_trips_and_creates_folder() {
        let dir = temp_dir();
        let path = dir.path().join("sub").join("settings.json");
        let s = Settings {
            always_elevated: true,
            exclusions: vec!["node_modules".to_string(), r"C:\Windows\WinSxS".to_string()],
            auto_snapshot_keep: 20,
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"always_elevated\""), "{text}");
    }

    #[test]
    fn auto_snapshot_keep_defaults_to_ten_and_is_clamped() {
        let dir = temp_dir();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"always_elevated": true}"#).unwrap();
        assert_eq!(load(&path).auto_snapshot_keep, 10);
        std::fs::write(&path, "{}").unwrap();
        assert_eq!(load(&path).auto_snapshot_keep, 10);
        std::fs::write(&path, r#"{"auto_snapshot_keep": 0}"#).unwrap();
        assert_eq!(load(&path).auto_snapshot_keep, 0);
        let big = Settings {
            auto_snapshot_keep: 200,
            ..Settings::default()
        };
        assert_eq!(big.sanitized().auto_snapshot_keep, 50);
    }
}
