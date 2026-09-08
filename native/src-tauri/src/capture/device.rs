//! Which microphone Relay opens.
//!
//! Three surfaces record — dictation, meetings and Talkback — and each called
//! `default_input_device()` directly. That is a reasonable default and it is
//! not a choice: `AudioInputSettings::selected_device` and `prefer_builtin_mic`
//! were both settings nothing read, so a user who picked a microphone was
//! recorded on whatever the OS preferred anyway.
//!
//! The resolution lives here, once, because three surfaces silently disagreeing
//! about which microphone is in use is worse than any of them being wrong.

use crate::settings::AudioInputSettings;

/// Picks a device name from what is available.
///
/// Pure, so the policy can be tested without an audio host — which matters,
/// because the interesting cases are all about *names* and the wrong answer is
/// a recording nobody can debug after the fact.
///
/// Precedence:
/// 1. An explicitly selected device, when it is still plugged in.
/// 2. Otherwise the OS default — unless it is a hands-free profile and
///    `prefer_builtin_mic` is on, in which case anything else.
/// 3. Otherwise whatever exists.
pub fn choose_input_device(
    available: &[String],
    os_default: Option<&str>,
    settings: &AudioInputSettings,
) -> Option<String> {
    if available.is_empty() {
        return None;
    }

    // An explicit choice wins whenever it is real. A name that no longer
    // matches anything — the headset is unplugged — falls through to the
    // default rather than failing the recording, because the user wants to be
    // recorded more than they want to be right about the device.
    if let Some(chosen) = settings
        .selected_device
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if let Some(found) = available.iter().find(|name| name.as_str() == chosen) {
            return Some(found.clone());
        }
        tracing::info!(
            "selected microphone '{}' is not connected; using the system default",
            chosen
        );
    }

    let default = os_default.filter(|name| available.iter().any(|a| a == name));

    // The one case worth overriding the OS on. A Bluetooth headset in
    // hands-free mode is 8 kHz mono and audibly worse than any other
    // microphone in the room — and Windows will happily make it the default
    // the moment it connects, without the user asking. Whisper on hands-free
    // audio is the difference between a transcript and a guess.
    //
    // Deliberately conservative: it only moves off the default when the
    // default is that specific bad case, and it never tries to identify a
    // "built-in" device positively. cpal reports no such flag, and guessing
    // from a name would mean overriding a good USB microphone somebody chose
    // on purpose.
    if settings.prefer_builtin_mic {
        if let Some(name) = default {
            if is_hands_free(name) {
                if let Some(better) = available.iter().find(|other| !is_hands_free(other)) {
                    tracing::info!(
                        "system default '{}' is a hands-free profile; recording on '{}'",
                        name,
                        better
                    );
                    return Some(better.clone());
                }
            }
        }
    }

    default
        .map(str::to_string)
        .or_else(|| available.first().cloned())
}

/// Whether this device name is a telephony profile rather than a microphone.
///
/// Name matching, because the platform offers nothing better through cpal. It
/// is used only to *avoid* a device, never to select one, so a false negative
/// costs the current behaviour and a false positive costs one device out of
/// several.
fn is_hands_free(name: &str) -> bool {
    let lowered = name.to_lowercase();
    ["hands-free", "hands free", "headset", "hfp", "communications"]
        .iter()
        .any(|marker| lowered.contains(marker))
}

/// The current preference, so every surface resolves the same way.
///
/// A process-wide value rather than a parameter because the three capture
/// surfaces each start their stream on their own spawned thread, in three
/// modules, and threading a settings struct through all of them would put
/// three copies of "which microphone" in three places — which is the thing
/// this module exists to prevent. It is set from the same two points that
/// already set the keep-warm duration: once at startup and again whenever
/// settings are saved.
static PREFERENCE: std::sync::RwLock<Option<AudioInputSettings>> =
    std::sync::RwLock::new(None);

/// Records the user's current audio-input preference.
pub fn set_preference(settings: &AudioInputSettings) {
    if let Ok(mut guard) = PREFERENCE.write() {
        *guard = Some(settings.clone());
    }
}

/// Opens the microphone the current preference asks for.
///
/// The entry point every capture surface uses. Before this, each called
/// `default_input_device()` and the two settings were read by nothing.
pub fn open_preferred(host: &cpal::Host) -> Option<cpal::Device> {
    use cpal::traits::HostTrait;
    let settings = PREFERENCE
        .read()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_default();
    open_input_device(host, &settings).or_else(|| host.default_input_device())
}

/// Opens the microphone the settings ask for.
///
/// Returns `None` only when there is no input device at all; every other
/// disagreement between the settings and reality resolves to something that
/// can record.
pub fn open_input_device(
    host: &cpal::Host,
    settings: &AudioInputSettings,
) -> Option<cpal::Device> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let devices: Vec<cpal::Device> = host.input_devices().ok()?.collect();
    let names: Vec<String> = devices
        .iter()
        .filter_map(|device| device.name().ok())
        .collect();
    let os_default = host.default_input_device().and_then(|d| d.name().ok());

    let chosen = choose_input_device(&names, os_default.as_deref(), settings);
    match chosen {
        Some(name) => devices
            .into_iter()
            .find(|device| device.name().ok().as_deref() == Some(name.as_str()))
            // Enumeration and the default can disagree on some hosts; the
            // default is still a device that records.
            .or_else(|| host.default_input_device()),
        None => host.default_input_device(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(selected: Option<&str>, prefer_builtin: bool) -> AudioInputSettings {
        AudioInputSettings {
            selected_device: selected.map(str::to_string),
            prefer_builtin_mic: prefer_builtin,
            ..Default::default()
        }
    }

    const BUILTIN: &str = "Microphone Array (Realtek(R) Audio)";
    const HEADSET: &str = "Headset (WH-1000XM4 Hands-Free AG Audio)";
    const USB: &str = "Yeti Stereo Microphone";

    #[test]
    fn an_explicit_choice_wins() {
        let available = vec![BUILTIN.to_string(), USB.to_string()];
        assert_eq!(
            choose_input_device(&available, Some(BUILTIN), &settings(Some(USB), true)),
            Some(USB.to_string())
        );
    }

    #[test]
    fn an_unplugged_choice_falls_back_rather_than_failing() {
        // The user wants to be recorded more than they want to be right about
        // the device.
        let available = vec![BUILTIN.to_string()];
        assert_eq!(
            choose_input_device(&available, Some(BUILTIN), &settings(Some(USB), true)),
            Some(BUILTIN.to_string())
        );
    }

    #[test]
    fn a_hands_free_default_is_stepped_over() {
        // Windows makes a Bluetooth headset the default the moment it
        // connects. In hands-free mode it is 8 kHz mono, which is the
        // difference between a transcript and a guess.
        let available = vec![HEADSET.to_string(), BUILTIN.to_string()];
        assert_eq!(
            choose_input_device(&available, Some(HEADSET), &settings(None, true)),
            Some(BUILTIN.to_string())
        );
    }

    #[test]
    fn a_hands_free_default_is_kept_when_it_is_all_there_is() {
        let available = vec![HEADSET.to_string()];
        assert_eq!(
            choose_input_device(&available, Some(HEADSET), &settings(None, true)),
            Some(HEADSET.to_string()),
            "a bad microphone still beats no microphone"
        );
    }

    #[test]
    fn a_hands_free_device_chosen_on_purpose_is_respected() {
        // The override only applies to the OS default. Somebody who picked
        // their headset meant it.
        let available = vec![HEADSET.to_string(), BUILTIN.to_string()];
        assert_eq!(
            choose_input_device(&available, Some(BUILTIN), &settings(Some(HEADSET), true)),
            Some(HEADSET.to_string())
        );
    }

    #[test]
    fn a_good_default_is_never_second_guessed() {
        // The rule avoids one bad case; it does not hunt for a "built-in"
        // device. Overriding a USB microphone somebody chose in their OS would
        // be worse than doing nothing.
        let available = vec![USB.to_string(), BUILTIN.to_string()];
        assert_eq!(
            choose_input_device(&available, Some(USB), &settings(None, true)),
            Some(USB.to_string())
        );
    }

    #[test]
    fn the_preference_can_be_turned_off() {
        let available = vec![HEADSET.to_string(), BUILTIN.to_string()];
        assert_eq!(
            choose_input_device(&available, Some(HEADSET), &settings(None, false)),
            Some(HEADSET.to_string())
        );
    }

    #[test]
    fn no_devices_means_no_choice() {
        assert_eq!(choose_input_device(&[], None, &settings(None, true)), None);
    }

    #[test]
    fn an_unknown_default_still_yields_something_recordable() {
        // Enumeration and the reported default disagree on some hosts.
        let available = vec![BUILTIN.to_string()];
        assert_eq!(
            choose_input_device(&available, Some("Ghost Device"), &settings(None, true)),
            Some(BUILTIN.to_string())
        );
    }
}
