// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use commitbee::services::analyzer::DiffHunk;

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
use std::collections::HashMap;
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
use std::path::PathBuf;
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
use std::sync::Arc;

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
use commitbee::domain::SymbolKind;
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
use commitbee::domain::{ChangeStatus, FileCategory, FileChange};
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
use commitbee::services::analyzer::AnalyzerService;
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
))]
use tree_sitter::{Language, Query};

// ─── Helpers ─────────────────────────────────────────────────────────────────

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
fn make_file_change(path: &str, diff: &str, additions: usize, deletions: usize) -> FileChange {
    FileChange {
        path: PathBuf::from(path),
        status: ChangeStatus::Added,
        diff: Arc::from(diff),
        additions,
        deletions,
        category: FileCategory::from_path(&PathBuf::from(path)),
        is_binary: false,
        old_path: None,
        rename_similarity: None,
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

#[cfg(feature = "lang-rust")]
#[test]
fn extract_symbols_rust_function() {
    let diff = "@@ -0,0 +1,3 @@\n+pub fn my_function() {\n+    println!(\"hello\");\n+}\n";
    let change = make_file_change("src/new_module.rs", diff, 3, 0);

    let staged = "pub fn my_function() {\n    println!(\"hello\");\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/new_module.rs"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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

#[cfg(feature = "lang-rust")]
#[test]
fn extract_symbols_rust_struct() {
    let diff = "@@ -0,0 +1,4 @@\n+pub struct MyConfig {\n+    pub name: String,\n+    pub value: i32,\n+}\n";
    let change = make_file_change("src/config_types.rs", diff, 4, 0);

    let staged = "pub struct MyConfig {\n    pub name: String,\n    pub value: i32,\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/config_types.rs"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
#[test]
fn extract_symbols_no_grammar() {
    let diff = "@@ -0,0 +1,2 @@\n+some data\n+more data\n";
    let change = make_file_change("data/file.xyz", diff, 2, 0);

    let staged_map = HashMap::from([(
        PathBuf::from("data/file.xyz"),
        "some data\nmore data\n".to_string(),
    )]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    assert!(
        symbols.is_empty(),
        "unknown file extension should produce no symbols, got {} symbols",
        symbols.len()
    );
}

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-javascript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp",
))]
#[test]
fn extract_symbols_binary_skipped() {
    let diff = "@@ -0,0 +1,3 @@\n+pub fn hidden() {}\n";
    let mut change = make_file_change("src/binary_mod.rs", diff, 1, 0);
    change.is_binary = true;

    let staged_map = HashMap::from([(
        PathBuf::from("src/binary_mod.rs"),
        "pub fn hidden() {}\n".to_string(),
    )]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    assert!(
        symbols.is_empty(),
        "binary files should be skipped, got {} symbols",
        symbols.len()
    );
}

// ─── Query compilation tests ────────────────────────────────────────────────

#[cfg(feature = "lang-rust")]
#[test]
fn query_compiles_for_rust() {
    let lang: Language = tree_sitter_rust::LANGUAGE.into();
    let query_source = include_str!("../src/queries/rust.scm");
    let query = Query::new(&lang, query_source);
    assert!(
        query.is_ok(),
        "Rust query should compile: {:?}",
        query.err()
    );
}

#[cfg(feature = "lang-typescript")]
#[test]
fn query_compiles_for_typescript() {
    let lang: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    let query_source = include_str!("../src/queries/typescript.scm");
    let query = Query::new(&lang, query_source);
    assert!(
        query.is_ok(),
        "TypeScript query should compile: {:?}",
        query.err()
    );
}

#[cfg(feature = "lang-javascript")]
#[test]
fn query_compiles_for_javascript() {
    let lang: Language = tree_sitter_javascript::LANGUAGE.into();
    let query_source = include_str!("../src/queries/javascript.scm");
    let query = Query::new(&lang, query_source);
    assert!(
        query.is_ok(),
        "JavaScript query should compile: {:?}",
        query.err()
    );
}

#[cfg(feature = "lang-python")]
#[test]
fn query_compiles_for_python() {
    let lang: Language = tree_sitter_python::LANGUAGE.into();
    let query_source = include_str!("../src/queries/python.scm");
    let query = Query::new(&lang, query_source);
    assert!(
        query.is_ok(),
        "Python query should compile: {:?}",
        query.err()
    );
}

#[cfg(feature = "lang-go")]
#[test]
fn query_compiles_for_go() {
    let lang: Language = tree_sitter_go::LANGUAGE.into();
    let query_source = include_str!("../src/queries/go.scm");
    let query = Query::new(&lang, query_source);
    assert!(query.is_ok(), "Go query should compile: {:?}", query.err());
}

// ─── Language-specific extraction tests ─────────────────────────────────────

#[cfg(feature = "lang-typescript")]
#[test]
fn extract_symbols_typescript_function() {
    let diff =
        "@@ -0,0 +1,3 @@\n+function greet(name: string): void {\n+  console.log(name);\n+}\n";
    let change = make_file_change("src/greet.ts", diff, 3, 0);

    let staged = "function greet(name: string): void {\n  console.log(name);\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/greet.ts"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let func = symbols
        .iter()
        .find(|s| s.name == "greet")
        .expect("expected a symbol named 'greet'");

    assert_eq!(func.kind, SymbolKind::Function, "expected Function kind");
    assert!(func.is_added, "expected is_added=true for staged content");
}

#[cfg(feature = "lang-typescript")]
#[test]
fn extract_symbols_typescript_class_and_method() {
    let diff =
        "@@ -0,0 +1,5 @@\n+class Greeter {\n+  sayHello() {\n+    return \"hello\";\n+  }\n+}\n";
    let change = make_file_change("src/greeter.ts", diff, 5, 0);

    let staged = "class Greeter {\n  sayHello() {\n    return \"hello\";\n  }\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/greeter.ts"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let cls = symbols
        .iter()
        .find(|s| s.name == "Greeter")
        .expect("expected a symbol named 'Greeter'");
    assert_eq!(cls.kind, SymbolKind::Class, "expected Class kind");

    let method = symbols
        .iter()
        .find(|s| s.name == "sayHello")
        .expect("expected a symbol named 'sayHello'");
    assert_eq!(method.kind, SymbolKind::Method, "expected Method kind");
}

#[cfg(feature = "lang-python")]
#[test]
fn extract_symbols_python_function_and_class() {
    let diff = "@@ -0,0 +1,5 @@\n+def greet():\n+    pass\n+\n+class Greeter:\n+    pass\n";
    let change = make_file_change("src/greet.py", diff, 5, 0);

    let staged = "def greet():\n    pass\n\nclass Greeter:\n    pass\n";

    let staged_map = HashMap::from([(PathBuf::from("src/greet.py"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let func = symbols
        .iter()
        .find(|s| s.name == "greet")
        .expect("expected a symbol named 'greet'");
    assert_eq!(func.kind, SymbolKind::Function, "expected Function kind");
    assert!(
        func.is_public,
        "expected is_public=true for greet (no underscore prefix)"
    );

    let cls = symbols
        .iter()
        .find(|s| s.name == "Greeter")
        .expect("expected a symbol named 'Greeter'");
    assert_eq!(cls.kind, SymbolKind::Class, "expected Class kind");
}

#[cfg(feature = "lang-python")]
#[test]
fn extract_symbols_python_private_function() {
    let diff = "@@ -0,0 +1,2 @@\n+def _helper():\n+    pass\n";
    let change = make_file_change("src/utils.py", diff, 2, 0);

    let staged = "def _helper():\n    pass\n";

    let staged_map = HashMap::from([(PathBuf::from("src/utils.py"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let func = symbols
        .iter()
        .find(|s| s.name == "_helper")
        .expect("expected a symbol named '_helper'");
    assert!(
        !func.is_public,
        "expected is_public=false for _helper (underscore prefix)"
    );
}

#[cfg(feature = "lang-go")]
#[test]
fn extract_symbols_go_function() {
    let diff = "@@ -0,0 +1,5 @@\n+package main\n+\n+func Greet() {\n+}\n+\n";
    let change = make_file_change("main.go", diff, 5, 0);

    let staged = "package main\n\nfunc Greet() {\n}\n\n";

    let staged_map = HashMap::from([(PathBuf::from("main.go"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let func = symbols
        .iter()
        .find(|s| s.name == "Greet")
        .expect("expected a symbol named 'Greet'");
    assert_eq!(func.kind, SymbolKind::Function, "expected Function kind");
    assert!(
        func.is_public,
        "expected is_public=true for Go exported function (uppercase)"
    );
}

#[cfg(feature = "lang-go")]
#[test]
fn extract_symbols_go_unexported_function() {
    let diff = "@@ -0,0 +1,5 @@\n+package main\n+\n+func greet() {\n+}\n+\n";
    let change = make_file_change("main.go", diff, 5, 0);

    let staged = "package main\n\nfunc greet() {\n}\n\n";

    let staged_map = HashMap::from([(PathBuf::from("main.go"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let func = symbols
        .iter()
        .find(|s| s.name == "greet")
        .expect("expected a symbol named 'greet'");
    assert!(
        !func.is_public,
        "expected is_public=false for Go unexported function (lowercase)"
    );
}

#[cfg(feature = "lang-javascript")]
#[test]
fn extract_symbols_javascript_class() {
    let diff = "@@ -0,0 +1,3 @@\n+class MyComponent {\n+  render() {}\n+}\n";
    let change = make_file_change("src/component.js", diff, 3, 0);

    let staged = "class MyComponent {\n  render() {}\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/component.js"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let cls = symbols
        .iter()
        .find(|s| s.name == "MyComponent")
        .expect("expected a symbol named 'MyComponent'");
    assert_eq!(cls.kind, SymbolKind::Class, "expected Class kind");

    let method = symbols
        .iter()
        .find(|s| s.name == "render")
        .expect("expected a symbol named 'render'");
    assert_eq!(method.kind, SymbolKind::Method, "expected Method kind");
}

#[cfg(feature = "lang-rust")]
#[test]
fn extract_symbols_rust_private_function() {
    let diff = "@@ -0,0 +1,3 @@\n+fn private_helper() {\n+    // internal\n+}\n";
    let change = make_file_change("src/internal.rs", diff, 3, 0);

    let staged = "fn private_helper() {\n    // internal\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/internal.rs"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let func = symbols
        .iter()
        .find(|s| s.name == "private_helper")
        .expect("expected a symbol named 'private_helper'");
    assert_eq!(func.kind, SymbolKind::Function, "expected Function kind");
    assert!(
        !func.is_public,
        "expected is_public=false for fn without pub"
    );
}

#[cfg(feature = "lang-rust")]
#[test]
fn extract_symbols_rust_enum() {
    let diff = "@@ -0,0 +1,4 @@\n+pub enum Color {\n+    Red,\n+    Blue,\n+}\n";
    let change = make_file_change("src/types.rs", diff, 4, 0);

    let staged = "pub enum Color {\n    Red,\n    Blue,\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/types.rs"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let enm = symbols
        .iter()
        .find(|s| s.name == "Color")
        .expect("expected a symbol named 'Color'");
    assert_eq!(enm.kind, SymbolKind::Enum, "expected Enum kind");
    assert!(enm.is_public, "expected is_public=true for pub enum");
}

#[cfg(feature = "lang-rust")]
#[test]
fn extract_symbols_rust_trait() {
    let diff = "@@ -0,0 +1,3 @@\n+pub trait Drawable {\n+    fn draw(&self);\n+}\n";
    let change = make_file_change("src/traits.rs", diff, 3, 0);

    let staged = "pub trait Drawable {\n    fn draw(&self);\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/traits.rs"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let trt = symbols
        .iter()
        .find(|s| s.name == "Drawable")
        .expect("expected a symbol named 'Drawable'");
    assert_eq!(trt.kind, SymbolKind::Trait, "expected Trait kind");
    assert!(trt.is_public, "expected is_public=true for pub trait");
}

#[cfg(feature = "lang-rust")]
#[test]
fn extract_symbols_rust_impl() {
    let diff = "@@ -0,0 +1,5 @@\n+impl MyStruct {\n+    pub fn new() -> Self {\n+        Self\n+    }\n+}\n";
    let change = make_file_change("src/my_struct.rs", diff, 5, 0);

    let staged = "impl MyStruct {\n    pub fn new() -> Self {\n        Self\n    }\n}\n";

    let staged_map = HashMap::from([(PathBuf::from("src/my_struct.rs"), staged.to_string())]);
    let head_map = HashMap::new();

    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

    let imp = symbols
        .iter()
        .find(|s| s.name == "MyStruct" && s.kind == SymbolKind::Impl)
        .expect("expected an Impl symbol named 'MyStruct'");
    assert_eq!(imp.kind, SymbolKind::Impl, "expected Impl kind");
}
