//! Turning the `<target>` argument into a [`ScanRoot`].

use std::path::Path;

use bf_scan::ScanRoot;

/// Parses `C`, `c:`, `C:\` or an existing folder path.
pub fn parse_target(arg: &str) -> Result<ScanRoot, String> {
    if let Some(letter) = drive_letter(arg) {
        return Ok(ScanRoot::Volume(letter));
    }
    let path = Path::new(arg);
    if !path.exists() {
        return Err(format!("path does not exist: {arg}"));
    }
    let absolute = std::path::absolute(path).map_err(|e| format!("{arg}: {e}"))?;
    Ok(ScanRoot::Folder(absolute))
}

/// The upper-case drive letter if `arg` is exactly a drive designator.
fn drive_letter(arg: &str) -> Option<char> {
    let mut chars = arg.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    match chars.as_str() {
        "" | ":" | ":\\" | ":/" => Some(letter.to_ascii_uppercase()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_forms_become_volumes() {
        for arg in ["C:", "c:", "C:\\", "C", "c"] {
            assert!(
                matches!(parse_target(arg), Ok(ScanRoot::Volume('C'))),
                "{arg}"
            );
        }
    }

    #[test]
    fn existing_folder_becomes_absolute_folder() {
        match parse_target(".") {
            Ok(ScanRoot::Folder(p)) => assert!(p.is_absolute()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn missing_path_is_an_error() {
        assert!(parse_target(r"E:\no\such\folder\here").is_err());
    }
}
