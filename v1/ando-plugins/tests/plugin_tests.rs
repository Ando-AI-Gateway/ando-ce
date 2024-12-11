use ando_plugin::plugin::{Phase, Plugin, PluginContext, PluginResult};
use std::collections::HashMap;

fn make_ctx_with_headers(method: &str, uri: &str, headers: HashMap<String, String>) -> PluginContext {
    PluginContext::new(
        method.to_string(),
        uri.to_string(),
        headers,
        "192.168.1.100".to_string(),
        "test-route".to_string(),
    )
}

fn make_ctx(method: &str, uri: &str) -> PluginContext {
    make_ctx_with_headers(method, uri, HashMap::new())
}

// =============================================================================
// Key Auth Plugin Tests
// =============================================================================
mod key_auth_tests {
    use super::*;
    use ando_plugins::auth::key_auth::KeyAuthPlugin;

    #[tokio::test]
    async fn test_key_auth_missing_key() {
        let plugin = KeyAuthPlugin::new();
        let mut ctx = make_ctx("GET", "/api/users");
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, .. } => assert_eq!(status, 401),
            _ => panic!("Expected 401 response"),
        }
    }

    #[tokio::test]
    async fn test_key_auth_from_header() {
        let plugin = KeyAuthPlugin::new();
        let headers = HashMap::from([
            ("X-API-KEY".to_string(), "my-secret-key".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(
            ctx.get_var("api_key"),
            Some(&serde_json::json!("my-secret-key"))
        );
    }

    #[tokio::test]
    async fn test_key_auth_from_custom_header() {
        let plugin = KeyAuthPlugin::new();
        let headers = HashMap::from([
            ("X-Custom-Auth".to_string(), "custom-key".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({"header": "X-Custom-Auth"});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(
            ctx.get_var("api_key"),
            Some(&serde_json::json!("custom-key"))
        );
    }

    #[tokio::test]
    async fn test_key_auth_from_query_string() {
        let plugin = KeyAuthPlugin::new();
        let mut ctx = make_ctx("GET", "/api/users?apikey=query-key-123");
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(
            ctx.get_var("api_key"),
            Some(&serde_json::json!("query-key-123"))
        );
    }

    #[tokio::test]
    async fn test_key_auth_from_custom_query_param() {
        let plugin = KeyAuthPlugin::new();
        let mut ctx = make_ctx("GET", "/api/users?token=my-token");
        let config = serde_json::json!({"query": "token"});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(
            ctx.get_var("api_key"),
            Some(&serde_json::json!("my-token"))
        );
    }

    #[tokio::test]
    async fn test_key_auth_hide_credentials() {
        let plugin = KeyAuthPlugin::new();
        let headers = HashMap::from([
            ("X-API-KEY".to_string(), "secret".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({"hide_credentials": true});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        // Header should be removed
        assert!(ctx.get_header("X-API-KEY").is_none());
        // Other headers should remain
        assert!(ctx.get_header("content-type").is_some());
    }

    #[tokio::test]
    async fn test_key_auth_header_priority_over_query() {
        let plugin = KeyAuthPlugin::new();
        let headers = HashMap::from([
            ("X-API-KEY".to_string(), "header-key".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users?apikey=query-key", headers);
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(
            ctx.get_var("api_key"),
            Some(&serde_json::json!("header-key"))
        );
    }

    #[test]
    fn test_key_auth_metadata() {
        let plugin = KeyAuthPlugin::new();
        assert_eq!(plugin.name(), "key-auth");
        assert_eq!(plugin.priority(), 2500);
        assert_eq!(plugin.phases(), vec![Phase::Access]);
    }
}

// =============================================================================
// Basic Auth Plugin Tests
// =============================================================================
mod basic_auth_tests {
    use super::*;
    use ando_plugins::auth::basic_auth::BasicAuthPlugin;

    #[tokio::test]
    async fn test_basic_auth_missing_header() {
        let plugin = BasicAuthPlugin::new();
        let mut ctx = make_ctx("GET", "/api/users");
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, headers, .. } => {
                assert_eq!(status, 401);
                assert!(headers.contains_key("www-authenticate"));
            }
            _ => panic!("Expected 401 response"),
        }
    }

    #[tokio::test]
    async fn test_basic_auth_invalid_format() {
        let plugin = BasicAuthPlugin::new();
        let headers = HashMap::from([
            ("authorization".to_string(), "Bearer not-basic".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, .. } => assert_eq!(status, 401),
            _ => panic!("Expected 401 response"),
        }
    }

    #[tokio::test]
    async fn test_basic_auth_valid() {
        let plugin = BasicAuthPlugin::new();
        let headers = HashMap::from([
            ("authorization".to_string(), "Basic dXNlcjpwYXNz".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(
            ctx.get_var("basic_auth_encoded"),
            Some(&serde_json::json!("dXNlcjpwYXNz"))
        );
    }

    #[tokio::test]
    async fn test_basic_auth_hide_credentials() {
        let plugin = BasicAuthPlugin::new();
        let headers = HashMap::from([
            ("authorization".to_string(), "Basic dXNlcjpwYXNz".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({"hide_credentials": true});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert!(ctx.get_header("authorization").is_none());
    }

    #[test]
    fn test_basic_auth_metadata() {
        let plugin = BasicAuthPlugin::new();
        assert_eq!(plugin.name(), "basic-auth");
        assert_eq!(plugin.priority(), 2520);
        assert_eq!(plugin.phases(), vec![Phase::Access]);
    }
}

// =============================================================================
// JWT Auth Plugin Tests
// =============================================================================
mod jwt_auth_tests {
    use super::*;
    use ando_plugins::auth::jwt_auth::JwtAuthPlugin;

    #[tokio::test]
    async fn test_jwt_auth_missing_token() {
        let plugin = JwtAuthPlugin::new();
        let mut ctx = make_ctx("GET", "/api/users");
        let config = serde_json::json!({"secret": "test-secret"});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, .. } => assert_eq!(status, 401),
            _ => panic!("Expected 401 response"),
        }
    }

    #[tokio::test]
    async fn test_jwt_auth_missing_secret_config() {
        let plugin = JwtAuthPlugin::new();
        let headers = HashMap::from([
            ("authorization".to_string(), "Bearer some.jwt.token".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Error(msg) => assert!(msg.contains("missing 'secret'")),
            _ => panic!("Expected Error result"),
        }
    }

    #[tokio::test]
    async fn test_jwt_auth_invalid_token() {
        let plugin = JwtAuthPlugin::new();
        let headers = HashMap::from([
            ("authorization".to_string(), "Bearer invalid.jwt.token".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({"secret": "test-secret"});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, .. } => assert_eq!(status, 401),
            _ => panic!("Expected 401 response"),
        }
    }

    #[tokio::test]
    async fn test_jwt_auth_valid_token() {
        // Create a valid JWT
        use jsonwebtoken::{encode, EncodingKey, Header};

        let claims = serde_json::json!({
            "sub": "user-123",
            "exp": chrono::Utc::now().timestamp() as usize + 3600,
        });

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("my-secret".as_bytes()),
        )
        .unwrap();

        let plugin = JwtAuthPlugin::new();
        let headers = HashMap::from([
            ("authorization".to_string(), format!("Bearer {}", token)),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api/users", headers);
        let config = serde_json::json!({"secret": "my-secret"});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(
            ctx.get_var("jwt_sub"),
            Some(&serde_json::json!("user-123"))
        );
    }

    #[test]
    fn test_jwt_auth_metadata() {
        let plugin = JwtAuthPlugin::new();
        assert_eq!(plugin.name(), "jwt-auth");
        assert_eq!(plugin.priority(), 2510);
        assert_eq!(plugin.phases(), vec![Phase::Access]);
    }
}

// =============================================================================
// IP Restriction Plugin Tests
// =============================================================================
mod ip_restriction_tests {
    use super::*;
    use ando_plugins::security::ip_restriction::IpRestrictionPlugin;

    fn make_ctx_with_ip(ip: &str) -> PluginContext {
        PluginContext::new(
            "GET".to_string(),
            "/api".to_string(),
            HashMap::new(),
            ip.to_string(),
            "r1".to_string(),
        )
    }

    #[tokio::test]
    async fn test_ip_restriction_no_config() {
        let plugin = IpRestrictionPlugin::new();
        let mut ctx = make_ctx_with_ip("1.2.3.4");
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
    }

    #[tokio::test]
    async fn test_ip_restriction_whitelist_allow() {
        let plugin = IpRestrictionPlugin::new();
        let mut ctx = make_ctx_with_ip("192.168.1.50");
        let config = serde_json::json!({"whitelist": ["192.168.0.0/16"]});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
    }

    #[tokio::test]
    async fn test_ip_restriction_whitelist_deny() {
        let plugin = IpRestrictionPlugin::new();
        let mut ctx = make_ctx_with_ip("10.0.0.1");
        let config = serde_json::json!({"whitelist": ["192.168.0.0/16"]});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, .. } => assert_eq!(status, 403),
            _ => panic!("Expected 403 response"),
        }
    }

    #[tokio::test]
    async fn test_ip_restriction_blacklist_deny() {
        let plugin = IpRestrictionPlugin::new();
        let mut ctx = make_ctx_with_ip("10.0.0.5");
        let config = serde_json::json!({"blacklist": ["10.0.0.0/8"]});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, .. } => assert_eq!(status, 403),
            _ => panic!("Expected 403 response"),
        }
    }

    #[tokio::test]
    async fn test_ip_restriction_blacklist_allow() {
        let plugin = IpRestrictionPlugin::new();
        let mut ctx = make_ctx_with_ip("192.168.1.1");
        let config = serde_json::json!({"blacklist": ["10.0.0.0/8"]});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
    }

    #[tokio::test]
    async fn test_ip_restriction_custom_message() {
        let plugin = IpRestrictionPlugin::new();
        let mut ctx = make_ctx_with_ip("10.0.0.1");
        let config = serde_json::json!({
            "blacklist": ["10.0.0.0/8"],
            "message": "Your IP is blocked"
        });

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { body, .. } => {
                let body_str = String::from_utf8(body.unwrap()).unwrap();
                assert!(body_str.contains("Your IP is blocked"));
            }
            _ => panic!("Expected Response"),
        }
    }

    #[test]
    fn test_ip_restriction_metadata() {
        let plugin = IpRestrictionPlugin::new();
        assert_eq!(plugin.name(), "ip-restriction");
        assert_eq!(plugin.priority(), 3000);
        assert_eq!(plugin.phases(), vec![Phase::Access]);
    }
}

// =============================================================================
// CORS Plugin Tests
// =============================================================================
mod cors_tests {
    use super::*;
    use ando_plugins::transform::cors::CorsPlugin;

    #[tokio::test]
    async fn test_cors_preflight_options() {
        let plugin = CorsPlugin::new();
        let mut ctx = make_ctx("OPTIONS", "/api/users");
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Rewrite, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, headers, body } => {
                assert_eq!(status, 204);
                assert_eq!(headers.get("Access-Control-Allow-Origin").unwrap(), "*");
                assert!(headers.contains_key("Access-Control-Allow-Methods"));
                assert!(headers.contains_key("Access-Control-Allow-Headers"));
                assert!(headers.contains_key("Access-Control-Max-Age"));
                assert!(body.is_none());
            }
            _ => panic!("Expected 204 preflight response"),
        }
    }

    #[tokio::test]
    async fn test_cors_preflight_custom_origin() {
        let plugin = CorsPlugin::new();
        let mut ctx = make_ctx("OPTIONS", "/api/users");
        let config = serde_json::json!({"allow_origins": "https://example.com"});

        let result = plugin.execute(Phase::Rewrite, &mut ctx, &config).await;
        match result {
            PluginResult::Response { headers, .. } => {
                assert_eq!(
                    headers.get("Access-Control-Allow-Origin").unwrap(),
                    "https://example.com"
                );
            }
            _ => panic!("Expected preflight response"),
        }
    }

    #[tokio::test]
    async fn test_cors_preflight_with_credentials() {
        let plugin = CorsPlugin::new();
        let mut ctx = make_ctx("OPTIONS", "/api");
        let config = serde_json::json!({"allow_credential": true});

        let result = plugin.execute(Phase::Rewrite, &mut ctx, &config).await;
        match result {
            PluginResult::Response { headers, .. } => {
                assert_eq!(
                    headers.get("Access-Control-Allow-Credentials").unwrap(),
                    "true"
                );
            }
            _ => panic!("Expected preflight response"),
        }
    }

    #[tokio::test]
    async fn test_cors_non_options_rewrite_phase() {
        let plugin = CorsPlugin::new();
        let mut ctx = make_ctx("GET", "/api/users");
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Rewrite, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
    }

    #[tokio::test]
    async fn test_cors_header_filter_phase() {
        let plugin = CorsPlugin::new();
        let mut ctx = make_ctx("GET", "/api/users");
        let config = serde_json::json!({
            "allow_origins": "https://example.com",
            "expose_headers": "X-RateLimit-Remaining",
            "allow_credential": true
        });

        let result = plugin.execute(Phase::HeaderFilter, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(
            ctx.response_headers.get("Access-Control-Allow-Origin").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            ctx.response_headers.get("Access-Control-Expose-Headers").unwrap(),
            "X-RateLimit-Remaining"
        );
        assert_eq!(
            ctx.response_headers.get("Access-Control-Allow-Credentials").unwrap(),
            "true"
        );
    }

    #[test]
    fn test_cors_metadata() {
        let plugin = CorsPlugin::new();
        assert_eq!(plugin.name(), "cors");
        assert_eq!(plugin.priority(), 4000);
        assert_eq!(plugin.phases(), vec![Phase::Rewrite, Phase::HeaderFilter]);
    }
}

// =============================================================================
// Request Transformer Plugin Tests
// =============================================================================
mod request_transformer_tests {
    use super::*;
    use ando_plugins::transform::request_transformer::RequestTransformerPlugin;

    #[tokio::test]
    async fn test_add_headers() {
        let plugin = RequestTransformerPlugin::new();
        let mut ctx = make_ctx("GET", "/api");
        let config = serde_json::json!({
            "add": {
                "headers": {
                    "X-Custom": "value1",
                    "X-Another": "value2"
                }
            }
        });

        let result = plugin.execute(Phase::Rewrite, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.request_headers.get("X-Custom").unwrap(), "value1");
        assert_eq!(ctx.request_headers.get("X-Another").unwrap(), "value2");
    }

    #[tokio::test]
    async fn test_remove_headers() {
        let plugin = RequestTransformerPlugin::new();
        let headers = HashMap::from([
            ("x-remove-me".to_string(), "value".to_string()),
            ("x-keep-me".to_string(), "value".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api", headers);
        let config = serde_json::json!({
            "remove": {
                "headers": ["x-remove-me"]
            }
        });

        let result = plugin.execute(Phase::Rewrite, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert!(ctx.get_header("x-remove-me").is_none());
        assert!(ctx.get_header("x-keep-me").is_some());
    }

    #[tokio::test]
    async fn test_rename_headers() {
        let plugin = RequestTransformerPlugin::new();
        let headers = HashMap::from([
            ("X-Old-Name".to_string(), "value123".to_string()),
        ]);
        let mut ctx = make_ctx_with_headers("GET", "/api", headers);
        let config = serde_json::json!({
            "rename": {
                "headers": {
                    "X-Old-Name": "X-New-Name"
                }
            }
        });

        let result = plugin.execute(Phase::Rewrite, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert!(ctx.get_header("X-Old-Name").is_none());
        assert_eq!(ctx.request_headers.get("X-New-Name").unwrap(), "value123");
    }

    #[test]
    fn test_request_transformer_metadata() {
        let plugin = RequestTransformerPlugin::new();
        assert_eq!(plugin.name(), "request-transformer");
        assert_eq!(plugin.priority(), 3000);
        assert_eq!(plugin.phases(), vec![Phase::Rewrite]);
    }
}

// =============================================================================
// Response Transformer Plugin Tests
// =============================================================================
mod response_transformer_tests {
    use super::*;
    use ando_plugins::transform::response_transformer::ResponseTransformerPlugin;

    #[tokio::test]
    async fn test_add_response_headers() {
        let plugin = ResponseTransformerPlugin::new();
        let mut ctx = make_ctx("GET", "/api");
        let config = serde_json::json!({
            "add": {
                "headers": {
                    "X-Powered-By": "Ando",
                    "X-Version": "1.0"
                }
            }
        });

        let result = plugin.execute(Phase::HeaderFilter, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.response_headers.get("X-Powered-By").unwrap(), "Ando");
        assert_eq!(ctx.response_headers.get("X-Version").unwrap(), "1.0");
    }

    #[tokio::test]
    async fn test_remove_response_headers() {
        let plugin = ResponseTransformerPlugin::new();
        let mut ctx = make_ctx("GET", "/api");
        ctx.response_headers.insert("server".to_string(), "nginx".to_string());
        ctx.response_headers.insert("x-keep".to_string(), "yes".to_string());

        let config = serde_json::json!({
            "remove": {
                "headers": ["server"]
            }
        });

        let result = plugin.execute(Phase::HeaderFilter, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert!(ctx.response_headers.get("server").is_none());
        assert!(ctx.response_headers.get("x-keep").is_some());
    }

    #[test]
    fn test_response_transformer_metadata() {
        let plugin = ResponseTransformerPlugin::new();
        assert_eq!(plugin.name(), "response-transformer");
        assert_eq!(plugin.priority(), 3001);
        assert_eq!(plugin.phases(), vec![Phase::HeaderFilter]);
    }
}

// =============================================================================
// Limit Count Plugin Tests
// =============================================================================
mod limit_count_tests {
    use super::*;
    use ando_plugins::traffic::limit_count::LimitCountPlugin;

    #[tokio::test]
    async fn test_limit_count_allows_within_limit() {
        let plugin = LimitCountPlugin::new();
        let config = serde_json::json!({
            "count": 5,
            "time_window": 60
        });

        for _ in 0..5 {
            let mut ctx = make_ctx("GET", "/api");
            let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
            assert!(matches!(result, PluginResult::Continue));
        }
    }

    #[tokio::test]
    async fn test_limit_count_rejects_over_limit() {
        let plugin = LimitCountPlugin::new();
        let config = serde_json::json!({
            "count": 3,
            "time_window": 60
        });

        // Make 3 requests (should all pass)
        for _ in 0..3 {
            let mut ctx = make_ctx("GET", "/api");
            let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
            assert!(matches!(result, PluginResult::Continue));
        }

        // 4th request should be rejected
        let mut ctx = make_ctx("GET", "/api");
        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, headers, .. } => {
                assert_eq!(status, 429);
                assert!(headers.contains_key("X-RateLimit-Limit"));
                assert!(headers.contains_key("Retry-After"));
            }
            _ => panic!("Expected 429 response"),
        }
    }

    #[tokio::test]
    async fn test_limit_count_sets_rate_limit_headers() {
        let plugin = LimitCountPlugin::new();
        let config = serde_json::json!({
            "count": 10,
            "time_window": 60
        });

        let mut ctx = make_ctx("GET", "/api");
        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
        assert_eq!(ctx.response_headers.get("X-RateLimit-Limit").unwrap(), "10");
        assert_eq!(ctx.response_headers.get("X-RateLimit-Remaining").unwrap(), "9");
    }

    #[tokio::test]
    async fn test_limit_count_custom_rejected_code() {
        let plugin = LimitCountPlugin::new();
        let config = serde_json::json!({
            "count": 1,
            "time_window": 60,
            "rejected_code": 503
        });

        let mut ctx = make_ctx("GET", "/api");
        plugin.execute(Phase::Access, &mut ctx, &config).await;

        let mut ctx = make_ctx("GET", "/api");
        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        match result {
            PluginResult::Response { status, .. } => assert_eq!(status, 503),
            _ => panic!("Expected 503 response"),
        }
    }

    #[test]
    fn test_limit_count_metadata() {
        let plugin = LimitCountPlugin::new();
        assert_eq!(plugin.name(), "limit-count");
        assert_eq!(plugin.priority(), 1002);
        assert_eq!(plugin.phases(), vec![Phase::Access]);
    }
}

// =============================================================================
// Limit Req Plugin Tests
// =============================================================================
mod limit_req_tests {
    use super::*;
    use ando_plugins::traffic::limit_req::LimitReqPlugin;

    #[tokio::test]
    async fn test_limit_req_allows_requests() {
        let plugin = LimitReqPlugin::new();
        let mut ctx = make_ctx("GET", "/api");
        let config = serde_json::json!({});

        let result = plugin.execute(Phase::Access, &mut ctx, &config).await;
        assert!(matches!(result, PluginResult::Continue));
    }

    #[test]
    fn test_limit_req_metadata() {
        let plugin = LimitReqPlugin::new();
        assert_eq!(plugin.name(), "limit-req");
        assert_eq!(plugin.priority(), 1001);
        assert_eq!(plugin.phases(), vec![Phase::Access]);
    }
}

// =============================================================================
// Plugin Registration Tests
// =============================================================================
mod registration_tests {
    use ando_plugin::registry::PluginRegistry;
    use ando_plugins::register_all;

    #[test]
    fn test_register_all_plugins() {
        let registry = PluginRegistry::new();
        register_all(&registry);

        assert!(registry.contains("key-auth"));
        assert!(registry.contains("jwt-auth"));
        assert!(registry.contains("basic-auth"));
        assert!(registry.contains("limit-count"));
        assert!(registry.contains("limit-req"));
        assert!(registry.contains("cors"));
        assert!(registry.contains("request-transformer"));
        assert!(registry.contains("response-transformer"));
        assert!(registry.contains("ip-restriction"));

        // At least 9 plugins
        assert!(registry.count() >= 9);
    }
}
