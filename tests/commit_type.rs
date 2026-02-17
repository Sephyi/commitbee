// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use commitbee::domain::CommitType;

#[test]
fn all_matches_enum_variants() {
    assert_eq!(CommitType::ALL.len(), 11);
    for s in CommitType::ALL {
        assert!(
            CommitType::parse(s).is_some(),
            "ALL entry {:?} has no matching parse result",
            s
        );
    }
}

#[test]
fn parse_roundtrips() {
    for s in CommitType::ALL {
        let ct = CommitType::parse(s).unwrap();
        assert_eq!(
            ct.as_str(),
            *s,
            "roundtrip failed for {:?}: as_str() returned {:?}",
            s,
            ct.as_str()
        );
    }
}

#[test]
fn parse_rejects_invalid() {
    for invalid in &["yolo", "", "FEAT"] {
        assert!(
            CommitType::parse(invalid).is_none(),
            "expected None for {:?}, but got Some",
            invalid
        );
    }
}

#[test]
fn display_matches_as_str() {
    assert_eq!(format!("{}", CommitType::Feat), "feat");

    for s in CommitType::ALL {
        let ct = CommitType::parse(s).unwrap();
        assert_eq!(
            ct.to_string(),
            ct.as_str(),
            "Display and as_str() differ for {:?}",
            s
        );
    }
}

#[test]
fn all_types_present() {
    let expected = [
        "feat", "fix", "refactor", "chore", "docs", "test", "style", "perf", "build", "ci",
        "revert",
    ];
    for expected_type in &expected {
        assert!(
            CommitType::ALL.contains(expected_type),
            "expected {:?} to be in CommitType::ALL",
            expected_type
        );
    }
}
