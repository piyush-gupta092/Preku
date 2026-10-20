//! One-time pairing tokens (Architecture.md §4.1): 128 random bits, valid for 2 minutes, usable
//! once. Pure (time is passed in), so the rules are unit-tested.

use base64::Engine;

use crate::foundation::errors::{PrekuError, Result};

pub const TOKEN_TTL_MS: i64 = 2 * 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// No pairing window is open.
    NotPairing,
    Expired,
    Invalid,
}

impl Rejection {
    /// The `reason` sent in `pair.rejected`.
    pub fn reason(self) -> &'static str {
        match self {
            Self::NotPairing => "not_pairing",
            Self::Expired => "expired",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PairingWindow {
    token: String,
    pub expires_at: i64,
}

impl PairingWindow {
    pub fn open(now: i64) -> Result<Self> {
        let mut bytes = [0u8; 16];
        rustls::crypto::ring::default_provider()
            .secure_random
            .fill(&mut bytes)
            .map_err(|_| PrekuError::Bridge("no secure random source".into()))?;
        Ok(Self::with_token(
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
            now,
        ))
    }

    fn with_token(token: String, now: i64) -> Self {
        Self {
            token,
            expires_at: now + TOKEN_TTL_MS,
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

/// Checks a token against the open window; a correct token closes the window (single use).
pub fn redeem(
    window: &mut Option<PairingWindow>,
    token: &str,
    now: i64,
) -> std::result::Result<(), Rejection> {
    let open = window.as_ref().ok_or(Rejection::NotPairing)?;
    if now > open.expires_at {
        *window = None;
        return Err(Rejection::Expired);
    }
    if !constant_time_eq(open.token.as_bytes(), token.as_bytes()) {
        return Err(Rejection::Invalid);
    }
    *window = None;
    Ok(())
}

/// Compares without an early exit, so timing doesn't leak how much of a guess was right.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_works_once() {
        let mut window = Some(PairingWindow::with_token("abc".into(), 0));
        assert_eq!(redeem(&mut window, "abc", 1_000), Ok(()));
        assert_eq!(
            redeem(&mut window, "abc", 1_001),
            Err(Rejection::NotPairing)
        );
    }

    #[test]
    fn wrong_and_expired_tokens_are_refused() {
        let mut window = Some(PairingWindow::with_token("abc".into(), 0));
        assert_eq!(redeem(&mut window, "abd", 1), Err(Rejection::Invalid));
        assert_eq!(redeem(&mut window, "ab", 1), Err(Rejection::Invalid));
        assert!(window.is_some(), "a wrong guess doesn't close the window");
        assert_eq!(
            redeem(&mut window, "abc", TOKEN_TTL_MS + 1),
            Err(Rejection::Expired)
        );
        assert!(window.is_none());
    }

    #[test]
    fn real_tokens_are_128_bit_and_unique() {
        let a = PairingWindow::open(0).unwrap();
        let b = PairingWindow::open(0).unwrap();
        assert_eq!(a.token().len(), 22, "16 bytes in base64url");
        assert_ne!(a.token(), b.token());
        assert_eq!(a.expires_at, TOKEN_TTL_MS);
    }
}
