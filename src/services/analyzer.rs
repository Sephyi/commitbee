// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::LazyLock;

use regex::Regex;
use std::path::Path;
use tree_sitter::{Language, Parser};

use crate::domain::{CodeSymbol, FileChange, SymbolKind};
use crate::error::Result;

/// Represents a diff hunk with line ranges
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
}

// Robust regex for parsing unified diff hunk headers
static HUNK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@@\s*-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s*@@").unwrap());

impl DiffHunk {
    /// Parse hunks from unified diff format
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

pub struct AnalyzerService;

impl AnalyzerService {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Extract symbols from file changes using full file content + hunk mapping
    pub fn extract_symbols(
        &mut self,
        changes: &[FileChange],
        staged_content: &dyn Fn(&Path) -> Option<String>,
        head_content: &dyn Fn(&Path) -> Option<String>,
    ) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();

        for change in changes {
            if change.is_binary {
                continue;
            }

            let ext = change
                .path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            let hunks = DiffHunk::parse_from_diff(&change.diff);

            // Get the appropriate language for parsing
            let language: Option<Language> = match ext {
                "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
                "ts" | "tsx" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
                "py" => Some(tree_sitter_python::LANGUAGE.into()),
                "go" => Some(tree_sitter_go::LANGUAGE.into()),
                "js" | "jsx" => Some(tree_sitter_javascript::LANGUAGE.into()),
                _ => None,
            };

            if let Some(lang) = language {
                let file_symbols = Self::extract_for_file_static(
                    lang,
                    change,
                    &hunks,
                    staged_content,
                    head_content,
                );
                symbols.extend(file_symbols);
            }
        }

        symbols
    }

    fn extract_for_file_static(
        language: Language,
        change: &FileChange,
        hunks: &[DiffHunk],
        staged_content: &dyn Fn(&Path) -> Option<String>,
        head_content: &dyn Fn(&Path) -> Option<String>,
    ) -> Vec<CodeSymbol> {
        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() {
            return Vec::new();
        }

        let mut symbols = Vec::new();

        // Parse staged (new) file content
        if let Some(content) = staged_content(&change.path) {
            let changed = Self::extract_changed_symbols_static(
                &mut parser,
                &change.path,
                &content,
                hunks,
                true,
            );
            symbols.extend(changed);
        }

        // Parse HEAD (old) file content
        if let Some(content) = head_content(&change.path) {
            let changed = Self::extract_changed_symbols_static(
                &mut parser,
                &change.path,
                &content,
                hunks,
                false,
            );
            symbols.extend(changed);
        }

        symbols
    }

    fn extract_changed_symbols_static(
        parser: &mut Parser,
        file: &Path,
        source: &str,
        hunks: &[DiffHunk],
        is_added: bool,
    ) -> Vec<CodeSymbol> {
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };

        let mut symbols = Vec::new();
        let mut cursor = tree.walk();

        Self::visit_node_with_hunks(&mut cursor, file, source, hunks, is_added, &mut symbols);

        symbols
    }

    fn visit_node_with_hunks(
        cursor: &mut tree_sitter::TreeCursor,
        file: &Path,
        source: &str,
        hunks: &[DiffHunk],
        is_added: bool,
        symbols: &mut Vec<CodeSymbol>,
    ) {
        loop {
            let node = cursor.node();
            let kind_str = node.kind();

            let symbol_kind = match kind_str {
                "function_item" | "function_definition" | "function_declaration" => {
                    Some(SymbolKind::Function)
                }
                "method_definition" | "method_declaration" => Some(SymbolKind::Method),
                "struct_item" | "struct_declaration" => Some(SymbolKind::Struct),
                "enum_item" | "enum_declaration" => Some(SymbolKind::Enum),
                "trait_item" => Some(SymbolKind::Trait),
                "impl_item" => Some(SymbolKind::Impl),
                "class_declaration" | "class_definition" => Some(SymbolKind::Class),
                "interface_declaration" => Some(SymbolKind::Interface),
                "const_item" | "const_declaration" => Some(SymbolKind::Const),
                "type_alias_declaration" | "type_item" => Some(SymbolKind::Type),
                _ => None,
            };

            if let Some(kind) = symbol_kind {
                let line_start = node.start_position().row + 1;
                let line_end = node.end_position().row + 1;

                // Check if this symbol's span intersects any changed hunk
                let intersects = hunks.iter().any(|h| {
                    if is_added {
                        h.intersects_new(line_start, line_end)
                    } else {
                        h.intersects_old(line_start, line_end)
                    }
                });

                if intersects {
                    let name = node
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("anonymous")
                        .to_string();

                    let is_public = node
                        .child(0)
                        .map(|n| n.kind() == "visibility_modifier")
                        .unwrap_or(false);

                    symbols.push(CodeSymbol {
                        kind,
                        name,
                        file: file.to_path_buf(),
                        line: line_start,
                        is_public,
                        is_added,
                    });
                }
            }

            // Recurse into children
            if cursor.goto_first_child() {
                Self::visit_node_with_hunks(cursor, file, source, hunks, is_added, symbols);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
