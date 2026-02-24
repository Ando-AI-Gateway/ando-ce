use ando_core::config::VictoriaLogsConfig;
use chrono::Utc;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tracing::{debug, error};

/// VictoriaLogs exporter — true no-op when disabled.
///
/// v2 design: When `enabled = false`, no channel or task is created.
/// The `access_log()` method becomes a branch-predicted no-op.
pub struct VictoriaLogsExporter {
    sender: Option<mpsc::Sender<serde_json::Value>>,
}

impl VictoriaLogsExporter {
    pub fn new(config: VictoriaLogsConfig) -> Self {
        if !config.enabled {
            return Self { sender: None };
        }

        let (tx, rx) = mpsc::channel(10_000);
        tokio::spawn(Self::flush_loop(config, rx));
        Self { sender: Some(tx) }
    }

    /// No-op constructor for disabled logging.
    pub fn disabled() -> Self {
        Self { sender: None }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn access_log(
        &self,
        route_id: &str,
        method: &str,
        uri: &str,
        status: u16,
        latency_ms: f64,
        client_ip: &str,
        upstream_addr: Option<&str>,
    ) {
        if self.sender.is_none() {
            return;
        }
        let entry = json!({
            "_msg": format!("{} {} {} {} {:.2}ms", method, uri, status, client_ip, latency_ms),
            "_time": Utc::now().to_rfc3339(),
            "level": "info",
            "type": "access",
            "route_id": route_id,
            "method": method,
            "uri": uri,
            "status": status,
            "latency_ms": latency_ms,
            "client_ip": client_ip,
            "upstream_addr": upstream_addr,
        });
        if let Some(ref sender) = self.sender {
            let _ = sender.try_send(entry);
        }
    }

    async fn flush_loop(config: VictoriaLogsConfig, mut rx: mpsc::Receiver<serde_json::Value>) {
        let client = reqwest::Client::new();
        let mut batch: Vec<serde_json::Value> = Vec::with_capacity(config.batch_size);
        let mut flush_interval = interval(Duration::from_secs(config.flush_interval_secs));

        loop {
            tokio::select! {
                Some(entry) = rx.recv() => {
                    batch.push(entry);
                    if batch.len() >= config.batch_size {
                        Self::flush(&client, &config.endpoint, &mut batch).await;
                    }
                }
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        Self::flush(&client, &config.endpoint, &mut batch).await;
                    }
                }
            }
        }
    }

    async fn flush(client: &reqwest::Client, endpoint: &str, batch: &mut Vec<serde_json::Value>) {
        if batch.is_empty() {
            return;
        }
        let mut body = String::new();
        for entry in batch.iter() {
            body.push_str(&serde_json::to_string(entry).unwrap_or_default());
            body.push('\n');
        }
        let count = batch.len();
        match client
            .post(endpoint)
            .header("Content-Type", "application/stream+json")
            .body(body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                debug!(count, "Flushed logs to VictoriaLogs");
            }
            Ok(resp) => {
                error!(status = %resp.status(), "VictoriaLogs flush failed");
            }
            Err(e) => {
                error!(error = %e, "VictoriaLogs connection error");
            }
        }
        batch.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ando_core::config::VictoriaLogsConfig;

    fn disabled_config() -> VictoriaLogsConfig {
        VictoriaLogsConfig {
            enabled: false,
            endpoint: "http://localhost:9428/insert/jsonline".to_string(),
            batch_size: 100,
            flush_interval_secs: 5,
        }
    }

    fn enabled_config() -> VictoriaLogsConfig {
        VictoriaLogsConfig {
            enabled: true,
            endpoint: "http://localhost:9428/insert/jsonline".to_string(),
            batch_size: 100,
            flush_interval_secs: 5,
        }
    }

    #[test]
    fn disabled_constructor_has_no_sender() {
        let exporter = VictoriaLogsExporter::disabled();
        assert!(exporter.sender.is_none());
    }

    #[test]
    fn new_with_disabled_config_has_no_sender() {
        let exporter = VictoriaLogsExporter::new(disabled_config());
        assert!(exporter.sender.is_none());
    }

    #[test]
    fn access_log_on_disabled_does_not_panic() {
        let exporter = VictoriaLogsExporter::disabled();
        exporter.access_log("route-1", "GET", "/api", 200, 1.5, "127.0.0.1", None);
        exporter.access_log(
            "route-2",
            "POST",
            "/api/users",
            201,
            2.3,
            "10.0.0.1",
            Some("10.0.0.2:8080"),
        );
        exporter.access_log("route-3", "DELETE", "/item/1", 404, 0.1, "::1", None);
    }

    #[tokio::test]
    async fn new_with_enabled_config_has_sender() {
        let exporter = VictoriaLogsExporter::new(enabled_config());
        assert!(exporter.sender.is_some());
    }

    #[tokio::test]
    async fn access_log_on_enabled_does_not_block() {
        let exporter = VictoriaLogsExporter::new(enabled_config());
        // Should not block or panic — try_send returns immediately
        exporter.access_log("r1", "GET", "/health", 200, 0.5, "127.0.0.1", None);
        exporter.access_log(
            "r2",
            "POST",
            "/api",
            404,
            1.1,
            "10.0.0.1",
            Some("10.0.0.2:8080"),
        );
        // Give channel consumer a moment
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }

    #[tokio::test]
    async fn access_log_backpressure_does_not_panic() {
        let exporter = VictoriaLogsExporter::new(enabled_config());
        // Flood the channel (capacity 10_000) — should never panic via try_send
        for i in 0..10_100u32 {
            exporter.access_log(
                "r1",
                "GET",
                "/",
                200,
                0.1,
                &format!("10.0.0.{}", i % 255),
                None,
            );
        }
    }
}
