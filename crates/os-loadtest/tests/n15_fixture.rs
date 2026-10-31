// spec: PERF-003
#[test]
fn n15_slow_store_uses_the_perf001_fixture() {
    use os_loadtest::{scenario, Size};
    for size in [Size::Small, Size::Large] {
        let a = scenario::find("perf-001").unwrap().config(size);
        let b = scenario::find("perf-003").unwrap().config(size);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }
}
