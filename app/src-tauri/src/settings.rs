//! Small persistent user settings stored as JSON in `%APPDATA%\ByteForest\settings.json`.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// User settings. Missing fields fall back to their defaults.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    /// Restart as administrator at every start.
    pub always_elevated: bool,
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
    fn save_then_load_round_trips_and_creates_folder() {
        let dir = temp_dir();
        let path = dir.path().join("sub").join("settings.json");
        let s = Settings {
            always_elevated: true,
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"always_elevated\""), "{text}");
    }
}
