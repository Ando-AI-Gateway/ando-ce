use ando_core::config::VictoriaLogsConfig;
use chrono::Utc;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
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

    async fn flush(
        client: &reqwest::Client,
        endpoint: &str,
        batch: &mut Vec<serde_json::Value>,
    ) {
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
