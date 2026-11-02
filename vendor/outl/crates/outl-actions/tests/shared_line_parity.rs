//! `shared_line` answers every case in `shared-line-cases.json`.
//!
//! That file is the contract, not this test: `SharedLine.swift` reads the
//! same cases from `OutlKitTests/SharedLineParityTests.swift`, because an
//! iOS share extension cannot load this library and therefore keeps the
//! only second implementation of the rule (RFC 0363).
//!
//! Two implementations drift, and the direction this one drifts in is a
//! share that silently loses its link. So the cases live in one file and
//! both suites fail on a disagreement, instead of one of them being
//! quietly right.

use std::path::PathBuf;

use outl_actions::capture::shared_line;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    why: String,
    text: Option<String>,
    url: Option<String>,
    line: Option<String>,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/shared-line-cases.json")
}

#[test]
fn shared_line_answers_every_shared_case() {
    let path = fixture_path();
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let fixture: Fixture = serde_json::from_str(&raw).expect("the fixture is valid JSON");

    assert!(
        fixture.cases.len() >= 10,
        "only {} cases in the fixture — a shrunken fixture is a pin that \
         stopped pinning",
        fixture.cases.len()
    );

    for case in &fixture.cases {
        let got = shared_line(case.text.as_deref(), case.url.as_deref());
        assert_eq!(
            got.as_deref(),
            case.line.as_deref(),
            "case {:?} ({}): text={:?} url={:?}",
            case.why,
            path.display(),
            case.text,
            case.url
        );
    }
}
