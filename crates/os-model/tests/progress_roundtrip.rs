//! Serde-Rundreise von `Progress` mit den Zusatzfeldern aus M1-P00c (docs/plan/M1.md 3.9).

use os_model::{CrawlState, EndReason, MemoryStage, Progress, Timestamp};

fn sample() -> Progress {
    Progress {
        memory_used_bytes: None,
        memory_limit_bytes: None,
        state: CrawlState::Completed,
        end_reason: Some(EndReason::QueueEmpty),
        crawled: 120,
        queued: 0,
        percent: 100,
        rate: 11.5,
        avg_response_time: Some(0.08),
        elapsed_s: 40,
        eta_s: None,
        active_threads: 0,
        started_at: Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap(),
        avg_rate: 4.0,
        paused_s: 30,
        discovered: 140,
        memory_stage: None,
        file_bytes: None,
    }
}

// spec: CRAWL-130
#[test]
fn progress_additional_fields_roundtrip_losslessly() {
    let p = sample();
    let json = serde_json::to_string(&p).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    // Pflichtfelder von CRAWL-130 unverändert vorhanden, Zusatzfelder ergänzt.
    for key in [
        "crawled",
        "queued",
        "percent",
        "rate",
        "avg_response_time",
        "elapsed_s",
        "eta_s",
        "state",
        "avg_rate",
        "paused_s",
        "discovered",
    ] {
        assert!(v.get(key).is_some(), "Feld {key} fehlt");
    }
    assert_eq!(v["avg_rate"], 4.0);
    assert_eq!(v["paused_s"], 30);
    assert_eq!(v["discovered"], 140);
    let back: Progress = serde_json::from_str(&json).unwrap();
    assert_eq!(back, p);
}

// spec: CRAWL-130
#[test]
fn progress_without_additional_fields_still_deserializes() {
    let mut v = serde_json::to_value(sample()).unwrap();
    let obj = v.as_object_mut().unwrap();
    for key in ["avg_rate", "paused_s", "discovered"] {
        obj.remove(key);
    }
    let back: Progress = serde_json::from_value(v).unwrap();
    assert_eq!((back.avg_rate, back.paused_s, back.discovered), (0.0, 0, 0));
}

// spec: CRAWL-129
#[test]
fn progress_memory_stage_and_file_bytes_roundtrip_and_default_to_none() {
    let mut p = sample();
    p.memory_stage = Some(MemoryStage::Warning);
    p.file_bytes = Some(123_456_789);
    let json = serde_json::to_string(&p).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["memory_stage"], "warning");
    assert_eq!(v["file_bytes"], 123_456_789u64);
    assert_eq!(serde_json::from_str::<Progress>(&json).unwrap(), p);

    // Ältere Ausgaben (vor M2) ohne die beiden Felder bleiben lesbar.
    let mut old = serde_json::to_value(sample()).unwrap();
    let obj = old.as_object_mut().unwrap();
    obj.remove("memory_stage");
    obj.remove("file_bytes");
    let back: Progress = serde_json::from_value(old).unwrap();
    assert_eq!((back.memory_stage, back.file_bytes), (None, None));
}
