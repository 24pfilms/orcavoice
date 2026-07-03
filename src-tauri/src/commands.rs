use crate::error::AppError;
use std::process::Command;

/// Checks whether `raw_text` begins with "open", "launch", or "start"
/// (case-insensitive). Returns the remainder as the application name, or
/// `None` when the text does not start with one of these prefixes or the
/// prefix is followed by nothing.
pub fn detect_open(raw_text: &str) -> Option<String> {
    let trimmed = raw_text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    for prefix in ["open ", "launch ", "start "] {
        if lower.strip_prefix(prefix).is_some() {
            // All prefixes are pure ASCII so byte offsets are identical in
            // both `lower` and `trimmed`.
            let app_name = trimmed[prefix.len()..].trim();
            if !app_name.is_empty() {
                return Some(app_name.to_string());
            }
        }
    }
    None
}

/// Launches an application by name using the OS-native mechanism.
/// Returns `Err` when the OS cannot find or start the application, so the
/// caller can fall back to pasting the transcript.
pub fn open_application(app_name: &str) -> Result<(), AppError> {
    let status = launch(app_name)
        .map_err(|e| AppError::Output(format!("Cannot launch \"{app_name}\": {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::Output(format!(
            "Could not find or launch \"{app_name}\""
        )))
    }
}

#[cfg(target_os = "windows")]
fn launch(app_name: &str) -> std::io::Result<std::process::ExitStatus> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("cmd")
        .args(["/C", "start", "", app_name])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
}

#[cfg(target_os = "macos")]
fn launch(app_name: &str) -> std::io::Result<std::process::ExitStatus> {
    Command::new("open").args(["-a", app_name]).status()
}

#[cfg(target_os = "linux")]
fn launch(app_name: &str) -> std::io::Result<std::process::ExitStatus> {
    Command::new("xdg-open").arg(app_name).status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_returns_none() {
        assert_eq!(detect_open(""), None);
        assert_eq!(detect_open("   "), None);
    }

    #[test]
    fn no_prefix_returns_none() {
        assert_eq!(detect_open("hello world"), None);
        assert_eq!(detect_open("write some code"), None);
    }

    #[test]
    fn open_alone_returns_none() {
        assert_eq!(detect_open("open"), None);
        assert_eq!(detect_open("OPEN"), None);
        assert_eq!(detect_open("open "), None);
    }

    #[test]
    fn open_prefix_extracts_app_name() {
        assert_eq!(detect_open("open chrome"), Some("chrome".to_string()));
        assert_eq!(
            detect_open("open visual studio code"),
            Some("visual studio code".to_string()),
        );
    }

    #[test]
    fn launch_prefix_extracts_app_name() {
        assert_eq!(detect_open("launch firefox"), Some("firefox".to_string()));
    }

    #[test]
    fn start_prefix_extracts_app_name() {
        assert_eq!(detect_open("start notepad"), Some("notepad".to_string()));
    }

    #[test]
    fn case_insensitive_prefix() {
        assert_eq!(detect_open("OPEN CHROME"), Some("CHROME".to_string()));
        assert_eq!(detect_open("Launch Firefox"), Some("Firefox".to_string()));
    }
}
