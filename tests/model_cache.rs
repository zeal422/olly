use std::time::{Duration, SystemTime};

use olly::ollama::models::ModelCache;

#[test]
fn cache_refreshes_when_stale() {
    let mut cache = ModelCache::new(Duration::from_secs(30));
    let now = SystemTime::now();
    cache.set_models_at(vec![String::from("llama3.2:latest")], now);

    let stale_point = now + Duration::from_secs(31);
    assert!(cache.should_refresh(stale_point));
}

#[test]
fn cache_is_fresh_before_ttl() {
    let mut cache = ModelCache::new(Duration::from_secs(30));
    let now = SystemTime::now();
    cache.set_models_at(vec![String::from("llama3.2:latest")], now);

    let fresh_point = now + Duration::from_secs(10);
    assert!(!cache.should_refresh(fresh_point));
    assert_eq!(cache.models(), &[String::from("llama3.2:latest")]);
}
