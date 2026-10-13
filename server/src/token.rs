//! Stateless, tamper-proof test tokens.
//!
//! A token carries everything needed to verify a result later: the test id (which also seeds
//! the word generator), when it was issued, and the mode. It is signed with HMAC-SHA256, so
//! the server needs no storage for tests in progress and any instance can verify any token.
//!
//! Layout before base64url: `id (16) | issued_at_ms (8, big endian) | mode (1) | mac (32)`.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use thiserror::Error;
use uuid::Uuid;

use crate::mode::TestMode;

const PAYLOAD_LEN: usize = 16 + 8 + 1;
const MAC_LEN: usize = 32;
const TOKEN_LEN: usize = PAYLOAD_LEN + MAC_LEN;

/// The verified contents of a token: one issued test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestTicket {
    /// Unique test id. Results are stored under it, which makes each token single-use.
    pub id: Uuid,
    /// Unix time in milliseconds when the server issued the test.
    pub issued_at_ms: u64,
    /// The test configuration.
    pub mode: TestMode,
}

impl TestTicket {
    /// Seed for [`crate::words::generate`]. The id is random, so its first half is too.
    pub fn seed(&self) -> u64 {
        self.id.as_u64_pair().0
    }
}

/// Why a token was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TokenError {
    /// Not valid base64url, wrong length, or an unknown mode code.
    #[error("malformed test token")]
    Malformed,
    /// The signature does not match: the token was forged or altered.
    #[error("invalid test token signature")]
    BadSignature,
}

/// Signs and verifies tokens with a server secret.
///
/// Holds an HMAC already keyed with the secret. Cloning keyed state per call is cheaper than
/// re-deriving the key schedule on every request.
#[derive(Clone)]
pub struct TokenSigner {
    keyed_mac: Hmac<Sha256>,
}

impl TokenSigner {
    /// Creates a signer from the server secret.
    ///
    /// # Arguments
    ///
    /// * `secret` - Secret key bytes. Use at least 32 random bytes.
    pub fn new(secret: &[u8]) -> Self {
        let keyed_mac =
            Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts keys of any length");
        Self { keyed_mac }
    }

    /// Encodes and signs a ticket.
    ///
    /// # Returns
    ///
    /// A 76-character URL-safe token.
    pub fn sign(&self, ticket: &TestTicket) -> String {
        let mut bytes = [0_u8; TOKEN_LEN];
        bytes[..16].copy_from_slice(ticket.id.as_bytes());
        bytes[16..24].copy_from_slice(&ticket.issued_at_ms.to_be_bytes());
        bytes[24] = ticket.mode.code();
        let mac = self.mac(&bytes[..PAYLOAD_LEN]).finalize().into_bytes();
        bytes[PAYLOAD_LEN..].copy_from_slice(&mac);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Decodes a token and checks its signature in constant time.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError::Malformed`] if the token cannot be decoded and
    /// [`TokenError::BadSignature`] if it was not signed with this secret.
    pub fn verify(&self, token: &str) -> Result<TestTicket, TokenError> {
        // Decode into a stack buffer one byte larger than needed, so an overlong token is
        // detected by its decoded length instead of overflowing.
        let mut bytes = [0_u8; TOKEN_LEN + 1];
        let decoded_len = URL_SAFE_NO_PAD
            .decode_slice(token, &mut bytes)
            .map_err(|_| TokenError::Malformed)?;
        if decoded_len != TOKEN_LEN {
            return Err(TokenError::Malformed);
        }
        self.mac(&bytes[..PAYLOAD_LEN])
            .verify_slice(&bytes[PAYLOAD_LEN..TOKEN_LEN])
            .map_err(|_| TokenError::BadSignature)?;

        let mut id = [0_u8; 16];
        id.copy_from_slice(&bytes[..16]);
        let mut issued_at = [0_u8; 8];
        issued_at.copy_from_slice(&bytes[16..24]);
        Ok(TestTicket {
            id: Uuid::from_bytes(id),
            issued_at_ms: u64::from_be_bytes(issued_at),
            mode: TestMode::from_code(bytes[24]).ok_or(TokenError::Malformed)?,
        })
    }

    fn mac(&self, payload: &[u8]) -> Hmac<Sha256> {
        let mut mac = self.keyed_mac.clone();
        mac.update(payload);
        mac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticket() -> TestTicket {
        TestTicket {
            id: Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef),
            issued_at_ms: 1_760_000_000_000,
            mode: TestMode::Words25,
        }
    }

    #[test]
    fn sign_then_verify_returns_ticket() {
        let signer = TokenSigner::new(b"secret");
        let token = signer.sign(&ticket());
        assert_eq!(token.len(), 76);
        assert_eq!(signer.verify(&token), Ok(ticket()));
    }

    #[test]
    fn rejects_token_from_other_secret() {
        let token = TokenSigner::new(b"secret").sign(&ticket());
        assert_eq!(
            TokenSigner::new(b"other").verify(&token),
            Err(TokenError::BadSignature)
        );
    }

    #[test]
    fn rejects_tampered_payload() {
        let signer = TokenSigner::new(b"secret");
        let mut bytes = URL_SAFE_NO_PAD
            .decode(signer.sign(&ticket()))
            .expect("valid base64");
        bytes[24] = TestMode::Words10.code();
        let forged = URL_SAFE_NO_PAD.encode(bytes);
        assert_eq!(signer.verify(&forged), Err(TokenError::BadSignature));
    }

    #[test]
    fn rejects_wrong_length_and_garbage() {
        let signer = TokenSigner::new(b"secret");
        let token = signer.sign(&ticket());
        assert_eq!(signer.verify(&token[..70]), Err(TokenError::Malformed));
        assert_eq!(
            signer.verify(&format!("{token}AAAA")),
            Err(TokenError::Malformed)
        );
        assert_eq!(signer.verify("not a token!"), Err(TokenError::Malformed));
    }

    #[test]
    fn seed_comes_from_id() {
        assert_eq!(ticket().seed(), 0x0123_4567_89ab_cdef);
    }
}
