use enigo::{Enigo, Keyboard, Settings};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InjectionError {
    #[error("Failed to connect to the OS input backend: {0}")]
    ConnectionFailed(String),

    #[error("Failed to simulate keystrokes: {0}")]
    SimulationFailed(String),

    #[error("Failed to access OS clipboard: {0}")]
    ClipboardError(String),
}

/// Represents the active OS window and document context captured when dictation starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFocusContext {
    pub hwnd: isize,
    pub title: String,
    pub process_id: u32,
}

/// Detailed outcome of an injection attempt, distinguishing successful insertions
/// from safely avoided wrong-target insertions (such as switched browser tabs or windows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectionOutcome {
    /// Text was successfully typed into the active field in the matching window and tab.
    Success,
    /// Active tab or document changed in the browser or editor; aborted typing so text
    /// is never inserted into the wrong tab.
    TabChanged {
        target_title: String,
        current_title: String,
    },
    /// Foreground application window changed; aborted typing so text is never
    /// inserted into the wrong application.
    AppChanged {
        target_title: String,
        current_title: String,
    },
    /// Waited for the user to return to the original window and tab, but the timeout expired.
    TimedOutWaitingForReturn {
        target_title: String,
    },
    /// Wait loop was cancelled (e.g. user began a new dictation session).
    Cancelled,
}

/// Normalizes titles to detect whether two window titles represent the same tab/document,
/// ignoring volatile decorations like unread counts `(1) `, dirty indicators `* `, etc.
pub fn is_same_tab_or_document(title_a: &str, title_b: &str) -> bool {
    if title_a == title_b {
        return true;
    }

    let clean = |s: &str| -> String {
        let mut s = s.trim();
        // Strip leading dirty markers (e.g. VS Code "* file.rs" or "● file.rs")
        if s.starts_with('*') || s.starts_with('●') {
            s = s[1..].trim();
        }
        // Strip leading unread badges like "(1) " or "[2] "
        if let Some(stripped) = s.strip_prefix('(') {
            if let Some(idx) = stripped.find(')') {
                let prefix = &stripped[..idx];
                if prefix.chars().all(|c| c.is_ascii_digit() || c == '+') {
                    s = stripped[idx + 1..].trim();
                }
            }
        } else if let Some(stripped) = s.strip_prefix('[') {
            if let Some(idx) = stripped.find(']') {
                let prefix = &stripped[..idx];
                if prefix.chars().all(|c| c.is_ascii_digit() || c == '+') {
                    s = stripped[idx + 1..].trim();
                }
            }
        }
        s.to_lowercase()
    };

    let a = clean(title_a);
    let b = clean(title_b);
    !a.is_empty() && a == b
}

/// Captures the foreground window context (HWND, title, PID) on Windows.
#[cfg(target_os = "windows")]
pub fn capture_target_focus_context() -> Option<TargetFocusContext> {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
        };

        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512) as usize;
        let title = String::from_utf16_lossy(&title_buf[..len]);

        Some(TargetFocusContext {
            hwnd: hwnd as isize,
            title,
            process_id: pid,
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn capture_target_focus_context() -> Option<TargetFocusContext> {
    None
}

/// Attempts to restore the target window back to the foreground if the user
/// switched to another window while dictation/transcription was processing.
#[cfg(target_os = "windows")]
pub fn restore_foreground_window(target_hwnd: isize) -> bool {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, SetForegroundWindow, GetWindowThreadProcessId,
        };
        use windows_sys::Win32::System::Threading::GetCurrentThreadId;

        #[link(name = "user32")]
        extern "system" {
            fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: i32) -> i32;
        }

        let current = GetForegroundWindow();
        if current as isize == target_hwnd {
            return true;
        }

        let current_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(target_hwnd as _, std::ptr::null_mut());

        if target_thread != 0 && current_thread != target_thread {
            AttachThreadInput(current_thread, target_thread, 1);
        }

        let ok = SetForegroundWindow(target_hwnd as _) != 0;

        if target_thread != 0 && current_thread != target_thread {
            AttachThreadInput(current_thread, target_thread, 0);
        }

        std::thread::sleep(std::time::Duration::from_millis(35));
        ok
    }
}

#[cfg(not(target_os = "windows"))]
pub fn restore_foreground_window(_target_hwnd: isize) -> bool {
    true
}

/// Injects text into the focused element, with protection against tab changes or window shifts.
/// If the user switched tabs in Chrome/Edge/Firefox or switched apps during transcription,
/// it prevents injection into the wrong location so text is never typed in the wrong place.
pub fn inject_text_safely(
    text: &str,
    target: Option<&TargetFocusContext>,
) -> Result<InjectionOutcome, InjectionError> {
    if text.trim().is_empty() {
        return Ok(InjectionOutcome::Success);
    }

    if let Some(target) = target {
        if let Some(current) = capture_target_focus_context() {
            if current.hwnd != target.hwnd {
                // The user switched to another application window. Attempt to restore target window.
                if restore_foreground_window(target.hwnd) {
                    if let Some(restored) = capture_target_focus_context() {
                        if !is_same_tab_or_document(&target.title, &restored.title) {
                            return Ok(InjectionOutcome::TabChanged {
                                target_title: target.title.clone(),
                                current_title: restored.title,
                            });
                        }
                    }
                } else {
                    return Ok(InjectionOutcome::AppChanged {
                        target_title: target.title.clone(),
                        current_title: current.title,
                    });
                }
            } else if !is_same_tab_or_document(&target.title, &current.title) {
                // Same window, but the active tab or document changed (e.g. Chrome, Edge, VS Code)!
                // Abort simulated typing so text is never injected into the wrong tab.
                return Ok(InjectionOutcome::TabChanged {
                    target_title: target.title.clone(),
                    current_title: current.title,
                });
            }
        }
    }

    inject_text(text)?;
    Ok(InjectionOutcome::Success)
}

/// Injects text into the target focused element. If the user switched tabs or windows
/// before transcription completes, this function does NOT blindly abort or type into the wrong tab:
/// it enters a polling loop (up to `timeout`) waiting for the user to return to the original window
/// and tab/document. As soon as the user returns, it allows the browser 150ms to reactivate
/// the input caret and injects the text directly into the text box.
pub fn inject_text_with_return_wait<F, W>(
    text: &str,
    target: Option<&TargetFocusContext>,
    timeout: std::time::Duration,
    check_interval: std::time::Duration,
    on_wait_start: W,
    is_cancelled: F,
) -> Result<InjectionOutcome, InjectionError>
where
    F: Fn() -> bool,
    W: FnOnce(&str),
{
    if text.trim().is_empty() {
        return Ok(InjectionOutcome::Success);
    }

    let target = match target {
        Some(t) => t,
        None => {
            inject_text(text)?;
            return Ok(InjectionOutcome::Success);
        }
    };

    // Fast path: Check if target window and tab are already active right now
    if let Some(current) = capture_target_focus_context() {
        if current.hwnd == target.hwnd {
            if is_same_tab_or_document(&target.title, &current.title) {
                inject_text(text)?;
                return Ok(InjectionOutcome::Success);
            }
        } else if restore_foreground_window(target.hwnd) {
            if let Some(restored) = capture_target_focus_context() {
                if is_same_tab_or_document(&target.title, &restored.title) {
                    inject_text(text)?;
                    return Ok(InjectionOutcome::Success);
                }
            }
        }
    }

    // Focus is currently in another tab or application.
    // Notify caller that we are entering wait mode so UI can inform the user.
    on_wait_start(&target.title);

    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if is_cancelled() {
            return Ok(InjectionOutcome::Cancelled);
        }

        std::thread::sleep(check_interval);

        if let Some(current) = capture_target_focus_context() {
            // Check if user returned to the target window and tab
            if current.hwnd == target.hwnd && is_same_tab_or_document(&target.title, &current.title) {
                // Stabilize: allow 150ms for browser DOM activeElement / caret to settle
                std::thread::sleep(std::time::Duration::from_millis(150));

                // Confirm tab hasn't shifted again before typing
                if let Some(recheck) = capture_target_focus_context() {
                    if recheck.hwnd == target.hwnd && is_same_tab_or_document(&target.title, &recheck.title) {
                        inject_text(text)?;
                        return Ok(InjectionOutcome::Success);
                    }
                }
            }
        }
    }

    Ok(InjectionOutcome::TimedOutWaitingForReturn {
        target_title: target.title.clone(),
    })
}

/// Types `text` into whichever field currently has OS focus, as if the user
/// had typed it themselves — this is what makes dictation "universal"
/// instead of confined to Relay's own window.
pub fn inject_text(text: &str) -> Result<(), InjectionError> {
    if text.trim().is_empty() {
        return Ok(());
    }

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| InjectionError::ConnectionFailed(e.to_string()))?;
    enigo
        .text(text)
        .map_err(|e| InjectionError::SimulationFailed(e.to_string()))?;
    Ok(())
}

/// Copies `text` directly to the OS clipboard natively using `arboard`.
/// This operates at the OS process level, completely bypassing browser/webview
/// document-focus restrictions which cause web-based `navigator.clipboard.writeText`
/// to fail with `DOMException: Document is not focused` when another application has focus.
pub fn copy_to_clipboard(text: &str) -> Result<(), InjectionError> {
    if text.is_empty() {
        return Ok(());
    }

    // Windows clipboard can occasionally be locked by another application
    // for a few milliseconds, so retry up to 3 times with brief backoff.
    for attempt in 0..3 {
        match arboard::Clipboard::new() {
            Ok(mut cb) => {
                match cb.set_text(text) {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        if attempt == 2 {
                            return Err(InjectionError::ClipboardError(format!(
                                "Failed to set clipboard text: {}",
                                e
                            )));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(30));
                    }
                }
            }
            Err(e) => {
                if attempt == 2 {
                    return Err(InjectionError::ClipboardError(format!(
                        "Failed to open clipboard: {}",
                        e
                    )));
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_copy_to_clipboard() {
        let test_str = "Relay dictation clipboard test string";
        let res = copy_to_clipboard(test_str);
        assert!(res.is_ok(), "Failed to copy to clipboard: {:?}", res);

        let mut cb = arboard::Clipboard::new().expect("Failed to open clipboard");
        let text = cb.get_text().expect("Failed to read text from clipboard");
        assert_eq!(text, test_str);
    }

    #[test]
    fn test_is_same_tab_or_document() {
        // Exact match
        assert!(is_same_tab_or_document("ChatGPT - Google Chrome", "ChatGPT - Google Chrome"));
        // Dirty / unread markers
        assert!(is_same_tab_or_document("* file.rs - VS Code", "file.rs - VS Code"));
        assert!(is_same_tab_or_document("(1) Inbox - Gmail", "(2) Inbox - Gmail"));
        assert!(is_same_tab_or_document("[99+] Slack | Channel", "Slack | Channel"));

        // Different tabs
        assert!(!is_same_tab_or_document("ChatGPT - Google Chrome", "YouTube - Google Chrome"));
        assert!(!is_same_tab_or_document("PR #1 - GitHub", "PR #2 - GitHub"));
    }

    #[test]
    fn test_inject_text_with_return_wait_empty_or_cancelled() {
        let dummy_target = TargetFocusContext {
            hwnd: 999999,
            title: "Nonexistent Window Title".to_string(),
            process_id: 1234,
        };

        // Empty text succeeds immediately
        let res_empty = inject_text_with_return_wait(
            "",
            Some(&dummy_target),
            std::time::Duration::from_millis(50),
            std::time::Duration::from_millis(10),
            |_| {},
            || false,
        );
        assert_eq!(res_empty.unwrap(), InjectionOutcome::Success);

        // Cancelled before/during wait returns Cancelled
        let res_cancelled = inject_text_with_return_wait(
            "Hello world",
            Some(&dummy_target),
            std::time::Duration::from_millis(50),
            std::time::Duration::from_millis(10),
            |_| {},
            || true, // immediately cancelled
        );
        assert_eq!(res_cancelled.unwrap(), InjectionOutcome::Cancelled);
    }
}
