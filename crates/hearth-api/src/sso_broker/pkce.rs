//! PKCE (RFC 7636) helpers for the broker's *own* OAuth2 flow toward Kanidm.
//!
//! Kanidm 1.10 enforces PKCE on OAuth2 clients by default, so the broker acts
//! as a confidential client with its own PKCE pair on the upstream leg
//! (broker -> Kanidm), independent of the PKCE pair the downstream app
//! (e.g. Synapse) presents to the broker on the downstream leg.
//!
//! `verify_s256` is also consumed by the token endpoint (Task 9) to check the
//! downstream app's PKCE pair.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Generates a fresh PKCE code verifier: 32 random bytes, base64url (no padding),
/// per RFC 7636 §4.1.
pub fn generate_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Derives the S256 code challenge for `verifier`, per RFC 7636 §4.2:
/// `BASE64URL-ENCODE(SHA256(ASCII(verifier)))`.
pub fn s256_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// Verifies that `verifier` produces `challenge` under S256, in constant time.
pub fn verify_s256(verifier: &str, challenge: &str) -> bool {
    let computed = s256_challenge(verifier);
    computed.as_bytes().ct_eq(challenge.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 7636 Appendix B test vector.
    const RFC_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const RFC_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

    #[test]
    fn s256_challenge_matches_rfc7636_vector() {
        assert_eq!(s256_challenge(RFC_VERIFIER), RFC_CHALLENGE);
    }

    #[test]
    fn verify_s256_accepts_matching_pair() {
        assert!(verify_s256(RFC_VERIFIER, RFC_CHALLENGE));
    }

    #[test]
    fn verify_s256_rejects_mismatched_pair() {
        assert!(!verify_s256(RFC_VERIFIER, "not-the-right-challenge"));
        assert!(!verify_s256("not-the-right-verifier", RFC_CHALLENGE));
    }

    #[test]
    fn generate_verifier_is_random_and_base64url() {
        let a = generate_verifier();
        let b = generate_verifier();
        assert_ne!(a, b, "two verifiers should not collide");
        // 32 random bytes -> 43 base64url chars, no padding.
        assert_eq!(a.len(), 43);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "verifier must use only base64url characters, got {a:?}"
        );
    }

    #[test]
    fn generate_verifier_round_trips_through_s256() {
        let verifier = generate_verifier();
        let challenge = s256_challenge(&verifier);
        assert!(verify_s256(&verifier, &challenge));
    }
}
