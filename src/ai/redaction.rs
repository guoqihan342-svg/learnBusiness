use std::sync::OnceLock;

use regex::Regex;

pub fn redact_sensitive_text(input: &str) -> String {
    let redacted = api_key_regex().replace_all(input, "[REDACTED_API_KEY]");
    let redacted = email_regex().replace_all(&redacted, "[REDACTED_EMAIL]");
    let redacted = phone_regex().replace_all(&redacted, "[REDACTED_PHONE]");
    long_number_regex()
        .replace_all(&redacted, "[REDACTED_NUMBER]")
        .into_owned()
}

fn email_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap())
}

fn phone_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?:\+?86[- ]?)?1[3-9]\d{9}").unwrap())
}

fn long_number_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\d{12,}").unwrap())
}

fn api_key_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"sk-[A-Za-z0-9_-]{8,}").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_email_and_phone() {
        let input = "联系人 test@example.com 电话 13800138000";
        let output = redact_sensitive_text(input);
        assert!(!output.contains("test@example.com"));
        assert!(!output.contains("13800138000"));
    }

    #[test]
    fn redacts_long_numbers_and_api_keys() {
        let input = "account 6222021234567890123 key sk-testSecret123456";
        let output = redact_sensitive_text(input);
        assert!(!output.contains("6222021234567890123"));
        assert!(!output.contains("sk-testSecret123456"));
    }
}
