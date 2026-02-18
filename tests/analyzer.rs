// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::{Path, PathBuf};

use commitbee::domain::{ChangeStatus, FileCategory, FileChange, SymbolKind};
use commitbee::services::analyzer::{AnalyzerService, DiffHunk};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_file_change(path: &str, diff: &str, additions: usize, deletions: usize) -> FileChange {
    FileChange {
        path: PathBuf::from(path),
        status: ChangeStatus::Added,
        diff: diff.to_string(),
        additions,
        deletions,
        category: FileCategory::from_path(&PathBuf::from(path)),
        is_binary: false,
    }
}

// ─── DiffHunk parsing tests ─────────────────────────────────────────────────

#[test]
fn parse_hunk_standard() {
    let diff = "@@ -10,5 +12,7 @@\n some code here\n";
    let hunks = DiffHunk::parse_from_diff(diff);

    assert_eq!(hunks.len(), 1, "expected exactly one hunk");
    assert_eq!(hunks[0].old_start, 10);
    assert_eq!(hunks[0].old_count, 5);
    assert_eq!(hunks[0].new_start, 12);
    assert_eq!(hunks[0].new_count, 7);
}

#[test]
fn parse_hunk_single_line() {
    let diff = "@@ -1 +1 @@\n";
    let hunks = DiffHunk::parse_from_diff(diff);

    assert_eq!(hunks.len(), 1, "expected exactly one hunk");
    assert_eq!(hunks[0].old_start, 1);
    assert_eq!(hunks[0].old_count, 1, "missing count should default to 1");
    assert_eq!(hunks[0].new_start, 1);
    assert_eq!(hunks[0].new_count, 1, "missing count should default to 1");
}

#[test]
fn parse_hunk_empty_diff() {
    let hunks = DiffHunk::parse_from_diff("");
    assert!(hunks.is_empty(), "empty diff should produce no hunks");
}

#[test]
fn parse_hunk_no_hunks() {
    let diff = "just some code\nmore code\nnothing special here";
    let hunks = DiffHunk::parse_from_diff(diff);
    assert!(
        hunks.is_empty(),
        "text without @@ markers should produce no hunks"
    );
}

#[test]
fn parse_hunk_multiple() {
    let diff = "\
diff --git a/src/lib.rs b/src/lib.rs
@@ -1,3 +1,4 @@
 use std::io;
+use std::path::Path;
 fn main() {}
@@ -20,5 +21,8 @@
 // section two
+fn helper() {}
@@ -50,2 +54,6 @@
 // section three
+fn another() {}
";
    let hunks = DiffHunk::parse_from_diff(diff);

    assert_eq!(hunks.len(), 3, "expected 3 hunks");

    assert_eq!(hunks[0].old_start, 1);
    assert_eq!(hunks[0].old_count, 3);
    assert_eq!(hunks[0].new_start, 1);
    assert_eq!(hunks[0].new_count, 4);

    assert_eq!(hunks[1].old_start, 20);
    assert_eq!(hunks[1].old_count, 5);
    assert_eq!(hunks[1].new_start, 21);
    assert_eq!(hunks[1].new_count, 8);

    assert_eq!(hunks[2].old_start, 50);
    assert_eq!(hunks[2].old_count, 2);
    assert_eq!(hunks[2].new_start, 54);
    assert_eq!(hunks[2].new_count, 6);
}

// ─── DiffHunk intersection tests ────────────────────────────────────────────

#[test]
fn intersects_new_within() {
    let hunk = DiffHunk {
        old_start: 0,
        old_count: 0,
        new_start: 10,
        new_count: 5,
    };
    // Range (11,14) is fully inside [10, 15)
    assert!(
        hunk.intersects_new(11, 14),
        "range (11,14) should intersect hunk at new_start=10, new_count=5"
    );
}

#[test]
fn intersects_new_outside() {
    let hunk = DiffHunk {
        old_start: 0,
        old_count: 0,
        new_start: 10,
        new_count: 5,
    };
    // Range (20,25) is entirely outside [10, 15)
    assert!(
        !hunk.intersects_new(20, 25),
        "range (20,25) should not intersect hunk at new_start=10, new_count=5"
    );
}

#[test]
fn intersects_old_boundary() {
    let hunk = DiffHunk {
        old_start: 10,
        old_count: 5,
        new_start: 0,
        new_count: 0,
    };
    // Range (10,15) overlaps [10, 15) — should intersect
    assert!(
        hunk.intersects_old(10, 15),
        "range (10,15) at exact boundary should intersect hunk at old_start=10, old_count=5"
    );
    // Range (15,20) starts exactly at hunk_end=15 — should NOT intersect (non-inclusive end)
    assert!(
        !hunk.intersects_old(15, 20),
        "range (15,20) should not intersect hunk ending at 15 (non-inclusive end)"
    );
}

// ─── AnalyzerService tests ──────────────────────────────────────────────────

#[test]
fn extract_symbols_rust_function() {
    let diff = "@@ -0,0 +1,3 @@\n+pub fn my_function() {\n+    println!(\"hello\");\n+}\n";
    let change = make_file_change("src/new_module.rs", diff, 3, 0);

    let staged = "pub fn my_function() {\n    println!(\"hello\");\n}\n";

    let staged_content = |_: &Path| -> Option<String> { Some(staged.to_string()) };
    let head_content = |_: &Path| -> Option<String> { None };

    let mut analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_content, &head_content);

    assert!(
        !symbols.is_empty(),
        "expected at least one symbol from Rust function"
    );

    let func = symbols
        .iter()
        .find(|s| s.name == "my_function")
        .expect("expected a symbol named 'my_function'");

    assert_eq!(func.kind, SymbolKind::Function, "expected Function kind");
    assert!(func.is_public, "expected is_public=true for pub fn");
    assert!(func.is_added, "expected is_added=true for staged content");
}

#[test]
fn extract_symbols_rust_struct() {
    let diff = "@@ -0,0 +1,4 @@\n+pub struct MyConfig {\n+    pub name: String,\n+    pub value: i32,\n+}\n";
    let change = make_file_change("src/config_types.rs", diff, 4, 0);

    let staged = "pub struct MyConfig {\n    pub name: String,\n    pub value: i32,\n}\n";

    let staged_content = |_: &Path| -> Option<String> { Some(staged.to_string()) };
    let head_content = |_: &Path| -> Option<String> { None };

    let mut analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_content, &head_content);

    assert!(
        !symbols.is_empty(),
        "expected at least one symbol from Rust struct"
    );

    let strct = symbols
        .iter()
        .find(|s| s.name == "MyConfig")
        .expect("expected a symbol named 'MyConfig'");

    assert_eq!(strct.kind, SymbolKind::Struct, "expected Struct kind");
    assert!(strct.is_public, "expected is_public=true for pub struct");
    assert!(strct.is_added, "expected is_added=true for staged content");
}

#[test]
fn extract_symbols_no_grammar() {
    let diff = "@@ -0,0 +1,2 @@\n+some data\n+more data\n";
    let change = make_file_change("data/file.xyz", diff, 2, 0);

    let staged_content =
        |_: &Path| -> Option<String> { Some("some data\nmore data\n".to_string()) };
    let head_content = |_: &Path| -> Option<String> { None };

    let mut analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_content, &head_content);

    assert!(
        symbols.is_empty(),
        "unknown file extension should produce no symbols, got {} symbols",
        symbols.len()
    );
}

#[test]
fn extract_symbols_binary_skipped() {
    let diff = "@@ -0,0 +1,3 @@\n+pub fn hidden() {}\n";
    let mut change = make_file_change("src/binary_mod.rs", diff, 1, 0);
    change.is_binary = true;

    let staged_content = |_: &Path| -> Option<String> { Some("pub fn hidden() {}\n".to_string()) };
    let head_content = |_: &Path| -> Option<String> { None };

    let mut analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_content, &head_content);

    assert!(
        symbols.is_empty(),
        "binary files should be skipped, got {} symbols",
        symbols.len()
    );
}
