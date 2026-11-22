use crate::fence::mermaid::{CACHED, Cache};

#[test]
fn completion_evicts_the_least_recently_used_drawing() {
    let mut cache = Cache::default();
    let limit = CACHED as u64;
    for key in 0..limit {
        cache.finish(key, Err("drawing".into()));
    }
    assert!(cache.get(0).is_some());
    cache.finish(limit, Err("new drawing".into()));
    assert!(cache.get(0).is_some());
    assert!(cache.get(1).is_none());
    assert!(cache.get(limit).is_some());
    assert_eq!(cache.completed.len(), CACHED);
}

#[test]
fn out_of_order_completions_leave_pending_keys_and_bound_completed_drawings() {
    let mut cache = Cache::default();
    let count = (CACHED * 2) as u64;
    for key in 0..count {
        cache.pending.entry(key).or_default();
    }
    for key in (1..count).rev() {
        cache.finish(key, Err("drawing".into()));
        assert!(cache.pending.contains_key(&0));
        assert!(cache.completed.len() <= CACHED);
    }
    cache.finish(0, Err("last drawing".into()));
    assert!(cache.pending.is_empty());
    assert!(cache.get(0).is_some());
    assert_eq!(cache.completed.len(), CACHED);
}
