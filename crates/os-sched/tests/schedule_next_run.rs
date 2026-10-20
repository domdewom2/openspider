//! Terminberechnung `next_run` (OUT-161) gegen Europe/Berlin inkl. Sommerzeit 2026-03-29 und 2026-10-25.

use os_model::Timestamp;
use os_sched::tasks::schedule::next_run;
use os_sched::tasks::{EuZone, FixedZone, Schedule, Zone};
use time::{OffsetDateTime, UtcOffset};

fn at(utc: &str) -> Timestamp {
    Timestamp::from_db(utc).expect("Zeitstempel")
}

fn berlin() -> EuZone {
    EuZone::central_europe()
}

fn daily(time: &str) -> Schedule {
    Schedule::Daily {
        time: time.into(),
        from: None,
    }
}

fn next(schedule: &Schedule, now: &str) -> String {
    next_run(schedule, at(now), &berlin())
        .map(|t| t.to_db())
        .unwrap_or_else(|| "none".into())
}

// spec: OUT-161
#[test]
fn eu_rules_switch_at_01_00_utc_on_the_last_sunday() {
    let z = berlin();
    let offset = |s: &str| z.offset_at(at(s).0).whole_hours();
    assert_eq!(offset("2026-03-29T00:59:59.000Z"), 1);
    assert_eq!(offset("2026-03-29T01:00:00.000Z"), 2);
    assert_eq!(offset("2026-10-25T00:59:59.000Z"), 2);
    assert_eq!(offset("2026-10-25T01:00:00.000Z"), 1);
    assert_eq!(offset("2026-01-15T12:00:00.000Z"), 1);
    assert_eq!(offset("2026-07-15T12:00:00.000Z"), 2);
}

// spec: OUT-161
#[test]
fn daily_0230_runs_exactly_once_on_the_spring_forward_day_at_0330() {
    let s = daily("02:30");
    // 02:30 existiert am 29.03.2026 nicht: der Lauf kommt um 03:30 MESZ = 01:30 UTC.
    assert_eq!(
        next(&s, "2026-03-28T12:00:00.000Z"),
        "2026-03-29T01:30:00.000Z"
    );
    // Danach ist der nächste Termin der 30.03. 02:30 MESZ = 00:30 UTC, nicht noch einmal am 29.03.
    assert_eq!(
        next(&s, "2026-03-29T01:30:00.000Z"),
        "2026-03-30T00:30:00.000Z"
    );
    // Der Vortag läuft normal um 02:30 MEZ = 01:30 UTC.
    assert_eq!(
        next(&s, "2026-03-27T12:00:00.000Z"),
        "2026-03-28T01:30:00.000Z"
    );
}

// spec: OUT-161
#[test]
fn daily_0230_runs_exactly_once_on_the_fall_back_day_at_the_first_occurrence() {
    let s = daily("02:30");
    // 02:30 gibt es zweimal; es zählt das erste Auftreten (MESZ = 00:30 UTC).
    assert_eq!(
        next(&s, "2026-10-24T12:00:00.000Z"),
        "2026-10-25T00:30:00.000Z"
    );
    // Das zweite Auftreten (01:30 UTC) löst keinen weiteren Lauf aus.
    assert_eq!(
        next(&s, "2026-10-25T00:30:00.000Z"),
        "2026-10-26T01:30:00.000Z"
    );
    assert_eq!(
        next(&s, "2026-10-25T00:45:00.000Z"),
        "2026-10-26T01:30:00.000Z"
    );
}

// spec: OUT-161
#[test]
fn weekly_keeps_weekday_and_wall_clock_time() {
    // 2026-10-19 ist ein Montag; Montag/Mittwoch/Freitag 06:00 lokal.
    let s = Schedule::Weekly {
        days: vec![1, 3, 5],
        time: "06:00".into(),
        from: None,
    };
    assert_eq!(
        next(&s, "2026-10-19T12:00:00.000Z"),
        "2026-10-21T04:00:00.000Z"
    );
    // Freitag 23.10. -> nächster Montag 26.10. (nach der Zeitumstellung: MEZ, 05:00 UTC)
    assert_eq!(
        next(&s, "2026-10-23T12:00:00.000Z"),
        "2026-10-26T05:00:00.000Z"
    );
    // Sonntag als 7 und Wochenende.
    let weekend = Schedule::Weekly {
        days: vec![6, 7],
        time: "10:00".into(),
        from: None,
    };
    assert_eq!(
        next(&weekend, "2026-10-19T12:00:00.000Z"),
        "2026-10-24T08:00:00.000Z"
    );
}

// spec: OUT-161
#[test]
fn monthly_day_31_uses_the_last_day_of_shorter_months() {
    let s = Schedule::Monthly {
        day: 31,
        time: "03:00".into(),
        from: None,
    };
    assert_eq!(
        next(&s, "2026-01-01T00:00:00.000Z"),
        "2026-01-31T02:00:00.000Z"
    );
    assert_eq!(
        next(&s, "2026-01-31T02:00:00.000Z"),
        "2026-02-28T02:00:00.000Z"
    );
    assert_eq!(
        next(&s, "2026-02-28T02:00:00.000Z"),
        "2026-03-31T01:00:00.000Z"
    );
    assert_eq!(
        next(&s, "2026-03-31T01:00:00.000Z"),
        "2026-04-30T01:00:00.000Z"
    );
    // Schaltjahr
    assert_eq!(
        next(&s, "2028-01-31T02:00:00.000Z"),
        "2028-02-29T02:00:00.000Z"
    );
    // Dezember -> Januar
    assert_eq!(
        next(&s, "2026-12-31T02:00:00.000Z"),
        "2027-01-31T02:00:00.000Z"
    );
}

// spec: OUT-161
#[test]
fn once_in_the_past_has_no_next_run_and_in_the_future_is_exact() {
    let s = Schedule::Once {
        at: "2026-10-21T03:00:00+02:00".into(),
    };
    assert_eq!(
        next(&s, "2026-10-20T00:00:00.000Z"),
        "2026-10-21T01:00:00.000Z"
    );
    assert_eq!(next(&s, "2026-10-21T01:00:00.000Z"), "none");
    assert_eq!(next(&s, "2026-10-22T00:00:00.000Z"), "none");
}

// spec: OUT-161 [P2]
#[test]
fn hourly_and_every_n_days_step_from_the_start_including_dst() {
    let hourly = Schedule::Hourly {
        start: "2026-03-29T00:30".into(),
    };
    assert_eq!(
        next(&hourly, "2026-03-28T22:00:00.000Z"),
        "2026-03-28T23:30:00.000Z"
    );
    assert_eq!(
        next(&hourly, "2026-03-28T23:30:00.000Z"),
        "2026-03-29T00:30:00.000Z"
    );
    // Stündlich ist ein fester Abstand in absoluter Zeit, auch über die Umstellung.
    assert_eq!(
        next(&hourly, "2026-03-29T00:30:00.000Z"),
        "2026-03-29T01:30:00.000Z"
    );

    let every3 = Schedule::EveryDays {
        every: 3,
        start: "2026-10-01T04:00".into(),
    };
    assert_eq!(
        next(&every3, "2026-10-01T10:00:00.000Z"),
        "2026-10-04T02:00:00.000Z"
    );
    // 25.10. 04:00 liegt nach der Rückstellung: MEZ = 03:00 UTC
    assert_eq!(
        next(&every3, "2026-10-22T10:00:00.000Z"),
        "2026-10-25T03:00:00.000Z"
    );
    assert_eq!(
        next(&every3, "2026-10-25T03:00:00.000Z"),
        "2026-10-28T03:00:00.000Z"
    );
}

// spec: OUT-161
#[test]
fn invalid_schedules_have_no_next_run() {
    for s in [
        daily("25:00"),
        daily("7:00"),
        Schedule::Weekly {
            days: vec![],
            time: "03:00".into(),
            from: None,
        },
        Schedule::Weekly {
            days: vec![8],
            time: "03:00".into(),
            from: None,
        },
        Schedule::Monthly {
            day: 0,
            time: "03:00".into(),
            from: None,
        },
        Schedule::EveryDays {
            every: 0,
            start: "2026-10-01T04:00".into(),
        },
    ] {
        assert_eq!(next(&s, "2026-10-20T00:00:00.000Z"), "none", "{s:?}");
    }
}

// spec: OUT-161
#[test]
fn the_start_date_is_a_lower_bound() {
    let s = Schedule::Daily {
        time: "03:00".into(),
        from: Some("2026-11-02".into()),
    };
    assert_eq!(
        next(&s, "2026-10-20T00:00:00.000Z"),
        "2026-11-02T02:00:00.000Z"
    );
}

// spec: OUT-161
#[test]
fn a_fixed_zone_has_no_dst() {
    let z = FixedZone(UtcOffset::from_hms(5, 30, 0).unwrap());
    let s = daily("12:00");
    let t = next_run(&s, at("2026-06-01T00:00:00.000Z"), &z).unwrap();
    assert_eq!(t.to_db(), "2026-06-01T06:30:00.000Z");
    let local: OffsetDateTime = t.0;
    assert_eq!(local.to_offset(z.0).hour(), 12);
}
