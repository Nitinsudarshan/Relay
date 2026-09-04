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
}
