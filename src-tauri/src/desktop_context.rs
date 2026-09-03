use crate::error::AppError;
use std::sync::Mutex;

pub const MAX_SELECTION_CHARS: usize = 16_000;
const SELECTION_TOO_LONG: &str = "Selected text exceeds the 16,000-character limit.";

#[derive(Debug)]
pub struct DesktopContext {
    #[cfg(target_os = "windows")]
    target_window: isize,
    pub selected_text: Option<String>,
}

#[derive(Default)]
pub struct DesktopContextState(pub Mutex<Option<DesktopContext>>);

impl DesktopContext {
    pub fn capture(selection_enabled: bool) -> Result<Self, AppError> {
        capture(selection_enabled)
    }

    pub fn restore_target(&self) -> Result<(), AppError> {
        restore_target(self)
    }
}

fn normalize_selection(
    ranges: impl IntoIterator<Item = String>,
) -> Result<Option<String>, AppError> {
    let mut selection = String::new();
    for range in ranges.into_iter().filter(|range| !range.is_empty()) {
        if !selection.is_empty() {
            selection.push('\n');
        }
        selection.push_str(&range);
        validate_selection_size(&selection)?;
    }

    let selection = selection.trim();
    validate_selection_size(selection)?;
    Ok((!selection.is_empty()).then(|| selection.to_string()))
}

pub fn validate_selection_size(selection: &str) -> Result<(), AppError> {
    if selection.chars().count() > MAX_SELECTION_CHARS {
        return Err(AppError::Output(SELECTION_TOO_LONG.to_string()));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn capture(selection_enabled: bool) -> Result<DesktopContext, AppError> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let target = unsafe { GetForegroundWindow() };
    if target.0.is_null() {
        return Err(AppError::Output(
            "Cannot capture the target window for this recording.".to_string(),
        ));
    }

    let selected_text = if selection_enabled {
        match read_selected_text() {
            Ok(selection) => selection,
            Err(SelectionCaptureError::TooLong) => {
                return Err(AppError::Output(SELECTION_TOO_LONG.to_string()));
            }
            // Unsupported controls and UI Automation failures preserve ordinary dictation.
            Err(SelectionCaptureError::Unavailable(_error)) => None,
        }
    } else {
        None
    };

    Ok(DesktopContext {
        target_window: target.0 as isize,
        selected_text,
    })
}

#[cfg(not(target_os = "windows"))]
fn capture(_selection_enabled: bool) -> Result<DesktopContext, AppError> {
    Ok(DesktopContext {
        selected_text: None,
    })
}

#[cfg(target_os = "windows")]
fn restore_target(context: &DesktopContext) -> Result<(), AppError> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, IsWindow, SetForegroundWindow,
    };

    let target = HWND(context.target_window as *mut c_void);
    unsafe {
        if !IsWindow(Some(target)).as_bool() {
            return Err(AppError::Output(
                "The original target window is no longer available.".to_string(),
            ));
        }
        if GetForegroundWindow() != target && !SetForegroundWindow(target).as_bool() {
            return Err(AppError::Output(
                "Windows would not restore the original target window.".to_string(),
            ));
        }
        if GetForegroundWindow() != target {
            return Err(AppError::Output(
                "The original target window could not be verified.".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn restore_target(_context: &DesktopContext) -> Result<(), AppError> {
    Ok(())
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
enum SelectionCaptureError {
    Unavailable(AppError),
    TooLong,
}

#[cfg(target_os = "windows")]
fn read_selected_text() -> Result<Option<String>, SelectionCaptureError> {
    use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
    };

    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let _com = if initialized.is_ok() {
        ComGuard(true)
    } else if initialized == RPC_E_CHANGED_MODE {
        ComGuard(false)
    } else {
        return Err(unavailable("initialize COM", initialized.message()));
    };

    let automation: IUIAutomation = unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| unavailable("create UI Automation", error))?
    };
    let focused = unsafe {
        automation
            .GetFocusedElement()
            .map_err(|error| unavailable("read the focused control", error))?
    };
    let pattern: IUIAutomationTextPattern = unsafe {
        focused
            .GetCurrentPatternAs(UIA_TextPatternId)
            .map_err(|error| unavailable("read the selected-text pattern", error))?
    };
    let ranges = unsafe {
        pattern
            .GetSelection()
            .map_err(|error| unavailable("read selected ranges", error))?
    };
    let length = unsafe {
        ranges
            .Length()
            .map_err(|error| unavailable("count selected ranges", error))?
    };
    if length < 0 || length as usize > MAX_SELECTION_CHARS + 1 {
        return Err(SelectionCaptureError::TooLong);
    }

    let mut selected_ranges = Vec::with_capacity(length as usize);
    // 32,002 UTF-16 code units can represent the first 16,001 Unicode scalars,
    // allowing oversize detection without requesting an unbounded BSTR.
    const READ_LIMIT: i32 = (MAX_SELECTION_CHARS as i32 + 1) * 2;
    for index in 0..length {
        let range = unsafe {
            ranges
                .GetElement(index)
                .map_err(|error| unavailable("read a selected range", error))?
        };
        let text = unsafe {
            range
                .GetText(READ_LIMIT)
                .map_err(|error| unavailable("read selected text", error))?
                .to_string()
        };
        selected_ranges.push(text);
    }

    normalize_selection(selected_ranges).map_err(|_| SelectionCaptureError::TooLong)
}

#[cfg(target_os = "windows")]
fn unavailable(operation: &str, error: impl std::fmt::Display) -> SelectionCaptureError {
    SelectionCaptureError::Unavailable(AppError::Output(format!(
        "UI Automation could not {operation}: {error}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_ranges_are_joined_and_outer_whitespace_is_trimmed() {
        let normalized = normalize_selection(["  first".to_string(), "second  ".to_string()]);
        assert_eq!(normalized.unwrap().as_deref(), Some("first\nsecond"));
    }

    #[test]
    fn empty_and_whitespace_only_selections_are_ignored() {
        assert_eq!(normalize_selection([" \r\n ".to_string()]).unwrap(), None);
    }

    #[test]
    fn selection_limit_counts_unicode_characters_not_bytes() {
        let allowed = "🦀".repeat(MAX_SELECTION_CHARS);
        assert!(validate_selection_size(&allowed).is_ok());
        assert!(validate_selection_size(&(allowed + "x")).is_err());
    }

    #[test]
    fn oversized_selection_is_rejected_without_echoing_its_content() {
        let error = validate_selection_size(&"sensitive".repeat(2_001)).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("Output error: {SELECTION_TOO_LONG}")
        );
        assert!(!error.to_string().contains("sensitive"));
    }
}
