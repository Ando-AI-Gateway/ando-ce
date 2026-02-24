//! PII / PHI scrubbing utilities for access logs and audit records.
//!
//! Satisfies:
//!
//! | Standard              | Control                                                   |
//! |-----------------------|-----------------------------------------------------------|
//! | HIPAA                 | 164.312(e)(2)(ii) – de-identification / encryption of PHI |
//! | GDPR                  | Art. 32 – pseudonymisation & data minimisation            |
//! | SOC2 Type II          | Confidentiality criteria (CC9.2)                          |
//! | ISO/IEC 27001:2022    | A.8.11 – Data masking                                     |
//!
//! # Usage
//!
//! ```
//! use ando_observability::pii_scrubber::{scrub_header, anonymize_ip, scrub_uri};
//!
//! // Mask a sensitive header
//! let (val, scrubbed) = scrub_header("Authorization", "Bearer eyJ...", &[]);
//! assert_eq!(val, "[REDACTED]");
//! assert!(scrubbed);
//!
//! // Anonymise an IPv4 address (GDPR)
//! assert_eq!(anonymize_ip("192.168.1.42"), "192.168.1.0");
//!
//! // Redact sensitive query-param values using compiled regexes
//! use regex::Regex;
//! let patterns = vec![Regex::new(r"(?i)ssn=[^&]+").unwrap()];
//! let (uri, scrubbed) = scrub_uri("/api/lookup?ssn=123-45-6789&foo=bar", &patterns);
//! assert!(scrubbed);
//! assert!(!uri.contains("123-45-6789"));
//! ```

use regex::Regex;
use std::net::IpAddr;

/// Replacement string used for all masked values.
pub const REDACTED: &str = "[REDACTED]";

/// Headers that are **always** masked, regardless of configuration.
///
/// These carry credentials, session tokens, or topology information and must
/// never appear in plaintext logs.
pub const ALWAYS_SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
    "x-access-token",
    "proxy-authorization",
    "www-authenticate", // may echo challenge with realm/nonce details
];

// ─────────────────────────────────────────────────────────────
// Header scrubbing
// ─────────────────────────────────────────────────────────────

/// Mask a single header value if the header name is considered sensitive.
///
/// Returns `(masked_value, was_scrubbed)`.
///
/// Sensitivity is determined by:
/// 1. Membership in [`ALWAYS_SENSITIVE_HEADERS`] (case-insensitive).
/// 2. Membership in the caller-supplied `extra` list (case-insensitive).
///
/// # Arguments
/// * `name`  – HTTP header name (e.g. `"Authorization"`).
/// * `value` – Raw header value as seen in the request.
/// * `extra` – Additional header names configured by the operator.
pub fn scrub_header(name: &str, value: &str, extra: &[String]) -> (String, bool) {
    let lower = name.to_lowercase();
    let is_sensitive = ALWAYS_SENSITIVE_HEADERS.contains(&lower.as_str())
        || extra.iter().any(|e| e.to_lowercase() == lower);

    if is_sensitive {
        (REDACTED.to_string(), true)
    } else {
        (value.to_string(), false)
    }
}

/// Scrub all headers in a mutable map.
///
/// Returns the number of headers that were masked.
///
/// ```
/// use std::collections::HashMap;
/// use ando_observability::pii_scrubber::scrub_headers_map;
///
/// let mut headers = HashMap::new();
/// headers.insert("authorization".to_string(), "Bearer secret".to_string());
/// headers.insert("content-type".to_string(), "application/json".to_string());
/// let count = scrub_headers_map(&mut headers, &[]);
/// assert_eq!(count, 1);
/// assert_eq!(headers["authorization"], "[REDACTED]");
/// assert_eq!(headers["content-type"], "application/json");
/// ```
pub fn scrub_headers_map(
    headers: &mut std::collections::HashMap<String, String>,
    extra: &[String],
) -> usize {
    let mut count = 0usize;
    for (key, value) in headers.iter_mut() {
        let (new_val, scrubbed) = scrub_header(key, value, extra);
        if scrubbed {
            *value = new_val;
            count += 1;
        }
    }
    count
}

// ─────────────────────────────────────────────────────────────
// IP anonymisation (GDPR Art. 32 pseudonymisation)
// ─────────────────────────────────────────────────────────────

/// Pseudonymise an IP address by zeroing its host-specific bits.
///
/// Technique aligns with the standard approach recognised by EU data-protection
/// authorities for making IP addresses *not directly identifiable*:
///
/// - **IPv4** `a.b.c.d` → `a.b.c.0`   (last octet zeroed)
/// - **IPv6** full address → network prefix with last 64 bits zeroed
///
/// Returns the original string unchanged when it cannot be parsed (already
/// anonymised, hostname, etc.).
pub fn anonymize_ip(ip: &str) -> String {
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            let [a, b, c, _] = v4.octets();
            format!("{a}.{b}.{c}.0")
        }
        Ok(IpAddr::V6(v6)) => {
            let mut segs = v6.segments();
            // Zero the last four 16-bit words (64-bit host part).
            segs[4] = 0;
            segs[5] = 0;
            segs[6] = 0;
            segs[7] = 0;
            std::net::Ipv6Addr::from(segs).to_string()
        }
        Err(_) => ip.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────
// URI / query-param scrubbing
// ─────────────────────────────────────────────────────────────

/// Replace any substrings in `uri` that match a compiled regex with
/// `[REDACTED]`.
///
/// Each pattern should match the part of the URI that contains a sensitive
/// value.  For example, to redact Social Security Numbers in query strings:
///
/// ```text
/// (?i)ssn=[^&]+
/// ```
///
/// Returns `(scrubbed_uri, was_scrubbed)`.
pub fn scrub_uri(uri: &str, patterns: &[Regex]) -> (String, bool) {
    let mut result = uri.to_string();
    let mut scrubbed = false;
    for re in patterns {
        let new = re.replace_all(&result, REDACTED).to_string();
        if new != result {
            scrubbed = true;
            result = new;
        }
    }
    (result, scrubbed)
}

/// Compile a list of regex pattern strings into [`Regex`] objects, ignoring
/// invalid patterns (logs a warning and skips).
pub fn compile_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns
        .iter()
        .filter_map(|p| {
            Regex::new(p)
                .map_err(|e| tracing::warn!("invalid PII redaction pattern {:?}: {}", p, e))
                .ok()
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ── scrub_header ─────────────────────────────────────────────

    #[test]
    fn authorization_header_is_always_masked() {
        let (val, scrubbed) = scrub_header("Authorization", "Bearer token123", &[]);
        assert_eq!(val, REDACTED);
        assert!(scrubbed);
    }

    #[test]
    fn authorization_header_case_insensitive() {
        let (val, _) = scrub_header("AUTHORIZATION", "Bearer token123", &[]);
        assert_eq!(val, REDACTED);
    }

    #[test]
    fn cookie_header_is_masked() {
        let (val, scrubbed) = scrub_header("cookie", "session=abc123", &[]);
        assert_eq!(val, REDACTED);
        assert!(scrubbed);
    }

    #[test]
    fn set_cookie_header_is_masked() {
        let (val, scrubbed) = scrub_header("Set-Cookie", "sid=xyz; HttpOnly", &[]);
        assert_eq!(val, REDACTED);
        assert!(scrubbed);
    }

    #[test]
    fn x_api_key_is_masked() {
        let (val, scrubbed) = scrub_header("x-api-key", "my-secret-key", &[]);
        assert_eq!(val, REDACTED);
        assert!(scrubbed);
    }

    #[test]
    fn content_type_is_not_masked() {
        let (val, scrubbed) = scrub_header("content-type", "application/json", &[]);
        assert_eq!(val, "application/json");
        assert!(!scrubbed);
    }

    #[test]
    fn extra_sensitive_header_is_masked() {
        let extra = vec!["x-patient-id".to_string()];
        let (val, scrubbed) = scrub_header("X-Patient-Id", "PAT-00123", &extra);
        assert_eq!(val, REDACTED);
        assert!(scrubbed);
    }

    #[test]
    fn non_sensitive_header_passes_through_unchanged() {
        let (val, scrubbed) = scrub_header("x-request-id", "req-abc", &[]);
        assert_eq!(val, "req-abc");
        assert!(!scrubbed);
    }

    // ── scrub_headers_map ────────────────────────────────────────

    #[test]
    fn scrub_headers_map_masks_sensitive_and_preserves_safe() {
        let mut headers = HashMap::new();
        headers.insert("authorization".to_string(), "Bearer secret".to_string());
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("x-api-key".to_string(), "my-key".to_string());

        let count = scrub_headers_map(&mut headers, &[]);
        assert_eq!(count, 2);
        assert_eq!(headers["authorization"], REDACTED);
        assert_eq!(headers["x-api-key"], REDACTED);
        assert_eq!(headers["content-type"], "application/json");
    }

    #[test]
    fn scrub_headers_map_returns_zero_when_no_sensitive_headers() {
        let mut headers = HashMap::new();
        headers.insert("accept".to_string(), "application/json".to_string());
        let count = scrub_headers_map(&mut headers, &[]);
        assert_eq!(count, 0);
    }

    // ── anonymize_ip ─────────────────────────────────────────────

    #[test]
    fn ipv4_last_octet_zeroed() {
        assert_eq!(anonymize_ip("192.168.1.42"), "192.168.1.0");
    }

    #[test]
    fn ipv4_all_last_octets_of_different_inputs() {
        assert_eq!(anonymize_ip("10.0.0.1"), "10.0.0.0");
        assert_eq!(anonymize_ip("172.16.254.255"), "172.16.254.0");
    }

    #[test]
    fn ipv6_last_64_bits_zeroed() {
        let anon = anonymize_ip("2001:db8::1");
        // First 64 bits preserved, last 64 zeroed
        let parsed: std::net::Ipv6Addr = anon.parse().unwrap();
        let segs = parsed.segments();
        assert_eq!(&segs[4..], &[0, 0, 0, 0]);
    }

    #[test]
    fn ipv6_loopback_anonymises() {
        let anon = anonymize_ip("::1");
        let parsed: std::net::Ipv6Addr = anon.parse().unwrap();
        let segs = parsed.segments();
        assert_eq!(&segs[4..], &[0, 0, 0, 0]);
    }

    #[test]
    fn unparseable_ip_passes_through() {
        let anon = anonymize_ip("not-an-ip");
        assert_eq!(anon, "not-an-ip");
    }

    #[test]
    fn already_anonymised_ipv4_unchanged() {
        assert_eq!(anonymize_ip("10.20.30.0"), "10.20.30.0");
    }

    // ── scrub_uri ────────────────────────────────────────────────

    #[test]
    fn ssn_pattern_is_redacted_from_query_string() {
        let patterns = vec![Regex::new(r"(?i)ssn=[^&]+").unwrap()];
        let (uri, scrubbed) = scrub_uri("/lookup?ssn=123-45-6789&foo=bar", &patterns);
        assert!(scrubbed);
        assert!(!uri.contains("123-45-6789"));
        assert!(uri.contains("foo=bar"));
    }

    #[test]
    fn credit_card_pattern_is_redacted() {
        let patterns = vec![Regex::new(r"\d{4}-\d{4}-\d{4}-\d{4}").unwrap()];
        let uri = "/pay?card=4111-1111-1111-1111&amount=100";
        let (result, scrubbed) = scrub_uri(uri, &patterns);
        assert!(scrubbed);
        assert!(!result.contains("4111-1111-1111-1111"));
    }

    #[test]
    fn no_matching_pattern_returns_uri_unchanged() {
        let patterns = vec![Regex::new(r"(?i)ssn=[^&]+").unwrap()];
        let uri = "/api/users?page=1&limit=10";
        let (result, scrubbed) = scrub_uri(uri, &patterns);
        assert!(!scrubbed);
        assert_eq!(result, uri);
    }

    #[test]
    fn empty_patterns_returns_original_uri() {
        let (result, scrubbed) = scrub_uri("/some/path?token=abc", &[]);
        assert!(!scrubbed);
        assert_eq!(result, "/some/path?token=abc");
    }

    #[test]
    fn multiple_patterns_all_applied() {
        let patterns = vec![
            Regex::new(r"(?i)ssn=[^&]+").unwrap(),
            Regex::new(r"(?i)token=[^&]+").unwrap(),
        ];
        let uri = "/lookup?ssn=111-22-3333&token=secret&page=1";
        let (result, scrubbed) = scrub_uri(uri, &patterns);
        assert!(scrubbed);
        assert!(!result.contains("111-22-3333"));
        assert!(!result.contains("secret"));
        assert!(result.contains("page=1"));
    }

    // ── compile_patterns ─────────────────────────────────────────

    #[test]
    fn valid_patterns_compile_correctly() {
        let patterns = vec![r"(?i)ssn=[^&]+".to_string(), r"\d+".to_string()];
        let compiled = compile_patterns(&patterns);
        assert_eq!(compiled.len(), 2);
    }

    #[test]
    fn invalid_pattern_is_skipped() {
        let patterns = vec![r"[invalid".to_string(), r"\d+".to_string()];
        let compiled = compile_patterns(&patterns);
        assert_eq!(compiled.len(), 1);
    }

    #[test]
    fn empty_patterns_list_returns_empty_vec() {
        let compiled = compile_patterns(&[]);
        assert!(compiled.is_empty());
    }
}
