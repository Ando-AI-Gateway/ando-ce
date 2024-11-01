use ando_core::upstream::{LoadBalancerType, Upstream};
use ando_proxy::balancer::LoadBalancer;
use std::collections::HashMap;

fn make_upstream(nodes: HashMap<String, u32>, lb_type: LoadBalancerType) -> Upstream {
    Upstream {
        id: "test".to_string(),
        name: "test".to_string(),
        description: String::new(),
        r#type: lb_type,
        hash_on: None,
        key: None,
        nodes,
        retries: 1,
        retry_timeout: None,
        timeout: None,
        scheme: "http".to_string(),
        pass_host: ando_core::upstream::PassHostMode::Pass,
        upstream_host: None,
        checks: None,
        discovery: None,
        tls: None,
        labels: HashMap::new(),
        created_at: None,
        updated_at: None,
    }
}

// =============================================================================
// Basic Load Balancer Tests
// =============================================================================

#[test]
fn test_load_balancer_empty_nodes() {
    let upstream = make_upstream(HashMap::new(), LoadBalancerType::Roundrobin);
    let lb = LoadBalancer::new(&upstream);
    assert_eq!(lb.select(), None);
}

#[test]
fn test_load_balancer_single_node() {
    let nodes = HashMap::from([("127.0.0.1:8080".to_string(), 1)]);
    let upstream = make_upstream(nodes, LoadBalancerType::Roundrobin);
    let lb = LoadBalancer::new(&upstream);

    // Should always return the single node
    for _ in 0..10 {
        assert_eq!(lb.select(), Some("127.0.0.1:8080"));
    }
}

#[test]
fn test_load_balancer_round_robin_distribution() {
    let nodes = HashMap::from([
        ("node1:80".to_string(), 1),
        ("node2:80".to_string(), 1),
    ]);
    let upstream = make_upstream(nodes, LoadBalancerType::Roundrobin);
    let lb = LoadBalancer::new(&upstream);

    let mut counts: HashMap<String, u32> = HashMap::new();
    for _ in 0..100 {
        let node = lb.select().unwrap();
        *counts.entry(node.to_string()).or_insert(0) += 1;
    }

    // With equal weights, distribution should be roughly 50/50
    assert_eq!(counts.len(), 2);
    assert_eq!(*counts.get("node1:80").unwrap(), 50);
    assert_eq!(*counts.get("node2:80").unwrap(), 50);
}

#[test]
fn test_load_balancer_weighted_distribution() {
    let nodes = HashMap::from([
        ("heavy:80".to_string(), 3),
        ("light:80".to_string(), 1),
    ]);
    let upstream = make_upstream(nodes, LoadBalancerType::Roundrobin);
    let lb = LoadBalancer::new(&upstream);

    let mut counts: HashMap<String, u32> = HashMap::new();
    // Total weight is 4, so cycle every 4 selections
    for _ in 0..40 {
        let node = lb.select().unwrap();
        *counts.entry(node.to_string()).or_insert(0) += 1;
    }

    // heavy should get 3/4 and light should get 1/4
    let heavy_count = *counts.get("heavy:80").unwrap();
    let light_count = *counts.get("light:80").unwrap();
    assert_eq!(heavy_count, 30);
    assert_eq!(light_count, 10);
}

#[test]
fn test_load_balancer_least_conn_falls_back_to_round_robin() {
    let nodes = HashMap::from([
        ("node1:80".to_string(), 1),
        ("node2:80".to_string(), 1),
    ]);
    let upstream = make_upstream(nodes, LoadBalancerType::LeastConn);
    let lb = LoadBalancer::new(&upstream);

    // Should still work (falls back to round-robin)
    let result = lb.select();
    assert!(result.is_some());
}

#[test]
fn test_load_balancer_chash_falls_back_to_round_robin() {
    let nodes = HashMap::from([
        ("node1:80".to_string(), 1),
    ]);
    let upstream = make_upstream(nodes, LoadBalancerType::Chash);
    let lb = LoadBalancer::new(&upstream);

    assert_eq!(lb.select(), Some("node1:80"));
}

#[test]
fn test_load_balancer_ewma_falls_back_to_round_robin() {
    let nodes = HashMap::from([
        ("node1:80".to_string(), 1),
    ]);
    let upstream = make_upstream(nodes, LoadBalancerType::Ewma);
    let lb = LoadBalancer::new(&upstream);

    assert_eq!(lb.select(), Some("node1:80"));
}

#[test]
fn test_load_balancer_three_nodes() {
    let nodes = HashMap::from([
        ("a:80".to_string(), 1),
        ("b:80".to_string(), 1),
        ("c:80".to_string(), 1),
    ]);
    let upstream = make_upstream(nodes, LoadBalancerType::Roundrobin);
    let lb = LoadBalancer::new(&upstream);

    let mut counts: HashMap<String, u32> = HashMap::new();
    for _ in 0..30 {
        let node = lb.select().unwrap();
        *counts.entry(node.to_string()).or_insert(0) += 1;
    }

    // Each should get 10 requests
    assert_eq!(counts.len(), 3);
    for count in counts.values() {
        assert_eq!(*count, 10);
    }
}
