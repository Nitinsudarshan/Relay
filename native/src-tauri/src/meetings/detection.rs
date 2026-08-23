use crate::meetings::{
    PROVIDER_GOOGLE_MEET, PROVIDER_OTHER, PROVIDER_TEAMS, PROVIDER_WEBEX, PROVIDER_ZOOM,
};
#[cfg(target_os = "windows")]
use std::sync::Mutex;

/// A single window-detection signal: an open, visible top-level window whose
/// title matched a known conferencing app's pattern. `confidence` reflects
/// how specific the matched title is — a bare "Zoom Meeting"/"Meet - Google
/// Meet" fallback title (no actual topic visible) scores lower than a title
/// that clearly contains a real meeting name, because the former is much
/// more likely to be a stray browser tab or idle app window rather than an
/// active call.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct WindowMatch {
    pub provider: String,
    pub title: String,
    pub raw_title: String,
    pub source: String,
    pub confidence: f32,
}

/// Identifies the meeting provider from a URL or window title.
pub fn identify_meeting_provider(text: &str) -> (String, Option<String>) {
    let lower = text.to_lowercase();
    if lower.contains("meet.google.com") || lower.contains("google meet") || lower.starts_with("meet - ") || lower.contains(" - google meet") {
        let url = extract_url_with_pattern(text, "meet.google.com");
        (PROVIDER_GOOGLE_MEET.to_string(), url)
    } else if lower.contains("zoom.us") || lower.contains("zoom meeting") {
        let url = extract_url_with_pattern(text, "zoom.us");
        (PROVIDER_ZOOM.to_string(), url)
    } else if lower.contains("teams.microsoft.com") || lower.contains("teams.live.com") || lower.contains("microsoft teams") {
        let url = extract_url_with_pattern(text, "teams.microsoft.com")
            .or_else(|| extract_url_with_pattern(text, "teams.live.com"));
        (PROVIDER_TEAMS.to_string(), url)
    } else if lower.contains("webex.com") || lower.contains("cisco webex") {
        let url = extract_url_with_pattern(text, "webex.com");
        (PROVIDER_WEBEX.to_string(), url)
    } else {
        (PROVIDER_OTHER.to_string(), None)
    }
}

fn extract_url_with_pattern(text: &str, pattern: &str) -> Option<String> {
    for word in text.split_whitespace() {
        let clean = word.trim_matches(|c| c == '(' || c == ')' || c == '<' || c == '>' || c == '"' || c == '\'' || c == ',');
        if clean.contains(pattern) && (clean.starts_with("http://") || clean.starts_with("https://")) {
            return Some(clean.to_string());
        }
    }
    None
}

/// Sanitizes a raw browser or app window title into a clean meeting title.
/// Returns one of a small set of known generic fallback strings when the
/// title carries no real topic — `is_generic_fallback_title` checks a
/// cleaned title against exactly these, which is what backs the confidence
/// score below.
pub fn clean_meeting_window_title(raw_title: &str, provider: &str) -> String {
    let mut title = raw_title.trim().to_string();

    // Strip common browser suffixes
    for browser in &[
        " - Google Chrome",
        " - Microsoft​ Edge",
        " - Microsoft Edge",
        " — Mozilla Firefox",
        " - Mozilla Firefox",
        " - Brave",
        " - Opera",
        " - Vivaldi",
    ] {
        if title.ends_with(browser) {
            title = title[..title.len() - browser.len()].trim().to_string();
        }
    }

    if provider == PROVIDER_GOOGLE_MEET {
        if title.starts_with("Meet - ") {
            title = title["Meet - ".len()..].trim().to_string();
        }
        if title.ends_with(" - Google Meet") {
            title = title[..title.len() - " - Google Meet".len()].trim().to_string();
        }
        if title.is_empty() || title == "Meet" {
            title = "Google Meet Session".to_string();
        }
    } else if provider == PROVIDER_ZOOM {
        if title.starts_with("Zoom - ") {
            title = title["Zoom - ".len()..].trim().to_string();
        }
        if title.ends_with(" - Zoom") {
            title = title[..title.len() - " - Zoom".len()].trim().to_string();
        }
        if title.is_empty() || title == "Zoom" || title == "Zoom Meeting" {
            title = "Zoom Meeting".to_string();
        }
    } else if provider == PROVIDER_TEAMS {
        if title.ends_with(" | Microsoft Teams") {
            title = title[..title.len() - " | Microsoft Teams".len()].trim().to_string();
        }
        if title.starts_with("Meeting in ") {
            title = title["Meeting in ".len()..].trim().to_string();
        }
        if title.is_empty() || title == "Microsoft Teams" || title == "Microsoft Teams Meeting" {
            title = "Teams Meeting".to_string();
        }
    } else if provider == PROVIDER_WEBEX {
        if title.ends_with(" - Cisco Webex Meetings") {
            title = title[..title.len() - " - Cisco Webex Meetings".len()].trim().to_string();
        }
        if title.ends_with(" - Webex") {
            title = title[..title.len() - " - Webex".len()].trim().to_string();
        }
        if title.is_empty() || title == "Webex" {
            title = "Webex Meeting".to_string();
        }
    }

    title
}

/// The exact set of bare fallback titles `clean_meeting_window_title`
/// produces when the real window title carried no distinguishing topic.
fn is_generic_fallback_title(title: &str) -> bool {
    matches!(
        title,
        "Google Meet Session" | "Zoom Meeting" | "Teams Meeting" | "Webex Meeting"
    )
}

/// A generic title is weaker evidence of a real, specific meeting than a
/// distinctive one — see `resolver.rs`'s candidate-graduation rule, which
/// only needs one hit at high confidence but several sustained hits at low
/// confidence before writing a vault record.
fn score_confidence(cleaned_title: &str) -> f32 {
    if is_generic_fallback_title(cleaned_title) {
        0.55
    } else {
        0.85
    }
}

/// Enumerates native top-level windows on Windows to detect active video
/// conferencing sessions. Pure signal detection — creates no records and
/// has no side effects; `resolver.rs` decides what happens with the result.
#[cfg(target_os = "windows")]
pub fn detect_active_conferencing_windows() -> Vec<WindowMatch> {
    type HWND = *mut std::ffi::c_void;
    type LPARAM = isize;
    type BOOL = i32;

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(lpEnumFunc: Option<unsafe extern "system" fn(HWND, LPARAM) -> BOOL>, lParam: LPARAM) -> BOOL;
        fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
        fn IsWindowVisible(hWnd: HWND) -> BOOL;
    }

    static FOUND: Mutex<Vec<WindowMatch>> = Mutex::new(Vec::new());

    if let Ok(mut g) = FOUND.lock() {
        g.clear();
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, _: LPARAM) -> BOOL {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }

        let mut buffer = [0u16; 512];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), 512);
        if len > 0 {
            let title = String::from_utf16_lossy(&buffer[..len as usize]);
            let lower = title.to_lowercase();

            let matched_provider = if lower.contains("zoom meeting") || (lower.contains("zoom") && lower.contains("meeting id")) {
                Some(PROVIDER_ZOOM)
            } else if lower.contains("meet - ") || lower.contains(" - google meet") || lower.contains("meet.google.com") {
                Some(PROVIDER_GOOGLE_MEET)
            } else if lower.contains("microsoft teams meeting") || (lower.contains(" | microsoft teams") && !lower.starts_with("chat")) {
                Some(PROVIDER_TEAMS)
            } else if lower.contains("cisco webex meeting") || lower.contains("webex meeting") || lower.contains(" - cisco webex") {
                Some(PROVIDER_WEBEX)
            } else {
                None
            };

            if let Some(provider) = matched_provider {
                let clean = clean_meeting_window_title(&title, provider);
                let confidence = score_confidence(&clean);
                if let Ok(mut g) = FOUND.lock() {
                    g.push(WindowMatch {
                        provider: provider.to_string(),
                        title: clean,
                        raw_title: title,
                        source: "window_detector".to_string(),
                        confidence,
                    });
                }
            }
        }
        1
    }

    unsafe {
        let _ = EnumWindows(Some(enum_proc), 0);
    }

    FOUND.lock().map(|g| g.clone()).unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
pub fn detect_active_conferencing_windows() -> Vec<WindowMatch> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identify_meeting_provider() {
        let (p1, u1) = identify_meeting_provider("Join meeting at https://meet.google.com/abc-defg-hij");
        assert_eq!(p1, PROVIDER_GOOGLE_MEET);
        assert_eq!(u1, Some("https://meet.google.com/abc-defg-hij".to_string()));

        let (p2, u2) = identify_meeting_provider("Zoom URL: https://zoom.us/j/1234567890 please join promptly.");
        assert_eq!(p2, PROVIDER_ZOOM);
        assert_eq!(u2, Some("https://zoom.us/j/1234567890".to_string()));

        let (p3, _) = identify_meeting_provider("Meet - Architecture Review");
        assert_eq!(p3, PROVIDER_GOOGLE_MEET);
    }

    #[test]
    fn test_clean_meeting_window_title() {
        let clean_meet = clean_meeting_window_title("Meet - Sprint Architecture Planning - Google Chrome", PROVIDER_GOOGLE_MEET);
        assert_eq!(clean_meet, "Sprint Architecture Planning");

        let clean_teams = clean_meeting_window_title("Product Review | Microsoft Teams", PROVIDER_TEAMS);
        assert_eq!(clean_teams, "Product Review");

        let clean_zoom = clean_meeting_window_title("Zoom Meeting", PROVIDER_ZOOM);
        assert_eq!(clean_zoom, "Zoom Meeting");
    }

    #[test]
    fn test_generic_titles_score_lower_confidence() {
        assert!(is_generic_fallback_title("Zoom Meeting"));
        assert!(is_generic_fallback_title("Google Meet Session"));
        assert!(is_generic_fallback_title("Teams Meeting"));
        assert!(is_generic_fallback_title("Webex Meeting"));
        assert!(!is_generic_fallback_title("Sprint Architecture Planning"));

        assert!(score_confidence("Zoom Meeting") < score_confidence("Sprint Architecture Planning"));
    }
}
