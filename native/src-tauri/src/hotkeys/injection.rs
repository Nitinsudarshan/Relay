use enigo::{Enigo, Keyboard, Settings};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InjectionError {
    #[error("Failed to connect to the OS input backend: {0}")]
    ConnectionFailed(String),

    #[error("Failed to simulate keystrokes: {0}")]
    SimulationFailed(String),
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
