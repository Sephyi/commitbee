// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! Tests for feature-gated language support (Java, C, C++, Ruby, C#).
//!
//! Each test is gated behind its corresponding Cargo feature flag.
//! Run with: `cargo test --test languages --features all-languages`

#[cfg(any(
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use std::collections::HashMap;
#[cfg(any(
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use std::path::PathBuf;
#[cfg(any(
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use std::sync::Arc;

#[cfg(any(
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use commitbee::domain::{ChangeStatus, FileCategory, FileChange, SymbolKind};
#[cfg(any(
    feature = "lang-java",
    feature = "lang-c",
    feature = "lang-cpp",
    feature = "lang-ruby",
    feature = "lang-csharp"
))]
use commitbee::services::analyzer::AnalyzerService;

#[cfg(any(
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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
            let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
        let symbols = analyzer.extract_symbols(&[change], &staged_map, &head_map);

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
