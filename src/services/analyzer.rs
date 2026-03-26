// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use rayon::prelude::*;
use regex::Regex;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use crate::domain::{CodeSymbol, FileChange, SymbolKind};
use crate::error::Result;

// ─── Embedded query patterns ────────────────────────────────────────────────

#[cfg(feature = "lang-rust")]
const RUST_QUERY: &str = include_str!("../queries/rust.scm");
#[cfg(feature = "lang-typescript")]
const TYPESCRIPT_QUERY: &str = include_str!("../queries/typescript.scm");
#[cfg(feature = "lang-javascript")]
const JAVASCRIPT_QUERY: &str = include_str!("../queries/javascript.scm");
#[cfg(feature = "lang-python")]
const PYTHON_QUERY: &str = include_str!("../queries/python.scm");
#[cfg(feature = "lang-go")]
const GO_QUERY: &str = include_str!("../queries/go.scm");

#[cfg(feature = "lang-java")]
const JAVA_QUERY: &str = include_str!("../queries/java.scm");
#[cfg(feature = "lang-c")]
const C_QUERY: &str = include_str!("../queries/c.scm");
#[cfg(feature = "lang-cpp")]
const CPP_QUERY: &str = include_str!("../queries/cpp.scm");
#[cfg(feature = "lang-ruby")]
const RUBY_QUERY: &str = include_str!("../queries/ruby.scm");
#[cfg(feature = "lang-csharp")]
const CSHARP_QUERY: &str = include_str!("../queries/csharp.scm");

/// Represents a diff hunk with line ranges
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
}

// Robust regex for parsing unified diff hunk headers
static HUNK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^@@\s*-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s*@@")
        .expect("static hunk header regex is valid")
});

impl DiffHunk {
    /// Parse hunks from unified diff format
    #[must_use]
    pub fn parse_from_diff(diff: &str) -> Vec<Self> {
        let mut hunks = Vec::new();

        for line in diff.lines() {
            if let Some(hunk) = Self::parse_hunk_header(line) {
                hunks.push(hunk);
            }
        }

        hunks
    }

    fn parse_hunk_header(line: &str) -> Option<Self> {
        let caps = HUNK_REGEX.captures(line)?;

        let old_start: usize = caps.get(1)?.as_str().parse().ok()?;
        let old_count: usize = caps
            .get(2)
            .map(|m| m.as_str().parse().unwrap_or(1))
            .unwrap_or(1);

        let new_start: usize = caps.get(3)?.as_str().parse().ok()?;
        let new_count: usize = caps
            .get(4)
            .map(|m| m.as_str().parse().unwrap_or(1))
            .unwrap_or(1);

        Some(Self {
            old_start,
            old_count,
            new_start,
            new_count,
        })
    }

    /// Check if a line range intersects this hunk (for new file)
    pub fn intersects_new(&self, line_start: usize, line_end: usize) -> bool {
        let hunk_end = self.new_start + self.new_count;
        line_start < hunk_end && line_end > self.new_start
    }

    /// Check if a line range intersects this hunk (for old file)
    pub fn intersects_old(&self, line_start: usize, line_end: usize) -> bool {
        let hunk_end = self.old_start + self.old_count;
        line_start < hunk_end && line_end > self.old_start
    }
}

/// Language-specific configuration for query-based symbol extraction
struct LanguageConfig {
    language: Language,
    query_source: &'static str,
    file_ext: &'static str,
}

pub struct AnalyzerService;

impl AnalyzerService {
    /// Body-like node kinds across all supported languages.
    const BODY_NODE_KINDS: &[&str] = &[
        "block",                          // Rust (fn), Python, Go, Java, C#
        "statement_block",                // TypeScript, JavaScript
        "compound_statement",             // C, C++
        "class_body",                     // TypeScript, JavaScript, Java
        "interface_body",                 // Java, C#
        "enum_body",                      // Java
        "field_declaration_list",         // Rust (struct), C, C++
        "ordered_field_declaration_list", // Rust (tuple structs)
        "enum_variant_list",              // Rust (enum variants)
        "declaration_list",               // Rust (impl, trait)
        "body_statement",                 // Ruby (method/class body)
        "enum_member_declaration_list",   // C# (enum body)
    ];

    const MAX_SIGNATURE_LEN: usize = 200;

    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Extract symbols from file changes using full file content + hunk mapping.
    /// Uses rayon to parse files in parallel across CPU cores.
    pub fn extract_symbols(
        &self,
        changes: &[FileChange],
        staged_content: &HashMap<PathBuf, String>,
        head_content: &HashMap<PathBuf, String>,
    ) -> Vec<CodeSymbol> {
        changes
            .par_iter()
            .filter(|change| !change.is_binary)
            .flat_map(|change| {
                let ext = change
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");

                let config = match ext {
                    #[cfg(feature = "lang-rust")]
                    "rs" => Some(LanguageConfig {
                        language: tree_sitter_rust::LANGUAGE.into(),
                        query_source: RUST_QUERY,
                        file_ext: "rs",
                    }),
                    #[cfg(feature = "lang-typescript")]
                    "ts" | "tsx" => Some(LanguageConfig {
                        language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                        query_source: TYPESCRIPT_QUERY,
                        file_ext: "ts",
                    }),
                    #[cfg(feature = "lang-python")]
                    "py" => Some(LanguageConfig {
                        language: tree_sitter_python::LANGUAGE.into(),
                        query_source: PYTHON_QUERY,
                        file_ext: "py",
                    }),
                    #[cfg(feature = "lang-go")]
                    "go" => Some(LanguageConfig {
                        language: tree_sitter_go::LANGUAGE.into(),
                        query_source: GO_QUERY,
                        file_ext: "go",
                    }),
                    #[cfg(feature = "lang-javascript")]
                    "js" | "jsx" => Some(LanguageConfig {
                        language: tree_sitter_javascript::LANGUAGE.into(),
                        query_source: JAVASCRIPT_QUERY,
                        file_ext: "js",
                    }),
                    #[cfg(feature = "lang-java")]
                    "java" => Some(LanguageConfig {
                        language: tree_sitter_java::LANGUAGE.into(),
                        query_source: JAVA_QUERY,
                        file_ext: "java",
                    }),
                    #[cfg(feature = "lang-c")]
                    "c" | "h" => Some(LanguageConfig {
                        language: tree_sitter_c::LANGUAGE.into(),
                        query_source: C_QUERY,
                        file_ext: "c",
                    }),
                    #[cfg(feature = "lang-cpp")]
                    "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(LanguageConfig {
                        language: tree_sitter_cpp::LANGUAGE.into(),
                        query_source: CPP_QUERY,
                        file_ext: "cpp",
                    }),
                    #[cfg(feature = "lang-ruby")]
                    "rb" => Some(LanguageConfig {
                        language: tree_sitter_ruby::LANGUAGE.into(),
                        query_source: RUBY_QUERY,
                        file_ext: "rb",
                    }),
                    #[cfg(feature = "lang-csharp")]
                    "cs" => Some(LanguageConfig {
                        language: tree_sitter_c_sharp::LANGUAGE.into(),
                        query_source: CSHARP_QUERY,
                        file_ext: "cs",
                    }),
                    _ => None,
                };

                config
                    .map(|cfg| {
                        let hunks = DiffHunk::parse_from_diff(&change.diff);
                        Self::extract_for_file(cfg, change, &hunks, staged_content, head_content)
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    fn extract_for_file(
        config: LanguageConfig,
        change: &FileChange,
        hunks: &[DiffHunk],
        staged_content: &HashMap<PathBuf, String>,
        head_content: &HashMap<PathBuf, String>,
    ) -> Vec<CodeSymbol> {
        let mut parser = Parser::new();
        if parser.set_language(&config.language).is_err() {
            return Vec::new();
        }

        let Ok(query) = Query::new(&config.language, config.query_source) else {
            return Vec::new();
        };

        let mut symbols = Vec::new();

        // Parse staged (new) file content
        if let Some(content) = staged_content.get(&change.path) {
            let changed = Self::extract_changed_symbols_with_query(
                &mut parser,
                &query,
                config.file_ext,
                &change.path,
                content,
                hunks,
                true,
            );
            symbols.extend(changed);
        }

        // Parse HEAD (old) file content
        if let Some(content) = head_content.get(&change.path) {
            let changed = Self::extract_changed_symbols_with_query(
                &mut parser,
                &query,
                config.file_ext,
                &change.path,
                content,
                hunks,
                false,
            );
            symbols.extend(changed);
        }

        symbols
    }

    fn extract_changed_symbols_with_query(
        parser: &mut Parser,
        query: &Query,
        file_ext: &str,
        file: &Path,
        source: &str,
        hunks: &[DiffHunk],
        is_added: bool,
    ) -> Vec<CodeSymbol> {
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };

        let Some(name_idx) = query.capture_index_for_name("name") else {
            return Vec::new();
        };
        let Some(def_idx) = query.capture_index_for_name("definition") else {
            return Vec::new();
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());

        let mut symbols = Vec::new();
        while let Some(m) = matches.next() {
            let def_capture = m.captures.iter().find(|c| c.index == def_idx);
            let name_capture = m.captures.iter().find(|c| c.index == name_idx);

            if let (Some(def), Some(name)) = (def_capture, name_capture) {
                let def_node = def.node;
                let line_start = def_node.start_position().row + 1;
                let line_end = def_node.end_position().row + 1;

                // Check if this symbol's span intersects any changed hunk
                let intersects = hunks.iter().any(|h| {
                    if is_added {
                        h.intersects_new(line_start, line_end)
                    } else {
                        h.intersects_old(line_start, line_end)
                    }
                });

                if !intersects {
                    continue;
                }

                let symbol_name = name
                    .node
                    .utf8_text(source.as_bytes())
                    .unwrap_or("anonymous")
                    .to_string();

                let kind = Self::node_kind_to_symbol_kind(def_node.kind());

                let is_public = Self::detect_visibility(def_node, file_ext, &symbol_name, source);

                let signature = Self::extract_signature(def_node, source);

                let parent_scope = Self::extract_parent_scope(def_node, source);

                if let Some(kind) = kind {
                    symbols.push(CodeSymbol {
                        kind,
                        name: symbol_name,
                        file: file.to_path_buf(),
                        line: line_start,
                        end_line: line_end,
                        is_public,
                        is_added,
                        is_whitespace_only: None,
                        span_change_kind: None,
                        signature,
                        parent_scope,
                    });
                }
            }
        }
        symbols
    }

    /// Map tree-sitter node kinds to `SymbolKind` values
    fn node_kind_to_symbol_kind(node_kind: &str) -> Option<SymbolKind> {
        match node_kind {
            // Functions (Rust, C, C++, Go, Python, JS/TS)
            "function_item" | "function_definition" | "function_declaration" => {
                Some(SymbolKind::Function)
            }
            // Methods (JS/TS, Java, C#)
            "method_definition" | "method_declaration" => Some(SymbolKind::Method),
            // Ruby methods
            "method" | "singleton_method" => Some(SymbolKind::Method),
            // Constructors (Java, C#)
            "constructor_declaration" => Some(SymbolKind::Method),
            // Structs (Rust, C#)
            "struct_item" | "struct_declaration" => Some(SymbolKind::Struct),
            // C/C++ struct specifier
            "struct_specifier" => Some(SymbolKind::Struct),
            // Enums (Rust, Java, C#)
            "enum_item" | "enum_declaration" => Some(SymbolKind::Enum),
            // C/C++ enum specifier
            "enum_specifier" => Some(SymbolKind::Enum),
            // Rust traits
            "trait_item" => Some(SymbolKind::Trait),
            // Rust impl blocks
            "impl_item" => Some(SymbolKind::Impl),
            // Classes (JS/TS, Java, C#, Python, Ruby)
            "class_declaration" | "class_definition" => Some(SymbolKind::Class),
            // C++ class specifier
            "class_specifier" => Some(SymbolKind::Class),
            // Ruby class and module
            "class" | "module" => Some(SymbolKind::Class),
            // Interfaces (TS, Java, C#)
            "interface_declaration" => Some(SymbolKind::Interface),
            // Constants (Rust, JS/TS)
            "const_item" | "const_declaration" => Some(SymbolKind::Const),
            // Type aliases (TS, Rust)
            "type_alias_declaration" | "type_item" | "type_declaration" => Some(SymbolKind::Type),
            // C typedef
            "type_definition" => Some(SymbolKind::Type),
            _ => None,
        }
    }

    /// Detect visibility based on language-specific conventions
    fn detect_visibility(
        def_node: tree_sitter::Node,
        file_ext: &str,
        symbol_name: &str,
        _source: &str,
    ) -> bool {
        match file_ext {
            // Rust: first child is a visibility_modifier (e.g., `pub`)
            "rs" => def_node
                .child(0)
                .map(|n| n.kind() == "visibility_modifier")
                .unwrap_or(false),
            // Python: public if name does not start with underscore
            "py" => !symbol_name.starts_with('_'),
            // Go: public if first character is uppercase
            "go" => symbol_name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false),
            // Java: check for `modifiers` child containing `public`
            "java" => Self::has_java_public_modifier(def_node),
            // C#: check for `modifier` child with `public` text
            "cs" => Self::has_csharp_public_modifier(def_node),
            // C/C++: no file-scope visibility modifiers, default to public
            "c" | "cpp" => true,
            // Ruby: no straightforward visibility detection from AST, default to public
            "rb" => true,
            // TypeScript/JavaScript: no standard visibility modifier at AST level
            _ => false,
        }
    }

    /// Check if a Java node has a `modifiers` child containing a `public` modifier
    fn has_java_public_modifier(node: tree_sitter::Node) -> bool {
        let child_count = node.child_count();
        for i in 0..child_count {
            #[allow(clippy::cast_possible_truncation)]
            if let Some(child) = node.child(i as u32)
                && child.kind() == "modifiers"
            {
                let mut cursor = child.walk();
                if cursor.goto_first_child() {
                    loop {
                        if cursor.node().kind() == "public" {
                            return true;
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
                return false;
            }
        }
        false
    }

    /// Extract the signature from a definition node by taking text before the body.
    /// Two-strategy: child_by_field_name("body") primary, BODY_NODE_KINDS fallback.
    pub(crate) fn extract_signature(node: tree_sitter::Node, source: &str) -> Option<String> {
        let node_start = node.start_byte();

        // Strategy 1: named "body" field (most grammars)
        // Strategy 2: scan children for known body node kinds
        let body_start = node
            .child_by_field_name("body")
            .map(|child| child.start_byte())
            .or_else(|| {
                (0..node.child_count())
                    .filter_map(|i| {
                        #[allow(clippy::cast_possible_truncation)]
                        node.child(i as u32)
                    })
                    .find(|child| Self::BODY_NODE_KINDS.contains(&child.kind()))
                    .map(|child| child.start_byte())
            });

        let sig_text = if let Some(body_byte) = body_start {
            &source[node_start..body_byte]
        } else {
            // No body found — take first line as fallback
            let text = node.utf8_text(source.as_bytes()).ok()?;
            return Some(Self::normalize_signature(
                text.lines().next().unwrap_or("").trim(),
            ));
        };

        Some(Self::normalize_signature(sig_text))
    }

    /// Collapse multi-line signature to single line and cap length.
    fn normalize_signature(raw: &str) -> String {
        let normalized: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.len() > Self::MAX_SIGNATURE_LEN {
            let end = normalized.floor_char_boundary(Self::MAX_SIGNATURE_LEN);
            format!("{}...", &normalized[..end])
        } else {
            normalized
        }
    }

    /// Walk up the AST to find the enclosing scope (impl, class, trait).
    /// Skips intermediate wrapper nodes like `declaration_list`, `class_body`, etc.
    fn extract_parent_scope(node: tree_sitter::Node, source: &str) -> Option<String> {
        let mut current = node.parent();
        while let Some(parent) = current {
            match parent.kind() {
                "impl_item" | "impl_block" => {
                    // Rust: impl Type { fn method() }
                    return parent
                        .child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());
                }
                "class_declaration" | "class_definition" | "class" | "class_specifier" => {
                    // Most languages: class Foo { method() }
                    return parent
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());
                }
                "trait_item" => {
                    return parent
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());
                }
                _ => {
                    current = parent.parent();
                }
            }
        }
        None
    }

    /// Check if a C# node has a `modifier` child with text "public"
    fn has_csharp_public_modifier(node: tree_sitter::Node) -> bool {
        let child_count = node.child_count();
        for i in 0..child_count {
            #[allow(clippy::cast_possible_truncation)]
            if let Some(child) = node.child(i as u32)
                && child.kind() == "modifier"
            {
                let mut cursor = child.walk();
                if cursor.goto_first_child() {
                    loop {
                        if cursor.node().kind() == "public" {
                            return true;
                        }
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_rust_function() {
        let source =
            "pub fn connect(host: &str, timeout: u64) -> Result<Connection> {\n    // body\n}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let func_node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(func_node, source);
        assert_eq!(
            sig.as_deref(),
            Some("pub fn connect(host: &str, timeout: u64) -> Result<Connection>")
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_rust_struct() {
        let source = "pub struct Config {\n    pub timeout: u64,\n}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(node, source);
        assert_eq!(sig.as_deref(), Some("pub struct Config"));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_rust_enum() {
        let source = "pub enum Status {\n    Active,\n    Inactive,\n}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(node, source);
        assert_eq!(sig.as_deref(), Some("pub enum Status"));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_multiline_params() {
        let source = "fn process(\n    items: Vec<Item>,\n    filter: &str,\n) -> Vec<Item> {\n    items\n}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(node, source);
        assert_eq!(
            sig.as_deref(),
            Some("fn process( items: Vec<Item>, filter: &str, ) -> Vec<Item>")
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_caps_length() {
        let params = (0..20)
            .map(|i| format!("p{i}: SomeLongTypeName"))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!("fn huge({params}) {{\n}}\n");
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(node, &source);
        assert!(sig.as_ref().is_some_and(|s| s.len() <= 203)); // 200 + "..."
        assert!(sig.as_ref().is_some_and(|s| s.ends_with("...")));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_impl_block() {
        let source = "impl Display for Config {\n    fn fmt(&self, f: &mut Formatter) -> Result {\n        Ok(())\n    }\n}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(node, source);
        assert_eq!(sig.as_deref(), Some("impl Display for Config"));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_trait_def() {
        let source = "pub trait Handler {\n    fn handle(&self);\n}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(node, source);
        assert_eq!(sig.as_deref(), Some("pub trait Handler"));
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_single_line_function() {
        let source = "fn foo() {}\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(node, source);
        assert!(
            sig.is_some(),
            "single-line function should have a signature"
        );
        assert!(
            sig.as_ref().unwrap().contains("fn foo()"),
            "signature should contain fn foo(), got: {:?}",
            sig
        );
    }

    #[cfg(feature = "lang-rust")]
    #[test]
    fn extract_signature_const_item_no_body() {
        let source = "pub const MAX: usize = 100;\n";
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let sig = AnalyzerService::extract_signature(node, source);
        assert!(
            sig.is_some(),
            "const item should have a signature (first-line fallback)"
        );
        assert!(
            sig.as_ref().unwrap().contains("MAX"),
            "signature should contain const name, got: {:?}",
            sig
        );
    }
}
