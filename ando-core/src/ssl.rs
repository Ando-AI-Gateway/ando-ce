use serde::{Deserialize, Serialize};

/// SSL certificate definition — APISIX-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslCertificate {
    pub id: String,

    /// PEM-encoded certificate.
    pub cert: String,

    /// PEM-encoded private key.
    pub key: String,

    /// SNI hostnames this cert applies to.
    #[serde(default)]
    pub snis: Vec<String>,

    /// Status: 1 = enabled, 0 = disabled.
    #[serde(default = "default_status")]
    pub status: u8,
}

fn default_status() -> u8 { 1 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_ssl_cert_deserializes() {
        let json = r#"{"id":"ssl1","cert":"CERT_PEM","key":"KEY_PEM"}"#;
        let ssl: SslCertificate = serde_json::from_str(json).unwrap();
        assert_eq!(ssl.id, "ssl1");
        assert_eq!(ssl.cert, "CERT_PEM");
        assert_eq!(ssl.key, "KEY_PEM");
        assert!(ssl.snis.is_empty());
        assert_eq!(ssl.status, 1, "default status must be 1 (enabled)");
    }

    #[test]
    fn ssl_cert_full_roundtrip() {
        let ssl = SslCertificate {
            id: "ssl2".into(),
            cert: "-----BEGIN CERTIFICATE-----\nXXX\n-----END CERTIFICATE-----".into(),
            key: "-----BEGIN PRIVATE KEY-----\nYYY\n-----END PRIVATE KEY-----".into(),
            snis: vec!["example.com".into(), "*.example.com".into()],
            status: 1,
        };
        let json = serde_json::to_string(&ssl).unwrap();
        let ssl2: SslCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(ssl2.id, "ssl2");
        assert_eq!(ssl2.snis.len(), 2);
        assert_eq!(ssl2.status, 1);
    }

    #[test]
    fn ssl_cert_disabled_status() {
        let json = r#"{"id":"ssl3","cert":"C","key":"K","status":0}"#;
        let ssl: SslCertificate = serde_json::from_str(json).unwrap();
        assert_eq!(ssl.status, 0);
    }

    #[test]
    fn ssl_cert_missing_required_field_errors() {
        let json = r#"{"id":"ssl4","cert":"C"}"#;
        let result: Result<SslCertificate, _> = serde_json::from_str(json);
        assert!(result.is_err(), "missing 'key' field should error");
    }
}
