use ando_core::upstream::ActiveHealthCheck;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::warn;

/// Track health status of upstream nodes.
pub struct HealthChecker {
    /// Node health status: addr -> is_healthy
    statuses: Arc<RwLock<HashMap<String, bool>>>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            statuses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a node is healthy.
    pub async fn is_healthy(&self, addr: &str) -> bool {
        let statuses = self.statuses.read().await;
        *statuses.get(addr).unwrap_or(&true)
    }

    /// Start active health checking for an upstream.
    pub fn start_active_check(
        &self,
        upstream_id: String,
        nodes: Vec<String>,
        config: ActiveHealthCheck,
    ) -> tokio::task::JoinHandle<()> {
        let statuses = Arc::clone(&self.statuses);

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs_f64(config.timeout))
                .build()
                .unwrap();

            let mut tick = interval(Duration::from_secs(config.interval));

            // Track consecutive success/failure counts
            let mut success_counts: HashMap<String, u32> = HashMap::new();
            let mut failure_counts: HashMap<String, u32> = HashMap::new();

            loop {
                tick.tick().await;

                for node in &nodes {
                    let url = format!("http://{}{}", node, config.http_path);

                    let is_healthy = match client.get(&url).send().await {
                        Ok(resp) => {
                            let status = resp.status().as_u16();
                            config.healthy_statuses.contains(&status)
                        }
                        Err(_) => false,
                    };

                    if is_healthy {
                        let count = success_counts.entry(node.clone()).or_insert(0);
                        *count += 1;
                        failure_counts.insert(node.clone(), 0);

                        if *count >= config.healthy_successes {
                            let mut s = statuses.write().await;
                            s.insert(node.clone(), true);
                        }
                    } else {
                        let count = failure_counts.entry(node.clone()).or_insert(0);
                        *count += 1;
                        success_counts.insert(node.clone(), 0);

                        if *count >= config.unhealthy_failures {
                            warn!(upstream = %upstream_id, node = %node, "Node marked unhealthy");
                            let mut s = statuses.write().await;
                            s.insert(node.clone(), false);
                        }
                    }
                }
            }
        })
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}
