use ando_core::upstream::{LoadBalancerType, Upstream};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Load balancer implementation supporting multiple strategies.
pub struct LoadBalancer {
    strategy: LoadBalancerType,
    nodes: Vec<WeightedNode>,
    counter: AtomicUsize,
}

struct WeightedNode {
    addr: String,
    weight: u32,
}

impl LoadBalancer {
    pub fn new(upstream: &Upstream) -> Self {
        let mut nodes: Vec<WeightedNode> = upstream
            .nodes
            .iter()
            .map(|(addr, weight)| WeightedNode {
                addr: addr.clone(),
                weight: *weight,
            })
            .collect();

        // Sort by weight descending for deterministic ordering
        nodes.sort_by(|a, b| b.weight.cmp(&a.weight));

        Self {
            strategy: upstream.r#type.clone(),
            nodes,
            counter: AtomicUsize::new(0),
        }
    }

    /// Select the next upstream node.
    pub fn select(&self) -> Option<&str> {
        if self.nodes.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalancerType::Roundrobin => self.round_robin(),
            LoadBalancerType::LeastConn => {
                // Simplified: falls back to round-robin for now
                self.round_robin()
            }
            _ => self.round_robin(),
        }
    }

    fn round_robin(&self) -> Option<&str> {
        if self.nodes.is_empty() {
            return None;
        }

        // Weighted round-robin: expand nodes by weight
        let total_weight: u32 = self.nodes.iter().map(|n| n.weight).sum();
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % total_weight as usize;

        let mut cumulative = 0u32;
        for node in &self.nodes {
            cumulative += node.weight;
            if idx < cumulative as usize {
                return Some(&node.addr);
            }
        }

        Some(&self.nodes[0].addr)
    }
}
