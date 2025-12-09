#![cfg(feature = "sim")]

use atomic_execution_bundle::{sim::Scenario, AtomicExecutionBundle, DenyCounters};
use std::fs::File;
use std::io::BufReader;

#[test]
fn executes_fixture_scenario() {
    let file = File::open("tests/fixtures/topstep_scenario.json").expect("fixture");
    let reader = BufReader::new(file);
    let scenario: Scenario = serde_json::from_reader(reader).expect("scenario json");

    let capsule = AtomicExecutionBundle::new();
    let counters = DenyCounters::new();
    let snapshot = scenario
        .execute(&capsule, &counters)
        .expect("scenario execute");

    assert_eq!(snapshot.accepts, 2);
    assert_eq!(snapshot.stale, 1);
    assert_eq!(snapshot.seq_mismatch, 0);
}
