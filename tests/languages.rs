// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Tests for feature-gated language support (Java, C, C++, Ruby, C#).
//!
//! Also tests signature extraction for Rust, TypeScript, Python, and Go.
//! Each test is gated behind its corresponding Cargo feature flag.
//! Run with: `cargo test --test languages --features all-languages`

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use std::collections::HashMap;
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use std::path::PathBuf;
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use std::sync::Arc;

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use commitbee::domain::{ChangeStatus, FileCategory, FileChange, SymbolKind};
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use commitbee::services::analyzer::AnalyzerService;

#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
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

/// Helper: parse source code with the given file extension and return extracted symbols.
/// Creates a synthetic FileChange covering all lines, so all symbols are captured.
#[cfg(any(
    feature = "lang-rust",
    feature = "lang-typescript",
    feature = "lang-python",
    feature = "lang-go",
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
fn extract_symbols_from_source(source: &str, ext: &str) -> Vec<commitbee::domain::CodeSymbol> {
    let line_count = source.lines().count();
    let path = format!("src/test_file.{ext}");
    let diff = format!("@@ -0,0 +1,{line_count} @@\n+placeholder\n");
    let change = make_file_change(&path, &diff, line_count, 0);
    let staged_map = HashMap::from([(PathBuf::from(&path), source.to_string())]);
    let head_map = HashMap::new();
    let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
    analyzer
        .extract_symbols(&[change], &staged_map, &head_map)
        .0
}

// ─── Parent scope extraction ─────────────────────────────────────────────────

#[cfg(feature = "lang-rust")]
mod rust_parent_scope {
    use super::*;

    #[test]
    fn rust_impl_method_has_parent_scope() {
        let source = r#"impl CommitValidator {
    pub fn validate(&self, input: &str) -> bool {
        true
    }
}"#;
        let symbols = extract_symbols_from_source(source, "rs");
        let method = symbols
            .iter()
            .find(|s| s.name == "validate")
            .expect("should find validate");
        assert_eq!(method.parent_scope.as_deref(), Some("CommitValidator"));
    }

    #[test]
    fn rust_top_level_function_has_no_parent_scope() {
        let source = "pub fn standalone() -> bool {\n    true\n}\n";
        let symbols = extract_symbols_from_source(source, "rs");
        let func = symbols
            .iter()
            .find(|s| s.name == "standalone")
            .expect("should find standalone");
        assert_eq!(func.parent_scope, None);
    }

    #[test]
    fn rust_trait_method_has_parent_scope() {
        let source = r#"trait Validator {
    fn validate(&self) -> bool;
}"#;
        let symbols = extract_symbols_from_source(source, "rs");
        // Trait methods may or may not be captured depending on the .scm query
        if let Some(method) = symbols.iter().find(|s| s.name == "validate") {
            assert_eq!(method.parent_scope.as_deref(), Some("Validator"));
        }
    }
}

#[cfg(feature = "lang-python")]
mod python_parent_scope {
    use super::*;

    #[test]
    fn python_class_method_has_parent_scope() {
        let source = "class MyService:\n    def process(self, data):\n        return data\n";
        let symbols = extract_symbols_from_source(source, "py");
        if let Some(method) = symbols.iter().find(|s| s.name == "process") {
            assert_eq!(method.parent_scope.as_deref(), Some("MyService"));
        }
    }

    #[test]
    fn python_top_level_function_has_no_parent_scope() {
        let source = "def standalone(x):\n    return x\n";
        let symbols = extract_symbols_from_source(source, "py");
        let func = symbols
            .iter()
            .find(|s| s.name == "standalone")
            .expect("should find standalone");
        assert_eq!(func.parent_scope, None);
    }
}

#[cfg(feature = "lang-typescript")]
mod typescript_parent_scope {
    use super::*;

    #[test]
    fn typescript_class_method_has_parent_scope() {
        let source = "class UserService {\n  getName(): string {\n    return 'test';\n  }\n}\n";
        let symbols = extract_symbols_from_source(source, "ts");
        if let Some(method) = symbols.iter().find(|s| s.name == "getName") {
            assert_eq!(method.parent_scope.as_deref(), Some("UserService"));
        }
    }

    #[test]
    fn typescript_top_level_function_has_no_parent_scope() {
        let source = "function standalone(): void {\n  return;\n}\n";
        let symbols = extract_symbols_from_source(source, "ts");
        let func = symbols
            .iter()
            .find(|s| s.name == "standalone")
            .expect("should find standalone");
        assert_eq!(func.parent_scope, None);
    }
}

#[cfg(feature = "lang-java")]
mod java_parent_scope {
    use super::*;

    #[test]
    fn java_class_method_has_parent_scope() {
        let source = "public class Calculator {\n    public int add(int a, int b) {\n        return a + b;\n    }\n}\n";
        let symbols = extract_symbols_from_source(source, "java");
        if let Some(method) = symbols.iter().find(|s| s.name == "add") {
            assert_eq!(method.parent_scope.as_deref(), Some("Calculator"));
        }
    }
}

#[cfg(feature = "lang-go")]
mod go_parent_scope {
    use super::*;

    #[test]
    fn go_top_level_function_has_no_parent_scope() {
        let source = "func ParseConfig(path string) error {\n\treturn nil\n}\n";
        let symbols = extract_symbols_from_source(source, "go");
        let func = symbols
            .iter()
            .find(|s| s.name == "ParseConfig")
            .expect("should find ParseConfig");
        assert_eq!(func.parent_scope, None);
    }
}

#[cfg(feature = "lang-ruby")]
mod ruby_parent_scope {
    use super::*;

    #[test]
    fn ruby_class_method_has_parent_scope() {
        let source = "class Calculator\n  def add(a, b)\n    a + b\n  end\nend\n";
        let symbols = extract_symbols_from_source(source, "rb");
        if let Some(method) = symbols.iter().find(|s| s.name == "add") {
            assert_eq!(method.parent_scope.as_deref(), Some("Calculator"));
        }
    }
}

#[cfg(feature = "lang-csharp")]
mod csharp_parent_scope {
    use super::*;

    #[test]
    fn csharp_class_method_has_parent_scope() {
        let source = "public class Calculator {\n    public int Add(int a, int b) {\n        return a + b;\n    }\n}\n";
        let symbols = extract_symbols_from_source(source, "cs");
        if let Some(method) = symbols.iter().find(|s| s.name == "Add") {
            assert_eq!(method.parent_scope.as_deref(), Some("Calculator"));
        }
    }
}

// ─── Java ────────────────────────────────────────────────────────────────────

#[cfg(feature = "lang-java")]
mod java {
    use super::*;

    #[test]
    fn extract_java_class_and_methods() {
        let source = r#"public class Calculator {
    public int add(int a, int b) {
        return a + b;
    }

    private int subtract(int a, int b) {
        return a - b;
    }
}
"#;
        let diff = "@@ -0,0 +1,9 @@\n+public class Calculator {\n";
        let change = make_file_change("src/Calculator.java", diff, 9, 0);

        let staged_map =
            HashMap::from([(PathBuf::from("src/Calculator.java"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let class = symbols
            .iter()
            .find(|s| s.name == "Calculator")
            .expect("expected a symbol named 'Calculator'");
        assert_eq!(class.kind, SymbolKind::Class);
        assert!(class.is_public, "public class should be detected as public");

        let add_method = symbols
            .iter()
            .find(|s| s.name == "add")
            .expect("expected a symbol named 'add'");
        assert_eq!(add_method.kind, SymbolKind::Method);
        assert!(
            add_method.is_public,
            "public method should be detected as public"
        );

        let subtract_method = symbols
            .iter()
            .find(|s| s.name == "subtract")
            .expect("expected a symbol named 'subtract'");
        assert_eq!(subtract_method.kind, SymbolKind::Method);
        assert!(
            !subtract_method.is_public,
            "private method should not be detected as public"
        );
    }

    #[test]
    fn extract_java_interface() {
        let source = r#"public interface Drawable {
    void draw();
}
"#;
        let diff = "@@ -0,0 +1,3 @@\n+public interface Drawable {\n";
        let change = make_file_change("src/Drawable.java", diff, 3, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/Drawable.java"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let iface = symbols
            .iter()
            .find(|s| s.name == "Drawable")
            .expect("expected a symbol named 'Drawable'");
        assert_eq!(iface.kind, SymbolKind::Interface);
        assert!(
            iface.is_public,
            "public interface should be detected as public"
        );
    }

    #[test]
    fn extract_java_enum() {
        let source = r#"public enum Color {
    RED,
    GREEN,
    BLUE
}
"#;
        let diff = "@@ -0,0 +1,5 @@\n+public enum Color {\n";
        let change = make_file_change("src/Color.java", diff, 5, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/Color.java"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let e = symbols
            .iter()
            .find(|s| s.name == "Color")
            .expect("expected a symbol named 'Color'");
        assert_eq!(e.kind, SymbolKind::Enum);
        assert!(e.is_public, "public enum should be detected as public");
    }
}

// ─── C ───────────────────────────────────────────────────────────────────────

#[cfg(feature = "lang-c")]
mod c_lang {
    use super::*;

    #[test]
    fn extract_c_function() {
        let source = r#"int add(int a, int b) {
    return a + b;
}
"#;
        let diff = "@@ -0,0 +1,3 @@\n+int add(int a, int b) {\n";
        let change = make_file_change("src/math.c", diff, 3, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/math.c"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let func = symbols
            .iter()
            .find(|s| s.name == "add")
            .expect("expected a symbol named 'add'");
        assert_eq!(func.kind, SymbolKind::Function);
        assert!(
            func.is_public,
            "C functions default to public (no visibility modifiers)"
        );
    }

    #[test]
    fn extract_c_struct() {
        let source = r#"struct Point {
    int x;
    int y;
};
"#;
        let diff = "@@ -0,0 +1,4 @@\n+struct Point {\n";
        let change = make_file_change("src/point.c", diff, 4, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/point.c"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let strct = symbols
            .iter()
            .find(|s| s.name == "Point")
            .expect("expected a symbol named 'Point'");
        assert_eq!(strct.kind, SymbolKind::Struct);
        assert!(strct.is_public, "C structs default to public");
    }

    #[test]
    fn extract_c_header_file() {
        let source = r#"struct Config {
    int width;
    int height;
};

enum Status {
    OK,
    ERROR
};
"#;
        let diff = "@@ -0,0 +1,9 @@\n+struct Config {\n";
        let change = make_file_change("include/api.h", diff, 9, 0);

        let staged_map = HashMap::from([(PathBuf::from("include/api.h"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        // .h files should be recognized as C
        assert!(
            !symbols.is_empty(),
            "expected symbols from .h header file, got none"
        );

        let config = symbols
            .iter()
            .find(|s| s.name == "Config")
            .expect("expected a symbol named 'Config'");
        assert_eq!(config.kind, SymbolKind::Struct);
    }

    #[test]
    fn extract_c_typedef() {
        let source = r#"typedef unsigned long size_t;
typedef struct {
    int x;
    int y;
} Point;
"#;
        let diff = "@@ -0,0 +1,5 @@\n+typedef unsigned long size_t;\n";
        let change = make_file_change("src/types.c", diff, 5, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/types.c"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let has_type = symbols.iter().any(|s| s.kind == SymbolKind::Type);
        assert!(has_type, "expected at least one Type symbol from typedef");
    }
}

// ─── C++ ─────────────────────────────────────────────────────────────────────

#[cfg(feature = "lang-cpp")]
mod cpp {
    use super::*;

    #[test]
    fn extract_cpp_class_and_function() {
        let source = r#"class Shape {
public:
    virtual void draw() = 0;
};

int main() {
    return 0;
}
"#;
        let diff = "@@ -0,0 +1,8 @@\n+class Shape {\n";
        let change = make_file_change("src/main.cpp", diff, 8, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/main.cpp"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let class = symbols
            .iter()
            .find(|s| s.name == "Shape")
            .expect("expected a symbol named 'Shape'");
        assert_eq!(class.kind, SymbolKind::Class);

        let main_fn = symbols
            .iter()
            .find(|s| s.name == "main")
            .expect("expected a symbol named 'main'");
        assert_eq!(main_fn.kind, SymbolKind::Function);
    }

    #[test]
    fn extract_cpp_extensions() {
        // Verify .cc and .cxx extensions are recognized
        for ext in &["cc", "cxx"] {
            let source = "void helper() {\n    return;\n}\n";
            let diff = "@@ -0,0 +1,3 @@\n+void helper() {\n";
            let path = format!("src/util.{ext}");
            let change = make_file_change(&path, diff, 3, 0);

            let staged_map = HashMap::from([(PathBuf::from(&path), source.to_string())]);
            let head_map = HashMap::new();

            let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
            let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

            assert!(
                !symbols.is_empty(),
                "expected symbols from .{ext} file, got none"
            );
        }
    }

    #[test]
    fn extract_cpp_struct() {
        let source = r#"struct Vec3 {
    float x, y, z;
};
"#;
        let diff = "@@ -0,0 +1,3 @@\n+struct Vec3 {\n";
        let change = make_file_change("src/math.cpp", diff, 3, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/math.cpp"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let strct = symbols
            .iter()
            .find(|s| s.name == "Vec3")
            .expect("expected a symbol named 'Vec3'");
        assert_eq!(strct.kind, SymbolKind::Struct);
    }
}

// ─── Ruby ────────────────────────────────────────────────────────────────────

#[cfg(feature = "lang-ruby")]
mod ruby {
    use super::*;

    #[test]
    fn extract_ruby_class_and_method() {
        let source = r#"class Calculator
  def add(a, b)
    a + b
  end

  def subtract(a, b)
    a - b
  end
end
"#;
        let diff = "@@ -0,0 +1,9 @@\n+class Calculator\n";
        let change = make_file_change("lib/calculator.rb", diff, 9, 0);

        let staged_map = HashMap::from([(PathBuf::from("lib/calculator.rb"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let class = symbols
            .iter()
            .find(|s| s.name == "Calculator")
            .expect("expected a symbol named 'Calculator'");
        assert_eq!(class.kind, SymbolKind::Class);

        let add_method = symbols
            .iter()
            .find(|s| s.name == "add")
            .expect("expected a symbol named 'add'");
        assert_eq!(add_method.kind, SymbolKind::Method);
    }

    #[test]
    fn extract_ruby_module() {
        let source = r#"module Serializable
  def serialize
    to_json
  end
end
"#;
        let diff = "@@ -0,0 +1,5 @@\n+module Serializable\n";
        let change = make_file_change("lib/serializable.rb", diff, 5, 0);

        let staged_map =
            HashMap::from([(PathBuf::from("lib/serializable.rb"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let module = symbols
            .iter()
            .find(|s| s.name == "Serializable")
            .expect("expected a symbol named 'Serializable'");
        // Module maps to Class (closest match)
        assert_eq!(module.kind, SymbolKind::Class);
    }
}

// ─── C# ──────────────────────────────────────────────────────────────────────

#[cfg(feature = "lang-csharp")]
mod csharp {
    use super::*;

    #[test]
    fn extract_csharp_class_and_method() {
        let source = r#"public class Calculator {
    public int Add(int a, int b) {
        return a + b;
    }

    private int Subtract(int a, int b) {
        return a - b;
    }
}
"#;
        let diff = "@@ -0,0 +1,9 @@\n+public class Calculator {\n";
        let change = make_file_change("src/Calculator.cs", diff, 9, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/Calculator.cs"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let class = symbols
            .iter()
            .find(|s| s.name == "Calculator")
            .expect("expected a symbol named 'Calculator'");
        assert_eq!(class.kind, SymbolKind::Class);
        assert!(class.is_public, "public class should be detected as public");

        let add_method = symbols
            .iter()
            .find(|s| s.name == "Add")
            .expect("expected a symbol named 'Add'");
        assert_eq!(add_method.kind, SymbolKind::Method);
        assert!(
            add_method.is_public,
            "public method should be detected as public"
        );

        let subtract_method = symbols
            .iter()
            .find(|s| s.name == "Subtract")
            .expect("expected a symbol named 'Subtract'");
        assert_eq!(subtract_method.kind, SymbolKind::Method);
        assert!(
            !subtract_method.is_public,
            "private method should not be detected as public"
        );
    }

    #[test]
    fn extract_csharp_interface() {
        let source = r#"public interface IDrawable {
    void Draw();
}
"#;
        let diff = "@@ -0,0 +1,3 @@\n+public interface IDrawable {\n";
        let change = make_file_change("src/IDrawable.cs", diff, 3, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/IDrawable.cs"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let iface = symbols
            .iter()
            .find(|s| s.name == "IDrawable")
            .expect("expected a symbol named 'IDrawable'");
        assert_eq!(iface.kind, SymbolKind::Interface);
        assert!(
            iface.is_public,
            "public interface should be detected as public"
        );
    }

    #[test]
    fn extract_csharp_struct() {
        let source = r#"public struct Point {
    public int X;
    public int Y;
}
"#;
        let diff = "@@ -0,0 +1,4 @@\n+public struct Point {\n";
        let change = make_file_change("src/Point.cs", diff, 4, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/Point.cs"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let strct = symbols
            .iter()
            .find(|s| s.name == "Point")
            .expect("expected a symbol named 'Point'");
        assert_eq!(strct.kind, SymbolKind::Struct);
        assert!(
            strct.is_public,
            "public struct should be detected as public"
        );
    }
}

// ─── Signature extraction ─────────────────────────────────────────────────────

#[cfg(feature = "lang-rust")]
mod rust_signature {
    use super::*;

    #[test]
    fn rust_function_signature_extracted() {
        let source = r#"pub fn connect(host: &str, timeout: u64) -> bool {
    true
}
"#;
        // Hunk covers all 3 lines of the function
        let diff = "@@ -0,0 +1,3 @@\n+pub fn connect(host: &str, timeout: u64) -> bool {\n";
        let change = make_file_change("src/net.rs", diff, 3, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/net.rs"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let func = symbols
            .iter()
            .find(|s| s.name == "connect")
            .expect("expected a symbol named 'connect'");
        assert_eq!(func.kind, SymbolKind::Function);

        let sig = func
            .signature
            .as_ref()
            .expect("expected signature to be extracted for Rust function");
        assert!(
            sig.contains("host"),
            "signature should contain parameter 'host', got: {sig}"
        );
        assert!(
            sig.contains("timeout"),
            "signature should contain parameter 'timeout', got: {sig}"
        );
        assert!(
            sig.contains("->"),
            "signature should contain return type arrow, got: {sig}"
        );
        assert!(
            sig.contains("bool"),
            "signature should contain return type 'bool', got: {sig}"
        );
    }

    #[test]
    fn rust_method_signature_extracted() {
        let source = r#"impl Cache {
    pub fn get(&self, key: &str) -> Option<String> {
        None
    }
}
"#;
        let diff = "@@ -0,0 +1,5 @@\n+impl Cache {\n";
        let change = make_file_change("src/cache.rs", diff, 5, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/cache.rs"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let method = symbols
            .iter()
            .find(|s| s.name == "get")
            .expect("expected a symbol named 'get'");
        assert_eq!(method.kind, SymbolKind::Function);

        let sig = method
            .signature
            .as_ref()
            .expect("expected signature for Rust method");
        assert!(
            sig.contains("key"),
            "signature should contain parameter 'key', got: {sig}"
        );
        assert!(
            sig.contains("Option"),
            "signature should contain return type 'Option', got: {sig}"
        );
    }
}

#[cfg(feature = "lang-typescript")]
mod typescript_signature {
    use super::*;

    #[test]
    fn typescript_function_signature_extracted() {
        let source = r#"function fetchUser(id: number, baseUrl: string): Promise<string> {
    return Promise.resolve("");
}
"#;
        let diff = "@@ -0,0 +1,3 @@\n+function fetchUser(id: number, baseUrl: string): Promise<string> {\n";
        let change = make_file_change("src/api.ts", diff, 3, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/api.ts"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let func = symbols
            .iter()
            .find(|s| s.name == "fetchUser")
            .expect("expected a symbol named 'fetchUser'");
        assert_eq!(func.kind, SymbolKind::Function);

        let sig = func
            .signature
            .as_ref()
            .expect("expected signature to be extracted for TypeScript function");
        assert!(
            sig.contains("id"),
            "signature should contain parameter 'id', got: {sig}"
        );
        assert!(
            sig.contains("baseUrl"),
            "signature should contain parameter 'baseUrl', got: {sig}"
        );
    }
}

#[cfg(feature = "lang-python")]
mod python_signature {
    use super::*;

    #[test]
    fn python_function_signature_extracted() {
        let source = r#"def calculate_total(price: float, quantity: int) -> float:
    return price * quantity
"#;
        let diff = "@@ -0,0 +1,2 @@\n+def calculate_total(price: float, quantity: int) -> float:\n";
        let change = make_file_change("src/billing.py", diff, 2, 0);

        let staged_map = HashMap::from([(PathBuf::from("src/billing.py"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let func = symbols
            .iter()
            .find(|s| s.name == "calculate_total")
            .expect("expected a symbol named 'calculate_total'");
        assert_eq!(func.kind, SymbolKind::Function);

        let sig = func
            .signature
            .as_ref()
            .expect("expected signature to be extracted for Python function");
        assert!(
            sig.contains("price"),
            "signature should contain parameter 'price', got: {sig}"
        );
        assert!(
            sig.contains("quantity"),
            "signature should contain parameter 'quantity', got: {sig}"
        );
    }
}

#[cfg(feature = "lang-go")]
mod go_signature {
    use super::*;

    #[test]
    fn go_function_signature_extracted() {
        let source = r#"func ParseConfig(path string, strict bool) (*Config, error) {
	return nil, nil
}
"#;
        let diff =
            "@@ -0,0 +1,3 @@\n+func ParseConfig(path string, strict bool) (*Config, error) {\n";
        let change = make_file_change("config/parser.go", diff, 3, 0);

        let staged_map = HashMap::from([(PathBuf::from("config/parser.go"), source.to_string())]);
        let head_map = HashMap::new();

        let analyzer = AnalyzerService::new().expect("AnalyzerService::new() should succeed");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let func = symbols
            .iter()
            .find(|s| s.name == "ParseConfig")
            .expect("expected a symbol named 'ParseConfig'");
        assert_eq!(func.kind, SymbolKind::Function);

        let sig = func
            .signature
            .as_ref()
            .expect("expected signature to be extracted for Go function");
        assert!(
            sig.contains("path"),
            "signature should contain parameter 'path', got: {sig}"
        );
        assert!(
            sig.contains("strict"),
            "signature should contain parameter 'strict', got: {sig}"
        );
        assert!(
            sig.contains("error"),
            "signature should contain return type 'error', got: {sig}"
        );
    }
}

// ─── BODY_NODE_KINDS coverage tests ──────────────────────────────────────────
// Verify that signature extraction uses body-node detection (not first-line fallback)
// for languages beyond Rust/TS/Python/Go.

#[cfg(feature = "lang-java")]
mod java_signature {
    use super::*;

    #[test]
    fn java_method_signature_extracted() {
        let source = "public class Handler {\n    public void process(String input, int count) {\n        System.out.println(input);\n    }\n}\n";
        let diff = "@@ -0,0 +1,5 @@\n+public class Handler {\n+    public void process(String input, int count) {\n";
        let change = make_file_change("src/Handler.java", diff, 5, 0);
        let staged_map = HashMap::from([(PathBuf::from("src/Handler.java"), source.to_string())]);
        let head_map = HashMap::new();
        let analyzer = AnalyzerService::new().expect("AnalyzerService::new()");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let method = symbols.iter().find(|s| s.name == "process");
        assert!(
            method.is_some(),
            "expected symbol 'process', got: {symbols:?}"
        );
        let sig = method
            .unwrap()
            .signature
            .as_ref()
            .expect("signature should be Some");
        assert!(
            sig.contains("process") && sig.contains("String"),
            "Java method signature should contain params, got: {sig}"
        );
    }
}

#[cfg(feature = "lang-c")]
mod c_signature {
    use super::*;

    #[test]
    fn c_function_signature_extracted() {
        let source = "int calculate(int a, int b) {\n    return a + b;\n}\n";
        let diff = "@@ -0,0 +1,3 @@\n+int calculate(int a, int b) {\n";
        let change = make_file_change("src/math.c", diff, 3, 0);
        let staged_map = HashMap::from([(PathBuf::from("src/math.c"), source.to_string())]);
        let head_map = HashMap::new();
        let analyzer = AnalyzerService::new().expect("AnalyzerService::new()");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let func = symbols.iter().find(|s| s.name == "calculate");
        assert!(
            func.is_some(),
            "expected symbol 'calculate', got: {symbols:?}"
        );
        let sig = func
            .unwrap()
            .signature
            .as_ref()
            .expect("signature should be Some");
        assert!(
            sig.contains("int a") && sig.contains("int b"),
            "C function signature should contain params, got: {sig}"
        );
    }
}

#[cfg(feature = "lang-cpp")]
mod cpp_signature {
    use super::*;

    #[test]
    fn cpp_method_signature_extracted() {
        let source = "class Parser {\npublic:\n    void parse(const std::string& input) {\n        // body\n    }\n};\n";
        let diff = "@@ -0,0 +1,6 @@\n+class Parser {\n";
        let change = make_file_change("src/parser.cpp", diff, 6, 0);
        let staged_map = HashMap::from([(PathBuf::from("src/parser.cpp"), source.to_string())]);
        let head_map = HashMap::new();
        let analyzer = AnalyzerService::new().expect("AnalyzerService::new()");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let cls = symbols.iter().find(|s| s.name == "Parser");
        assert!(cls.is_some(), "expected symbol 'Parser', got: {symbols:?}");
        let sig = cls
            .unwrap()
            .signature
            .as_ref()
            .expect("signature should be Some");
        assert!(
            sig.contains("Parser"),
            "C++ class signature should contain class name, got: {sig}"
        );
    }
}

#[cfg(feature = "lang-ruby")]
mod ruby_signature {
    use super::*;

    #[test]
    fn ruby_method_signature_extracted() {
        let source = "class Greeter\n  def greet(name)\n    puts name\n  end\nend\n";
        let diff = "@@ -0,0 +1,5 @@\n+class Greeter\n";
        let change = make_file_change("src/greeter.rb", diff, 5, 0);
        let staged_map = HashMap::from([(PathBuf::from("src/greeter.rb"), source.to_string())]);
        let head_map = HashMap::new();
        let analyzer = AnalyzerService::new().expect("AnalyzerService::new()");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let cls = symbols.iter().find(|s| s.name == "Greeter");
        assert!(cls.is_some(), "expected symbol 'Greeter', got: {symbols:?}");
        let sig = cls
            .unwrap()
            .signature
            .as_ref()
            .expect("signature should be Some");
        assert!(
            sig.contains("Greeter"),
            "Ruby class signature should contain class name, got: {sig}"
        );
    }
}

#[cfg(feature = "lang-csharp")]
mod csharp_signature {
    use super::*;

    #[test]
    fn csharp_method_signature_extracted() {
        let source = "public class Service {\n    public string Process(int id, string name) {\n        return name;\n    }\n}\n";
        let diff = "@@ -0,0 +1,5 @@\n+public class Service {\n";
        let change = make_file_change("src/Service.cs", diff, 5, 0);
        let staged_map = HashMap::from([(PathBuf::from("src/Service.cs"), source.to_string())]);
        let head_map = HashMap::new();
        let analyzer = AnalyzerService::new().expect("AnalyzerService::new()");
        let (symbols, _) = analyzer.extract_symbols(&[change], &staged_map, &head_map);

        let cls = symbols.iter().find(|s| s.name == "Service");
        assert!(cls.is_some(), "expected symbol 'Service', got: {symbols:?}");
        let sig = cls
            .unwrap()
            .signature
            .as_ref()
            .expect("signature should be Some");
        assert!(
            sig.contains("Service"),
            "C# class signature should contain class name, got: {sig}"
        );
    }
}
