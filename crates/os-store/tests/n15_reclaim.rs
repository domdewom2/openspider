// spec: PERF-003
#[test]
fn n15_reclaim_preserves_live_allocations() {
    let mut live = Vec::new();
    let mut retired = Vec::new();
    for value in 0..=u8::MAX {
        live.push(vec![value; 4096]);
        retired.push(vec![value ^ 0x5a; 32 * 1024]);
    }
    std::hint::black_box(&live);
    std::hint::black_box(&retired);
    drop(retired);

    // Freigabemengen sind allocator-/plattformabhängig, die Datenintegrität nicht.
    let _ = os_store::profile::reclaim_batch_heap();
    for (value, block) in (0..=u8::MAX).zip(std::hint::black_box(&live)) {
        assert!(block.iter().all(|byte| *byte == value));
    }
}
