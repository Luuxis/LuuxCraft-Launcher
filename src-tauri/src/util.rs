//! Small shared helpers.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Replaces every secret with a placeholder. Used on anything that may reach
/// the logs or the UI (game output, command lines).
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut redacted = text.to_owned();
    for secret in secrets.iter().filter(|s| s.len() >= 8) {
        if redacted.contains(secret.as_str()) {
            redacted = redacted.replace(secret.as_str(), "????????");
        }
    }
    redacted
}

/// `1.8.0_392` → 8, `17.0.9` → 17, `21` → 21.
pub fn java_major(version: &str) -> Option<u32> {
    let mut parts = version.trim().split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_long_secrets_only() {
        let secrets = vec!["supersecrettoken".to_owned(), "ab".to_owned()];
        assert_eq!(
            redact("token=supersecrettoken ab", &secrets),
            "token=???????? ab"
        );
    }

    #[test]
    fn parses_java_majors() {
        assert_eq!(java_major("1.8.0_392"), Some(8));
        assert_eq!(java_major("17.0.9"), Some(17));
        assert_eq!(java_major("21"), Some(21));
        assert_eq!(java_major("21.0.6+7-LTS"), Some(21));
        assert_eq!(java_major("garbage"), None);
    }
}
