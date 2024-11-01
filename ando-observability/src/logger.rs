use ando_core::config::VictoriaLogsConfig;
use chrono::Utc;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

/// Exports logs to VictoriaLogs via the JSON line stream API.
///
/// Logs are buffered and flushed in batches for efficiency.
pub struct VictoriaLogsExporter {
    sender: mpsc::Sender<serde_json::Value>,
}

impl VictoriaLogsExporter {
    /// Create a new exporter and start the background flush task.
    pub fn new(config: VictoriaLogsConfig) -> Self {
        let (tx, rx) = mpsc::channel(10_000);

        if config.enabled {
            tokio::spawn(Self::flush_loop(config, rx));
        }

        Self { sender: tx }
    }

    /// Send a log entry (non-blocking).
    pub fn log(&self, entry: serde_json::Value) {
        if let Err(e) = self.sender.try_send(entry) {
            warn!("Log buffer full, dropping entry: {}", e);
        }
    }

    /// Send an access log entry.
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
        self.log(entry);
    }

    /// Send an error log entry.
    pub fn error_log(&self, message: &str, route_id: Option<&str>, error: &str) {
        let entry = json!({
            "_msg": message,
            "_time": Utc::now().to_rfc3339(),
            "level": "error",
            "type": "error",
            "route_id": route_id,
            "error": error,
        });
        self.log(entry);
    }

    /// Background flush loop.
    async fn flush_loop(config: VictoriaLogsConfig, mut rx: mpsc::Receiver<serde_json::Value>) {
        info!(
            endpoint = %config.endpoint,
            batch_size = config.batch_size,
            flush_interval = config.flush_interval_secs,
            "Starting VictoriaLogs flush loop"
        );

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

    /// Flush a batch of logs to VictoriaLogs.
    async fn flush(
        client: &reqwest::Client,
        endpoint: &str,
        batch: &mut Vec<serde_json::Value>,
    ) {
        if batch.is_empty() {
            return;
        }

        // Build NDJSON (newline-delimited JSON)
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
            Ok(resp) => {
                if resp.status().is_success() {
                    debug!(count = count, "Flushed logs to VictoriaLogs");
                } else {
                    error!(
                        status = %resp.status(),
                        count = count,
                        "VictoriaLogs flush failed"
                    );
                }
            }
            Err(e) => {
                error!(error = %e, count = count, "VictoriaLogs flush error");
            }
        }

        batch.clear();
    }
}
