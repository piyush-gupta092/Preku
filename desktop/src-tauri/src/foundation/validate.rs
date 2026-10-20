use super::errors::{PrekuError, Result};

/// Keys for settings and secrets: 1-128 chars of `a-z`, `0-9`, `.`, `_`, `-`.
pub(crate) fn key(kind: &str, key: &str) -> Result<()> {
    let valid = !key.is_empty()
        && key.len() <= 128
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(PrekuError::InvalidInput(format!(
            "{kind} key must be 1-128 chars of a-z, 0-9, '.', '_' or '-'"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_keys() {
        assert!(key("Setting", "github.token-1_a").is_ok());
    }

    #[test]
    fn rejects_bad_keys() {
        let too_long = "x".repeat(129);
        for bad in ["", "Has Upper", "spaces here", too_long.as_str()] {
            assert!(key("Setting", bad).is_err(), "{bad:?} should be rejected");
        }
    }
}
