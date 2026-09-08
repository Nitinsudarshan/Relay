use crate::settings::InjectionMethod;
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

/// Releases any lingering modifier keys (Ctrl, Shift, Alt, Win) on Windows.
///
/// Prevents modifier collisions when hotkey triggers (e.g. push-to-talk `Ctrl+Space`)
/// are released just before text injection begins, preventing synthetic shortcuts
/// or dropped keystrokes in applications like Windows 11 Notepad.
#[cfg(target_os = "windows")]
pub fn release_modifier_keys() {
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
            VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL,
            VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
        };

        let modifiers = [
            VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
            VK_SHIFT, VK_LSHIFT, VK_RSHIFT,
            VK_MENU, VK_LMENU, VK_RMENU,
            VK_LWIN, VK_RWIN,
        ];

        let mut inputs = Vec::new();
        for &vk in &modifiers {
            if (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 {
                let mut input = std::mem::zeroed::<INPUT>();
                input.r#type = INPUT_KEYBOARD;
                input.Anonymous.ki = KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                };
                inputs.push(input);
            }
        }

        if !inputs.is_empty() {
            SendInput(
                inputs.len() as u32,
                inputs.as_mut_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn release_modifier_keys() {}

/// Simulates a `Ctrl+V` paste shortcut via native OS input queue.
#[cfg(target_os = "windows")]
pub fn paste_from_clipboard() -> Result<(), InjectionError> {
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
        };

        release_modifier_keys();

        // Win32 virtual key for 'V' is 0x56
        const VK_V: u16 = 0x56;

        let mut inputs = [std::mem::zeroed::<INPUT>(); 4];

        // 1. Ctrl Down
        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[0].Anonymous.ki = KEYBDINPUT {
            wVk: VK_CONTROL,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };

        // 2. V Down
        inputs[1].r#type = INPUT_KEYBOARD;
        inputs[1].Anonymous.ki = KEYBDINPUT {
            wVk: VK_V,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };

        // 3. V Up
        inputs[2].r#type = INPUT_KEYBOARD;
        inputs[2].Anonymous.ki = KEYBDINPUT {
            wVk: VK_V,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };

        // 4. Ctrl Up
        inputs[3].r#type = INPUT_KEYBOARD;
        inputs[3].Anonymous.ki = KEYBDINPUT {
            wVk: VK_CONTROL,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };

        let sent = SendInput(
            4,
            inputs.as_mut_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );

        if sent != 4 {
            return Err(InjectionError::SimulationFailed(format!(
                "SendInput sent {}/4 keys for Ctrl+V paste",
                sent
            )));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn paste_from_clipboard() -> Result<(), InjectionError> {
    use enigo::{Direction, Key};
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| InjectionError::ConnectionFailed(e.to_string()))?;
    #[cfg(target_os = "macos")]
    let mod_key = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let mod_key = Key::Control;

    enigo.key(mod_key, Direction::Press)
        .map_err(|e| InjectionError::SimulationFailed(e.to_string()))?;
    enigo.key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| InjectionError::SimulationFailed(e.to_string()))?;
    enigo.key(mod_key, Direction::Release)
        .map_err(|e| InjectionError::SimulationFailed(e.to_string()))?;
    Ok(())
}

/// Whether the caret is still where it was when `before` was captured.
///
/// The guard on replacing text Relay already injected. Getting this wrong does
/// not fail safe: a selection sent to the wrong window selects and overwrites
/// somebody else's text.
///
/// The asymmetric case is the one worth stating. If focus information was
/// available at injection and is missing now — or the reverse — that is not
/// evidence of sameness, and it is refused. Only two present-and-matching
/// contexts, or two absent ones on a platform that reports none at all, count
/// as unchanged.
pub fn focus_unchanged(
    before: Option<&TargetFocusContext>,
    now: Option<&TargetFocusContext>,
) -> bool {
    match (before, now) {
        (Some(before), Some(now)) => {
            before.hwnd == now.hwnd && is_same_tab_or_document(&before.title, &now.title)
        }
        // Neither time reported a context: the platform does not provide one.
        // Refusing here would disable replacement outright there, and the
        // selection is bounded by what Relay itself typed.
        (None, None) => true,
        _ => false,
    }
}

/// Selects the last `steps` cursor positions before the caret.
///
/// Shift+Left, repeated. Used by the dictation cleanup to select exactly what
/// Relay injected so a replacement overwrites it rather than appending to it.
///
/// `steps` must come from `capture::text_normalize::cursor_steps`, not from
/// `chars().count()`. An arrow key moves by grapheme cluster, so counting chars
/// over-selects on any script with combining marks and would swallow whatever
/// the user had written before dictating.
///
/// Selecting nothing is a success: the caller has nothing to replace and a
/// subsequent injection simply inserts.
pub fn select_previous(steps: usize) -> Result<(), InjectionError> {
    use enigo::{Direction, Key};
    if steps == 0 {
        return Ok(());
    }

    release_modifier_keys();

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| InjectionError::ConnectionFailed(e.to_string()))?;
    enigo
        .key(Key::Shift, Direction::Press)
        .map_err(|e| InjectionError::SimulationFailed(e.to_string()))?;
    let mut result = Ok(());
    for _ in 0..steps {
        if let Err(e) = enigo.key(Key::LeftArrow, Direction::Click) {
            result = Err(InjectionError::SimulationFailed(e.to_string()));
            break;
        }
    }
    // Released even when a press failed part-way: leaving Shift held would
    // turn the user's next keystroke into a selection.
    let released = enigo
        .key(Key::Shift, Direction::Release)
        .map_err(|e| InjectionError::SimulationFailed(e.to_string()));
    result.and(released)
}

/// Injects text using simulated individual keystrokes.
pub fn inject_keystrokes(text: &str) -> Result<(), InjectionError> {
    if text.trim().is_empty() {
        return Ok(());
    }

    release_modifier_keys();

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| InjectionError::ConnectionFailed(e.to_string()))?;
    enigo
        .text(text)
        .map_err(|e| InjectionError::SimulationFailed(e.to_string()))?;
    Ok(())
}

/// Injects text into the focused element, with protection against tab changes or window shifts.
/// If the user switched tabs in Chrome/Edge/Firefox or switched apps during transcription,
/// it prevents injection into the wrong location so text is never typed in the wrong place.
pub fn inject_text_safely(
    text: &str,
    target: Option<&TargetFocusContext>,
    method: InjectionMethod,
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

    inject_text(text, method)?;
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
    method: InjectionMethod,
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
            inject_text(text, method)?;
            return Ok(InjectionOutcome::Success);
        }
    };

    // Fast path: Check if target window and tab are already active right now
    if let Some(current) = capture_target_focus_context() {
        if current.hwnd == target.hwnd {
            if is_same_tab_or_document(&target.title, &current.title) {
                inject_text(text, method)?;
                return Ok(InjectionOutcome::Success);
            }
        } else if restore_foreground_window(target.hwnd) {
            if let Some(restored) = capture_target_focus_context() {
                if is_same_tab_or_document(&target.title, &restored.title) {
                    inject_text(text, method)?;
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
                        inject_text(text, method)?;
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

/// Injects `text` into whichever field currently has OS focus using the selected `InjectionMethod`.
pub fn inject_text(text: &str, method: InjectionMethod) -> Result<(), InjectionError> {
    if text.trim().is_empty() {
        return Ok(());
    }

    match method {
        InjectionMethod::ClipboardPaste => {
            copy_to_clipboard(text)?;
            paste_from_clipboard()
        }
        InjectionMethod::Keystrokes => {
            inject_keystrokes(text)
        }
    }
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

    fn focus(hwnd: isize, title: &str) -> TargetFocusContext {
        TargetFocusContext {
            hwnd,
            title: title.to_string(),
            process_id: 42,
        }
    }

    #[test]
    fn replacement_is_refused_when_the_window_changed() {
        // The failure this guard exists for: a selection sent to the wrong
        // window selects and overwrites somebody else's text.
        let before = focus(1, "Untitled - Notepad");
        let elsewhere = focus(2, "Slack");
        assert!(!focus_unchanged(Some(&before), Some(&elsewhere)));
    }

    #[test]
    fn replacement_is_allowed_in_the_same_field() {
        let before = focus(1, "Untitled - Notepad");
        assert!(focus_unchanged(Some(&before), Some(&focus(1, "Untitled - Notepad"))));
    }

    #[test]
    fn a_volatile_title_decoration_does_not_count_as_moving() {
        // An unread badge or a dirty marker changes the title without moving
        // the caret; refusing there would make the feature feel broken in
        // exactly the apps people dictate into.
        let before = focus(1, "inbox - Gmail");
        assert!(focus_unchanged(Some(&before), Some(&focus(1, "(3) inbox - Gmail"))));
    }

    #[test]
    fn missing_focus_information_on_one_side_only_is_refused() {
        // Not evidence of sameness. Treating it as such is how a replacement
        // reaches a window nobody checked.
        let before = focus(1, "Untitled - Notepad");
        assert!(!focus_unchanged(Some(&before), None));
        assert!(!focus_unchanged(None, Some(&before)));
    }

    #[test]
    fn a_platform_that_reports_no_focus_at_all_still_works() {
        assert!(focus_unchanged(None, None));
    }

    use super::*;

    #[test]
    fn test_native_copy_to_clipboard() {
        // Headless CI environments (e.g. Linux runners without an X11/Wayland display server)
        // do not have an accessible OS clipboard. Skip when no clipboard backend is available.
        let mut cb = match arboard::Clipboard::new() {
            Ok(cb) => cb,
            Err(_) => return,
        };

        let test_str = "Relay dictation clipboard test string";
        let res = copy_to_clipboard(test_str);
        assert!(res.is_ok(), "Failed to copy to clipboard: {:?}", res);

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
            InjectionMethod::ClipboardPaste,
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
            InjectionMethod::Keystrokes,
            |_| {},
            || true, // immediately cancelled
        );
        assert_eq!(res_cancelled.unwrap(), InjectionOutcome::Cancelled);
    }
}
