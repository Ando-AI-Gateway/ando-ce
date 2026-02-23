# Ando CE — Compliance Guide

> **Frameworks covered:** SOC2 Type II · ISO/IEC 27001:2022 · HIPAA · GDPR

This document maps each compliance requirement to the specific Ando CE control
that satisfies it, explains how to enable each control, and provides
ready-to-use configuration snippets.

---

## Table of Contents

1. [Control Matrix](#control-matrix)
2. [Quick-start: compliance profiles](#quick-start-compliance-profiles)
3. [SOC2 Type II](#soc2-type-ii)
4. [ISO/IEC 27001:2022](#isoiec-270012022)
5. [HIPAA](#hipaa)
6. [GDPR](#gdpr)
7. [Security Headers Plugin](#security-headers-plugin)
8. [Audit Log](#audit-log)
9. [PII / PHI Scrubbing](#pii--phi-scrubbing)
10. [TLS Hardening](#tls-hardening)
11. [Log Retention](#log-retention)
12. [Shared Responsibility Model](#shared-responsibility-model)

---

## Control Matrix

| #  | Requirement                         | Standard(s)                    | Ando CE Control                                         | Config key                                |
|----|-------------------------------------|--------------------------------|---------------------------------------------------------|-------------------------------------------|
| 1  | Encryption in transit               | HIPAA / SOC2 / ISO / GDPR      | TLS 1.2+ enforced at the proxy listener                 | `compliance.tls.min_version`              |
| 2  | HSTS header                         | SOC2 / ISO / GDPR              | `security-headers` plugin                               | `hsts_max_age`, `hsts_include_subdomains` |
| 3  | Clickjacking protection             | SOC2 / ISO / OWASP             | `security-headers` → `X-Frame-Options: DENY`            | `x_frame_options`                         |
| 4  | MIME sniffing protection            | SOC2 / ISO / OWASP             | `security-headers` → `X-Content-Type-Options: nosniff`  | `x_content_type_options`                  |
| 5  | Audit trail — every request         | HIPAA / SOC2 / ISO / GDPR      | `audit_log` module — structured JSON per transaction    | `compliance.audit_log.enabled`            |
| 6  | Audit trail — identity              | HIPAA / SOC2 / ISO             | `consumer_id` field in `AuditLogEntry`                  | automatic when auth plugin is active      |
| 7  | Audit trail — outcome               | HIPAA / SOC2 / ISO             | `outcome` + `deny_plugin` + `deny_reason` fields        | automatic                                 |
| 8  | Audit trail — body integrity        | HIPAA 164.312(b)               | `request_body_hash` (SHA-256) in `AuditLogEntry`        | `audit_log.include_request_body_hash`     |
| 9  | Sensitive header masking            | HIPAA / GDPR / SOC2            | `pii_scrubber::scrub_header` — Authorization, Cookie …  | `pii_scrubbing.scrub_headers`             |
| 10 | IP address pseudonymisation         | GDPR Art. 32                   | `pii_scrubber::anonymize_ip` — last octet / 64 bits     | `pii_scrubbing.anonymize_ips`             |
| 11 | URI query-param redaction           | HIPAA / GDPR                   | `pii_scrubber::scrub_uri` — regex-based redaction       | `pii_scrubbing.uri_redact_patterns`       |
| 12 | Cache-Control for PHI routes        | HIPAA 164.312(e)(2)(i)         | `security-headers` → `Cache-Control: no-store`          | `no_store_cache: true` (per-route)        |
| 13 | Log retention policy                | HIPAA 164.530(j) / SOC2        | `log_retention_days` (informational + infra config)     | `compliance.log_retention_days`           |
| 14 | Data residency tagging              | GDPR Art. 44 / sovereign cloud | `data_residency_region` tag for infra routing           | `compliance.data_residency_region`        |
| 15 | Rate limiting (availability)        | SOC2 Availability criteria     | `rate-limiting` plugin                                  | plugin config per-route                   |
| 16 | IP allow/deny lists                 | SOC2 CC6.6 / ISO A.8.22        | `ip-restriction` plugin                                 | plugin config per-route                   |
| 17 | Authentication                      | SOC2 CC6.1 / HIPAA 164.312(d)  | `key-auth`, `basic-auth`, `jwt-auth` plugins            | plugin config per-route                   |
| 18 | Referrer-Policy header              | GDPR / ISO A.8.23              | `security-headers` → `Referrer-Policy: no-referrer`     | `referrer_policy`                         |
| 19 | Permissions-Policy header           | SOC2 / OWASP                   | `security-headers` → `Permissions-Policy`               | `permissions_policy`                      |
| 20 | Content-Security-Policy             | SOC2 / OWASP A05               | `security-headers` → `Content-Security-Policy`          | `content_security_policy`                 |

---

## Quick-start: compliance profiles

### HIPAA profile

```yaml
# config/ando.yaml
compliance:
  hipaa: true
  tls:
    min_version: "TLSv1.2"
    strict_ciphers: true
    hsts_max_age_secs: 63072000       # 2 years
  audit_log:
    enabled: true
    include_request_body_hash: true   # non-repudiation / integrity
    format: "json"
    file_path: "/var/log/ando/audit.log"
  pii_scrubbing:
    scrub_headers: true
    anonymize_ips: false              # PHI routes: keep exact IP for audit
    uri_redact_patterns:
      - "(?i)ssn=[^&]+"
      - "(?i)dob=[^&]+"
      - "(?i)mrn=[^&]+"
  log_retention_days: 2190            # HIPAA § 164.530(j): 6 years
```

Add `security-headers` with `no_store_cache: true` on all PHI-bearing routes:

```yaml
# Route plugin config
plugins:
  - name: security-headers
    config:
      no_store_cache: true
```

### GDPR profile

```yaml
compliance:
  gdpr: true
  tls:
    min_version: "TLSv1.2"
  audit_log:
    enabled: true            # Art. 30 — Records of Processing Activities
    format: "json"
  pii_scrubbing:
    scrub_headers: true
    anonymize_ips: true      # Art. 32 pseudonymisation
    uri_redact_patterns:
      - "(?i)email=[^&]+"
      - "(?i)phone=[^&]+"
  log_retention_days: 365
  data_residency_region: "eu"
```

### SOC2 + ISO 27001 profile

```yaml
compliance:
  soc2: true
  iso27001: true
  tls:
    min_version: "TLSv1.2"
    strict_ciphers: true
  audit_log:
    enabled: true
    format: "json"
  pii_scrubbing:
    scrub_headers: true
  log_retention_days: 365
```

---

## SOC2 Type II

### Trust Service Criteria mapping

| TSC        | Criterion                                                   | Ando CE control              |
|------------|-------------------------------------------------------------|------------------------------|
| CC6.1      | Logical access restricted to authorised users               | `key-auth` / `jwt-auth`      |
| CC6.6      | Data transmitted over public networks encrypted             | TLS min 1.2, HSTS            |
| CC6.7      | Transmission security controls                              | `security-headers` plugin    |
| CC7.2      | Anomalous activities detected and logged                    | Audit log (every request)    |
| CC9.2      | Confidential information identified and protected           | PII scrubbing, header masking|
| A1.1–A1.3  | Availability commitments and system components              | Rate limiting, health checks |

### Recommended configuration

```yaml
compliance:
  soc2: true
  tls:
    min_version: "TLSv1.2"
    strict_ciphers: true
  audit_log:
    enabled: true
  pii_scrubbing:
    scrub_headers: true
  log_retention_days: 365
```

---

## ISO/IEC 27001:2022

### Annex A control mapping

| Annex A Control | Description                             | Ando CE implementation                   |
|-----------------|-----------------------------------------|------------------------------------------|
| A.5.15          | Access control                          | Auth plugins (key-auth, jwt-auth, basic) |
| A.5.16          | Identity management                     | `consumer_id` in audit records           |
| A.5.26          | Response to information security events | Audit log → SIEM integration             |
| A.8.11          | Data masking                            | PII scrubber (header + IP + URI)         |
| A.8.15          | Logging                                 | `audit_log` module                       |
| A.8.16          | Monitoring activities                   | Prometheus / VictoriaMetrics metrics     |
| A.8.22          | Filtering of web services               | `ip-restriction` plugin                  |
| A.8.23          | Web filtering                           | `security-headers` plugin                |
| A.8.24          | Use of cryptography                     | TLS config, JWT signing, HMAC auth       |

---

## HIPAA

### Technical Safeguards (45 CFR § 164.312)

| Safeguard                | Specification                              | Ando CE control                                  |
|--------------------------|---------------------------------------------|--------------------------------------------------|
| Access Control §(a)(1)  | Unique user identification                  | `consumer_id` recorded in every audit entry     |
| Access Control §(a)(1)  | Automatic logoff                            | JWT `exp` claim enforced by `jwt-auth` plugin    |
| Audit Controls §(b)     | Hardware/software activity review           | `AuditLogEntry` with outcome, plugin, request ID|
| Integrity §(e)(2)(i)    | Authentication of ePHI                      | `request_body_hash` (SHA-256)                    |
| Transmission §(e)(1)    | Encryption of ePHI in transit               | TLS 1.2+ listener, HSTS header                  |
| Transmission §(e)(2)(i) | Prevent unauthorised access during transit  | `Cache-Control: no-store` via security-headers   |

### PHI de-identification

Enable PII scrubbing on the routes that carry PHI:

```yaml
compliance:
  hipaa: true
  pii_scrubbing:
    scrub_headers: true
    uri_redact_patterns:
      - "(?i)ssn=[^&]+"
      - "(?i)dob=[^&]+"
      - "(?i)mrn=[^&]+"
      - "(?i)npi=[^&]+"
```

The `pii_scrubbed: true` flag in each `AuditLogEntry` provides auditors with
evidence that de-identification procedures were applied.

### Administrative Safeguards

| Requirement              | Ando CE support                                          |
|--------------------------|----------------------------------------------------------|
| § 164.530(j) Retention   | `compliance.log_retention_days` (≥ 2190 for 6 years)    |
| § 164.308(a)(5) Training | See [CONTRIBUTING.md](CONTRIBUTING.md) security section  |

---

## GDPR

### Article & Recital mapping

| Article / Recital          | Requirement                              | Ando CE control                               |
|----------------------------|------------------------------------------|-----------------------------------------------|
| Art. 5(1)(c) Minimisation  | Collect only necessary personal data     | `pii_scrubbing` — scrub headers & URIs        |
| Art. 5(1)(e) Retention     | Not kept longer than necessary           | `log_retention_days` + external purge job     |
| Art. 25 Privacy by design  | Default to most privacy-protective mode  | All scrubbing flags default-off (explicit opt-in) |
| Art. 30 Processing records | Document all processing activities       | `AuditLogEntry` JSON stream → DPA register    |
| Art. 32(1)(a) Pseudonymisation | Technical measure for risk reduction | `anonymize_ip` — IP pseudonymisation          |
| Art. 32(1)(b) Confidentiality | Ongoing confidentiality of data      | TLS 1.2+, HSTS, header masking               |
| Art. 33 Breach notification | 72-hour notification obligation         | Audit log + SIEM alerting (operator layer)    |
| Art. 44 Transfer safeguards | Data not transferred out of EEA        | `data_residency_region` tag for infra routing |

### IP address pseudonymisation

Under GDPR, IP addresses are personal data.  Enable anonymisation:

```yaml
compliance:
  gdpr: true
  pii_scrubbing:
    anonymize_ips: true
```

IPv4 result: `192.168.1.42` → `192.168.1.0`  
IPv6 result: last 64 bits zeroed (network prefix retained)

---

## Security Headers Plugin

The `security-headers` plugin (`ando-plugins/src/traffic/security_headers.rs`)
injects protective response headers on every matched route.

### Full configuration reference

```yaml
plugins:
  - name: security-headers
    config:
      # HSTS — forces HTTPS for browsers (HIPAA / SOC2 / ISO A.8.24)
      hsts_max_age: 31536000           # 1 year (seconds)
      hsts_include_subdomains: true
      hsts_preload: true

      # Clickjacking
      x_frame_options: "DENY"         # DENY | SAMEORIGIN | "" (omit)

      # MIME sniffing
      x_content_type_options: true    # → "nosniff"

      # Referrer leakage (GDPR)
      referrer_policy: "no-referrer"

      # CSP (set per-route based on content requirements)
      content_security_policy: "default-src 'self'"

      # Permissions (disable geolocation/mic/camera by default)
      permissions_policy: "geolocation=(), microphone=(), camera=()"

      # PHI / PII routes: prevent browser/CDN caching (HIPAA)
      no_store_cache: false
```

### Headers emitted (default config)

| Header                      | Value                              | Compliance driver              |
|-----------------------------|------------------------------------|--------------------------------|
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains; preload` | HIPAA / SOC2 / ISO |
| `X-Frame-Options`           | `DENY`                             | OWASP A05                      |
| `X-Content-Type-Options`    | `nosniff`                          | OWASP A05                      |
| `X-XSS-Protection`          | `1; mode=block`                    | Legacy browser hardening       |
| `Referrer-Policy`           | `no-referrer`                      | GDPR data minimisation         |
| `Permissions-Policy`        | `geolocation=(), microphone=(), camera=()` | SOC2 / OWASP     |

---

## Audit Log

Every request produces an `AuditLogEntry` (JSON) written to stdout or a file.

### Example record

```json
{
  "timestamp": "2026-02-23T10:05:33.421Z",
  "request_id": "req-a1b2c3d4",
  "consumer_id": "user-alice",
  "trace_id": "trace-xyz789",
  "route_id": "route-patients-api",
  "method": "GET",
  "uri": "/api/v1/patients?mrn=[REDACTED]",
  "response_status": 200,
  "duration_ms": 4.2,
  "outcome": "ALLOW",
  "deny_plugin": null,
  "deny_reason": null,
  "client_ip": "10.0.1.0",
  "pii_scrubbed": true,
  "request_body_hash": null
}
```

### Shipping to a SIEM

**VictoriaLogs / Grafana Loki:**

```yaml
observability:
  victoria_logs:
    enabled: true
    endpoint: "http://vlogs:9428/insert/jsonline"
```

**Splunk / Elastic:** pipe stdout to a Forwarder agent, or write to
`audit_log.file_path` and tail with Filebeat/Fluentd.

---

## PII / PHI Scrubbing

The `pii_scrubber` module (`ando-observability/src/pii_scrubber.rs`) provides
three functions callable from the proxy worker:

| Function             | What it does                                        |
|----------------------|-----------------------------------------------------|
| `scrub_header`       | Masks a single header value                         |
| `scrub_headers_map`  | Masks all sensitive headers in a `HashMap`          |
| `anonymize_ip`       | Pseudonymises an IPv4 or IPv6 address               |
| `scrub_uri`          | Redacts pattern-matched substrings in a URI         |
| `compile_patterns`   | Compiles regex patterns at startup (zero hot-path cost) |

### Always-masked headers

The following headers are **always** masked regardless of config:

- `Authorization`
- `Cookie`
- `Set-Cookie`
- `X-Api-Key`
- `X-Auth-Token`
- `X-Access-Token`
- `Proxy-Authorization`
- `WWW-Authenticate`

Add operator-specific headers via `extra_sensitive_headers`.

---

## TLS Hardening

Ando CE proxies TLS at the OS/kernel level.  The compliance config documents
the minimum version requirement; enforcement is done by the TLS library
configured in your deployment.

### Recommended cipher suites (NIST/FIPS 140-3)

```
TLS_AES_256_GCM_SHA384
TLS_AES_128_GCM_SHA256
TLS_CHACHA20_POLY1305_SHA256
ECDHE-RSA-AES256-GCM-SHA384
ECDHE-RSA-AES128-GCM-SHA256
```

Set these in your TLS terminator (nginx, HAProxy, or the OS-level config for
Ando CE's native TLS listener).

### Docker / Kubernetes TLS configuration

```yaml
# docker-compose.yml snippet
environment:
  - ANDO_PROXY_HTTPS_ADDR=0.0.0.0:9443
  - ANDO_COMPLIANCE_TLS_MIN_VERSION=TLSv1.2
```

---

## Log Retention

| Regulation | Minimum retention | Config value              |
|------------|-------------------|---------------------------|
| HIPAA      | 6 years           | `log_retention_days: 2190`|
| SOC2       | 1 year (common)   | `log_retention_days: 365` |
| GDPR       | Operator-defined  | Set per data category     |
| ISO 27001  | Operator-defined  | Typically 1–3 years       |

Ando CE records the `log_retention_days` value in the gateway configuration.
Actual purging of log files **must be implemented at the infrastructure layer**
(logrotate, S3 lifecycle rules, VictoriaLogs retention policies, etc.).

---

## Shared Responsibility Model

| Area                          | Ando CE (gateway)          | Operator / Infra                              |
|-------------------------------|----------------------------|-----------------------------------------------|
| Encryption in transit         | TLS listener, HSTS header  | Certificate management, CA trust store        |
| Encryption at rest            | Not applicable             | Disk encryption (LUKS, AWS EBS, GCP CMEK)     |
| Audit log generation          | ✅ AuditLogEntry per request | SIEM ingestion, integrity, tamper protection |
| Audit log retention           | Config documents policy    | Storage layer (S3 lifecycle, retention locks) |
| PII / PHI scrubbing in logs   | ✅ pii_scrubber module       | Application-layer de-identification          |
| Access control                | ✅ Auth plugins              | IdP / LDAP / SSO directory management        |
| Vulnerability management      | ✅ CI Clippy + audit         | OS patching, container scanning, pen testing  |
| Incident response             | Audit log + metrics        | SOC / SIRT team, runbooks, escalation paths  |
| Business continuity           | HA deployment (EE)         | DR runbooks, RPO/RTO commitments             |
| GDPR DPA (Art. 28)            | Not applicable             | Operator signs DPA with data subjects        |
| HIPAA BAA                     | Not applicable             | Operator signs BAA with covered entities     |

---

*Last updated: 2026-02-23 — Ando CE v0.1.x*
