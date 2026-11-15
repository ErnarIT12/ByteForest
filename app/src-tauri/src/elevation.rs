//! Restarting the app with administrator rights, and the startup arguments it relies on.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::Win32::UI::Shell::{SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HRESULT, PCWSTR};

/// Marks a process started by an elevation attempt, so it never tries again (no loops).
pub const ELEVATED_ATTEMPT: &str = "--elevated-attempt";

/// `ERROR_CANCELLED`: the user declined the UAC prompt.
const ERROR_CANCELLED: u32 = 1223;

/// Why a relaunch did not happen.
#[derive(Debug, PartialEq, Eq)]
pub enum ElevateError {
    /// The user declined the UAC prompt.
    Declined,
    /// Anything else, with a description.
    Other(String),
}

/// Command line for the elevated instance: `--elevated-attempt` plus `--scan "<target>"`.
/// A target containing a double quote is dropped, since it could break out of the quoting.
pub fn build_elevated_args(target: Option<&str>) -> String {
    match target {
        Some(t) if !t.contains('"') && !t.is_empty() => {
            // Backslashes right before the closing quote must be doubled, or `"C:\"`
            // would be read as `C:"` (CommandLineToArgvW rules).
            let trailing = t.len() - t.trim_end_matches('\\').len();
            let slashes = "\\".repeat(trailing);
            format!("{ELEVATED_ATTEMPT} --scan \"{t}{slashes}\"")
        }
        _ => ELEVATED_ATTEMPT.to_string(),
    }
}

/// True when the error means the user declined the UAC prompt (`ERROR_CANCELLED`).
pub fn is_declined(code: HRESULT) -> bool {
    code == HRESULT::from_win32(ERROR_CANCELLED)
}

/// Whether to relaunch elevated at startup: wanted, not yet elevated, not already tried.
pub fn should_auto_elevate(always: bool, elevated: bool, args: &[String]) -> bool {
    always && !elevated && !args.iter().any(|a| a == ELEVATED_ATTEMPT)
}

/// The value following the first `--scan`. An empty value, or one that looks like another
/// flag (starts with `--`), gives `None`.
pub fn parse_startup_scan(args: &[String]) -> Option<String> {
    let pos = args.iter().position(|a| a == "--scan")?;
    args.get(pos + 1)
        .filter(|v| !v.is_empty() && !v.starts_with("--"))
        .cloned()
}

/// Null-terminated UTF-16 copy of `s`.
fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// Starts this executable again with the UAC "runas" verb and the given command line.
/// Returns `Ok` once the new process has been launched; the caller should then exit.
pub fn relaunch_elevated(args: &str) -> Result<(), ElevateError> {
    let exe = std::env::current_exe().map_err(|e| ElevateError::Other(e.to_string()))?;
    tracing::info!("requesting elevation: {} {args}", exe.display());
    let exe_w = wide(exe.as_os_str());
    let args_w = wide(OsStr::new(args));
    let verb_w = wide(OsStr::new("runas"));

    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        lpVerb: PCWSTR(verb_w.as_ptr()),
        lpFile: PCWSTR(exe_w.as_ptr()),
        lpParameters: PCWSTR(args_w.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised with the correct `cbSize`, and the three wide
    // strings it points to outlive the call (they are locals of this function).
    let result = unsafe { ShellExecuteExW(&mut info) };
    match result {
        Ok(()) => {
            tracing::info!("elevation requested successfully");
            Ok(())
        }
        Err(e) => {
            let code = e.code();
            if is_declined(code) {
                tracing::info!("elevation declined by the user");
                Err(ElevateError::Declined)
            } else {
                tracing::warn!("elevation failed: {e}");
                Err(ElevateError::Other(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn args_with_target_are_quoted() {
        assert_eq!(
            build_elevated_args(Some(r"E:\Мои файлы")),
            r#"--elevated-attempt --scan "E:\Мои файлы""#
        );
    }

    #[test]
    fn trailing_backslashes_are_doubled() {
        assert_eq!(
            build_elevated_args(Some(r"C:\")),
            r#"--elevated-attempt --scan "C:\\""#
        );
    }

    /// Splits `line` with the real `CommandLineToArgvW` (the first token is the program name).
    fn split(line: &str) -> Vec<String> {
        use windows::Win32::Foundation::{HLOCAL, LocalFree};
        use windows::Win32::UI::Shell::CommandLineToArgvW;
        let wide_line = wide(OsStr::new(line));
        let mut count = 0;
        // SAFETY: the string is null-terminated and `count` is a valid out pointer; the
        // returned array holds `count` valid null-terminated strings and is freed once.
        unsafe {
            let argv = CommandLineToArgvW(PCWSTR(wide_line.as_ptr()), &mut count);
            assert!(!argv.is_null());
            let out = (0..count as usize)
                .map(|i| (*argv.add(i)).to_string().unwrap())
                .collect();
            LocalFree(Some(HLOCAL(argv.cast())));
            out
        }
    }

    #[test]
    fn windows_parses_the_built_arguments_back() {
        for target in [r"C:\", r"E:\Мои файлы\", r"E:\a b", r"E:\a b\\"] {
            let line = format!("app {}", build_elevated_args(Some(target)));
            assert_eq!(
                split(&line),
                ["app", "--elevated-attempt", "--scan", target],
                "{line}"
            );
        }
    }

    #[test]
    fn args_without_target_are_just_the_marker() {
        assert_eq!(build_elevated_args(None), "--elevated-attempt");
    }

    #[test]
    fn target_with_a_quote_is_dropped() {
        assert_eq!(
            build_elevated_args(Some(r#"E:\a" --evil"#)),
            "--elevated-attempt"
        );
    }

    #[test]
    fn only_error_cancelled_is_a_decline() {
        assert!(is_declined(HRESULT::from_win32(1223)));
        assert!(!is_declined(HRESULT::from_win32(5)));
    }

    #[test]
    fn auto_elevate_rules() {
        assert!(should_auto_elevate(true, false, &[]));
        assert!(!should_auto_elevate(
            true,
            false,
            &args(&["--elevated-attempt"])
        ));
        assert!(!should_auto_elevate(true, true, &[]));
        assert!(!should_auto_elevate(false, false, &[]));
    }

    #[test]
    fn startup_scan_is_the_value_after_the_flag() {
        assert_eq!(
            parse_startup_scan(&args(&["app", "--scan", "C:"])),
            Some("C:".to_string())
        );
        assert_eq!(parse_startup_scan(&args(&["app", "--scan"])), None);
        assert_eq!(parse_startup_scan(&args(&["app"])), None);
        assert_eq!(parse_startup_scan(&args(&["app", "--scan", ""])), None);
        assert_eq!(
            parse_startup_scan(&args(&["app", "--scan", "--elevated-attempt"])),
            None
        );
        assert_eq!(
            parse_startup_scan(&args(&["app", "--scan", "C:", "--scan", "D:"])),
            Some("C:".to_string()),
            "the first --scan wins"
        );
    }
}
