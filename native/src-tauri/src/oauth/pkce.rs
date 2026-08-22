use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl PkceChallenge {
    /// Generates a cryptographically random PKCE code verifier, S256 challenge, and OAuth state.
    pub fn new() -> Self {
        let mut verifier_bytes = [0u8; 64];
        let u1 = uuid::Uuid::new_v4();
        let u2 = uuid::Uuid::new_v4();
        let u3 = uuid::Uuid::new_v4();
        let u4 = uuid::Uuid::new_v4();
        verifier_bytes[0..16].copy_from_slice(u1.as_bytes());
        verifier_bytes[16..32].copy_from_slice(u2.as_bytes());
        verifier_bytes[32..48].copy_from_slice(u3.as_bytes());
        verifier_bytes[48..64].copy_from_slice(u4.as_bytes());
        let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        let challenge = URL_SAFE_NO_PAD.encode(hash);

        let state = uuid::Uuid::new_v4().to_string();

        Self {
            verifier,
            challenge,
            state,
        }
    }
}

impl Default for PkceChallenge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_generation() {
        let pkce = PkceChallenge::new();
        assert!(pkce.verifier.len() >= 43 && pkce.verifier.len() <= 128);
        assert!(!pkce.challenge.is_empty());
        assert!(!pkce.state.is_empty());

        let mut hasher = Sha256::new();
        hasher.update(pkce.verifier.as_bytes());
        let hash = hasher.finalize();
        let expected_challenge = URL_SAFE_NO_PAD.encode(hash);
        assert_eq!(pkce.challenge, expected_challenge);
    }
}
