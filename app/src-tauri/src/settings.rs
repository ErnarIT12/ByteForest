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
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes settings to `path`, creating the parent folder if needed.
pub fn save(path: &Path, s: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(s).map_err(io::Error::other)?;
    std::fs::write(path, text)
}

/// `%APPDATA%\ByteForest\settings.json` (relative to the current folder if `APPDATA` is unset).
pub fn settings_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map_or_else(PathBuf::new, PathBuf::from);
    base.join("ByteForest").join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Temp folder under `D:\Temp` when it exists, else the system temp folder.
    fn temp_dir() -> tempfile::TempDir {
        let preferred = Path::new(r"D:\Temp");
        if preferred.is_dir() {
            tempfile::tempdir_in(preferred).unwrap()
        } else {
            tempfile::tempdir().unwrap()
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
