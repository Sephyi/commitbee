// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! Integration tests for the evaluation harness.
//!
//! Runs all fixtures through the deterministic (no-LLM) pipeline and
//! asserts type inference, evidence flags, prompt content, connections,
//! and breaking change detection.

#![cfg(feature = "eval")]

use std::path::PathBuf;

use commitbee::eval::EvalRunner;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/eval")
}

/// Run all fixtures and assert every one passes.
#[test]
fn all_fixtures_pass() {
    let runner = EvalRunner::new(fixtures_dir(), None);
    let results = runner.run_sync().expect("eval runner should not error");

    assert!(!results.is_empty(), "should discover at least one fixture");

    let mut failures = Vec::new();
    for result in &results {
        if !result.passed() {
            let mut detail = format!("FIXTURE FAILED: {}\n", result.fixture_name);
            if !result.type_passed {
                detail.push_str(&format!(
                    "  Type: expected={}, actual={}\n",
                    result.expected_type, result.actual_type
                ));
            }
            if !result.scope_passed {
                detail.push_str(&format!(
                    "  Scope: expected={:?}, actual={:?}\n",
                    result.expected_scope, result.actual_scope
                ));
            }
            if !result.prompt_assembled {
                detail.push_str("  Prompt: failed to assemble\n");
            }
            for failure in &result.assertion_failures {
                detail.push_str(&format!("  {}\n", failure));
            }
            if let Some(ref err) = result.error {
                detail.push_str(&format!("  Error: {}\n", err));
            }
            failures.push(detail);
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {} fixtures failed:\n\n{}",
            failures.len(),
            results.len(),
            failures.join("\n")
        );
    }
}

/// Each fixture category runs independently.
#[test]
fn type_inference_fixtures() {
    let runner = EvalRunner::new(fixtures_dir(), None);
    let results = runner.run_sync().expect("eval runner should not error");

    for result in &results {
        assert!(
            result.type_passed,
            "Type mismatch in {}: expected={}, actual={}",
            result.fixture_name, result.expected_type, result.actual_type
        );
    }
}

#[test]
fn evidence_flag_fixtures() {
    let runner = EvalRunner::new(fixtures_dir(), None);
    let results = runner.run_sync().expect("eval runner should not error");

    for result in &results {
        let evidence_failures: Vec<_> = result
            .assertion_failures
            .iter()
            .filter(|f| f.category == "evidence")
            .collect();

        assert!(
            evidence_failures.is_empty(),
            "Evidence failures in {}: {:?}",
            result.fixture_name,
            evidence_failures
                .iter()
                .map(|f| &f.message)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn prompt_content_fixtures() {
    let runner = EvalRunner::new(fixtures_dir(), None);
    let results = runner.run_sync().expect("eval runner should not error");

    for result in &results {
        assert!(
            result.prompt_assembled,
            "Prompt assembly failed for {}",
            result.fixture_name
        );

        let prompt_failures: Vec<_> = result
            .assertion_failures
            .iter()
            .filter(|f| f.category == "prompt")
            .collect();

        assert!(
            prompt_failures.is_empty(),
            "Prompt content failures in {}: {:?}",
            result.fixture_name,
            prompt_failures
                .iter()
                .map(|f| &f.message)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn connection_detection_fixtures() {
    let runner = EvalRunner::new(fixtures_dir(), None);
    let results = runner.run_sync().expect("eval runner should not error");

    for result in &results {
        let conn_failures: Vec<_> = result
            .assertion_failures
            .iter()
            .filter(|f| f.category == "connections")
            .collect();

        assert!(
            conn_failures.is_empty(),
            "Connection failures in {}: {:?}",
            result.fixture_name,
            conn_failures.iter().map(|f| &f.message).collect::<Vec<_>>()
        );
    }
}

#[test]
fn breaking_change_fixtures() {
    let runner = EvalRunner::new(fixtures_dir(), None);
    let results = runner.run_sync().expect("eval runner should not error");

    for result in &results {
        let breaking_failures: Vec<_> = result
            .assertion_failures
            .iter()
            .filter(|f| f.category == "breaking")
            .collect();

        assert!(
            breaking_failures.is_empty(),
            "Breaking change failures in {}: {:?}",
            result.fixture_name,
            breaking_failures
                .iter()
                .map(|f| &f.message)
                .collect::<Vec<_>>()
        );
    }
}

/// Verify specific fixture count to catch accidental fixture deletion.
#[test]
fn fixture_count() {
    let runner = EvalRunner::new(fixtures_dir(), None);
    let results = runner.run_sync().expect("eval runner should not error");
    // 2 original (simple-feat, style-only) + 10 new = 12
    assert!(
        results.len() >= 12,
        "Expected at least 12 fixtures, found {}",
        results.len()
    );
}
