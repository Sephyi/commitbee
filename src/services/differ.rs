// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::HashSet;

use crate::domain::diff::ChangeDetail;

#[allow(dead_code)]
pub struct AstDiffer;

#[allow(dead_code)]
impl AstDiffer {
    /// Compare old and new versions of a function/method definition.
    /// Must be called while both Tree objects are alive (F-002 lifetime constraint).
    pub fn diff_function(
        old_node: tree_sitter::Node,
        old_source: &str,
        new_node: tree_sitter::Node,
        new_source: &str,
    ) -> Vec<ChangeDetail> {
        let mut changes = Vec::new();

        // Compare parameters
        let old_params = Self::extract_params(old_node, old_source);
        let new_params = Self::extract_params(new_node, new_source);
        Self::diff_params(&old_params, &new_params, &mut changes);

        // Compare return type
        let old_ret = Self::extract_return_type(old_node, old_source);
        let new_ret = Self::extract_return_type(new_node, new_source);
        if old_ret != new_ret
            && let (Some(o), Some(n)) = (old_ret, new_ret)
        {
            changes.push(ChangeDetail::ReturnTypeChanged { old: o, new: n });
        }

        // Compare visibility
        let old_vis = Self::extract_visibility(old_node, old_source);
        let new_vis = Self::extract_visibility(new_node, new_source);
        if old_vis != new_vis {
            changes.push(ChangeDetail::VisibilityChanged {
                old: old_vis,
                new: new_vis,
            });
        }

        // Compare async
        let old_async = Self::is_async(old_node);
        let new_async = Self::is_async(new_node);
        if old_async != new_async {
            changes.push(ChangeDetail::AsyncChanged(new_async));
        }

        // Compare body using whitespace-stripped comparison (F-015)
        let old_body = Self::extract_body_text(old_node, old_source);
        let new_body = Self::extract_body_text(new_node, new_source);
        if Self::bodies_semantically_equal(old_body.as_deref(), new_body.as_deref()) {
            changes.push(ChangeDetail::BodyUnchanged);
        } else {
            let old_lines: Vec<&str> = old_body.as_deref().unwrap_or("").lines().collect();
            let new_lines: Vec<&str> = new_body.as_deref().unwrap_or("").lines().collect();
            let old_set: HashSet<&str> = old_lines.iter().copied().collect();
            let new_set: HashSet<&str> = new_lines.iter().copied().collect();
            let additions = new_set.difference(&old_set).count();
            let deletions = old_set.difference(&new_set).count();
            changes.push(ChangeDetail::BodyModified {
                additions,
                deletions,
            });
        }

        changes
    }

    /// Phase 2 stub (F-004) — struct/enum diffing not implemented yet.
    pub fn diff_struct(
        _old_node: tree_sitter::Node,
        _old_source: &str,
        _new_node: tree_sitter::Node,
        _new_source: &str,
    ) -> Vec<ChangeDetail> {
        Vec::new()
    }

    fn bodies_semantically_equal(old_body: Option<&str>, new_body: Option<&str>) -> bool {
        let strip = |s: &str| -> String { s.chars().filter(|c| !c.is_whitespace()).collect() };
        match (old_body, new_body) {
            (Some(o), Some(n)) => strip(o) == strip(n),
            (None, None) => true,
            _ => false,
        }
    }

    /// Extract parameter list as vec of (name, type) string pairs.
    fn extract_params(node: tree_sitter::Node, source: &str) -> Vec<(String, String)> {
        let mut params = Vec::new();
        // Look for parameters/parameter_list/formal_parameters child
        let param_node = node.child_by_field_name("parameters").or_else(|| {
            (0..node.child_count())
                .filter_map(|i| {
                    #[allow(clippy::cast_possible_truncation)]
                    node.child(i as u32)
                })
                .find(|c| {
                    matches!(
                        c.kind(),
                        "parameters" | "parameter_list" | "formal_parameters" | "type_parameters"
                    )
                })
        });

        if let Some(pnode) = param_node {
            for i in 0..pnode.child_count() {
                #[allow(clippy::cast_possible_truncation)]
                if let Some(child) = pnode.child(i as u32) {
                    // Skip delimiters like ( ) ,
                    if matches!(child.kind(), "(" | ")" | "," | "&" | "comment") {
                        continue;
                    }
                    // Try to get name and type from the parameter
                    let name = child
                        .child_by_field_name("pattern")
                        .or_else(|| child.child_by_field_name("name"))
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("")
                        .to_string();
                    let typ = child
                        .child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("")
                        .to_string();
                    if !name.is_empty() || !typ.is_empty() {
                        params.push((name, typ));
                    }
                }
            }
        }
        params
    }

    /// Compare parameter lists, emitting Added/Removed/TypeChanged.
    fn diff_params(
        old: &[(String, String)],
        new: &[(String, String)],
        changes: &mut Vec<ChangeDetail>,
    ) {
        let old_names: HashSet<&str> = old.iter().map(|(n, _)| n.as_str()).collect();
        let new_names: HashSet<&str> = new.iter().map(|(n, _)| n.as_str()).collect();

        // Added params
        for (name, typ) in new {
            if !old_names.contains(name.as_str()) && !name.is_empty() {
                let desc = if typ.is_empty() {
                    name.clone()
                } else {
                    format!("{name}: {typ}")
                };
                changes.push(ChangeDetail::ParamAdded(desc));
            }
        }

        // Removed params
        for (name, typ) in old {
            if !new_names.contains(name.as_str()) && !name.is_empty() {
                let desc = if typ.is_empty() {
                    name.clone()
                } else {
                    format!("{name}: {typ}")
                };
                changes.push(ChangeDetail::ParamRemoved(desc));
            }
        }

        // Type changes for params that exist in both
        for (name, old_typ) in old {
            if let Some((_, new_typ)) = new.iter().find(|(n, _)| n == name)
                && old_typ != new_typ
                && !old_typ.is_empty()
                && !new_typ.is_empty()
            {
                changes.push(ChangeDetail::ParamTypeChanged {
                    name: name.clone(),
                    old_type: old_typ.clone(),
                    new_type: new_typ.clone(),
                });
            }
        }
    }

    fn extract_return_type(node: tree_sitter::Node, source: &str) -> Option<String> {
        node.child_by_field_name("return_type")
            .and_then(|rt| rt.utf8_text(source.as_bytes()).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn extract_visibility(node: tree_sitter::Node, source: &str) -> Option<String> {
        // Check for visibility_modifier child (Rust)
        (0..node.child_count())
            .filter_map(|i| {
                #[allow(clippy::cast_possible_truncation)]
                node.child(i as u32)
            })
            .find(|c| {
                matches!(
                    c.kind(),
                    "visibility_modifier" | "modifiers" | "access_specifier"
                )
            })
            .and_then(|v| v.utf8_text(source.as_bytes()).ok())
            .map(|s| s.trim().to_string())
    }

    fn is_async(node: tree_sitter::Node) -> bool {
        (0..node.child_count())
            .filter_map(|i| {
                #[allow(clippy::cast_possible_truncation)]
                node.child(i as u32)
            })
            .any(|c| c.kind() == "async")
    }

    fn extract_body_text(node: tree_sitter::Node, source: &str) -> Option<String> {
        // Use same BODY_NODE_KINDS logic as AnalyzerService
        let body = node.child_by_field_name("body").or_else(|| {
            (0..node.child_count())
                .filter_map(|i| {
                    #[allow(clippy::cast_possible_truncation)]
                    node.child(i as u32)
                })
                .find(|c| {
                    matches!(
                        c.kind(),
                        "block"
                            | "statement_block"
                            | "compound_statement"
                            | "class_body"
                            | "interface_body"
                            | "enum_body"
                            | "field_declaration_list"
                            | "ordered_field_declaration_list"
                            | "enum_variant_list"
                            | "declaration_list"
                            | "body_statement"
                            | "enum_member_declaration_list"
                    )
                })
        });
        body.and_then(|b| b.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_function(tree: &tree_sitter::Tree) -> tree_sitter::Node<'_> {
        let root = tree.root_node();
        (0..root.child_count())
            .filter_map(|i| {
                #[allow(clippy::cast_possible_truncation)]
                root.child(i as u32)
            })
            .find(|n| matches!(n.kind(), "function_item" | "function_definition"))
            .expect("no function found")
    }

    #[test]
    fn detect_added_parameter() {
        let old_src = "fn process(items: Vec<Item>) -> bool { true }";
        let new_src = "fn process(items: Vec<Item>, strict: bool) -> bool { true }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let old_tree = parser.parse(old_src, None).unwrap();
        let new_tree = parser.parse(new_src, None).unwrap();
        let old_fn = first_function(&old_tree);
        let new_fn = first_function(&new_tree);
        let changes = AstDiffer::diff_function(old_fn, old_src, new_fn, new_src);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ChangeDetail::ParamAdded(p) if p.contains("strict"))),
            "should detect added param 'strict': {changes:?}"
        );
    }

    #[test]
    fn detect_return_type_change() {
        let old_src = "fn validate(input: &str) -> bool { true }";
        let new_src = "fn validate(input: &str) -> Result<()> { Ok(()) }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let old_tree = parser.parse(old_src, None).unwrap();
        let new_tree = parser.parse(new_src, None).unwrap();
        let changes = AstDiffer::diff_function(
            first_function(&old_tree),
            old_src,
            first_function(&new_tree),
            new_src,
        );
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ChangeDetail::ReturnTypeChanged { .. })),
            "should detect return type change: {changes:?}"
        );
    }

    #[test]
    fn detect_visibility_change() {
        let old_src = "fn internal() {}";
        let new_src = "pub fn internal() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let old_tree = parser.parse(old_src, None).unwrap();
        let new_tree = parser.parse(new_src, None).unwrap();
        let changes = AstDiffer::diff_function(
            first_function(&old_tree),
            old_src,
            first_function(&new_tree),
            new_src,
        );
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ChangeDetail::VisibilityChanged { .. })),
            "should detect visibility change: {changes:?}"
        );
    }

    #[test]
    fn body_whitespace_only_is_unchanged() {
        let old_src = "fn foo() {\n    let x = 1;\n}";
        let new_src = "fn foo() {\n        let x = 1;\n}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let old_tree = parser.parse(old_src, None).unwrap();
        let new_tree = parser.parse(new_src, None).unwrap();
        let changes = AstDiffer::diff_function(
            first_function(&old_tree),
            old_src,
            first_function(&new_tree),
            new_src,
        );
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ChangeDetail::BodyUnchanged)),
            "whitespace-only body change should be BodyUnchanged: {changes:?}"
        );
    }

    #[test]
    fn body_real_change_is_modified() {
        let old_src = "fn foo() { let x = 1; }";
        let new_src = "fn foo() { let x = 2; let y = 3; }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let old_tree = parser.parse(old_src, None).unwrap();
        let new_tree = parser.parse(new_src, None).unwrap();
        let changes = AstDiffer::diff_function(
            first_function(&old_tree),
            old_src,
            first_function(&new_tree),
            new_src,
        );
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ChangeDetail::BodyModified { .. })),
            "real body change should be BodyModified: {changes:?}"
        );
    }

    #[test]
    fn format_oneline_shows_all_changes() {
        let diff = crate::domain::diff::SymbolDiff {
            name: "validate".into(),
            file: "src/lib.rs".into(),
            line: 42,
            parent_scope: Some("CommitValidator".into()),
            changes: vec![
                ChangeDetail::ParamAdded("strict: bool".into()),
                ChangeDetail::ReturnTypeChanged {
                    old: "bool".into(),
                    new: "Result<()>".into(),
                },
                ChangeDetail::BodyModified {
                    additions: 5,
                    deletions: 2,
                },
            ],
        };
        let line = diff.format_oneline();
        assert!(
            line.contains("CommitValidator::"),
            "should show parent scope"
        );
        assert!(
            line.contains("+param strict: bool"),
            "should show added param"
        );
        assert!(
            line.contains("return bool \u{2192} Result<()>"),
            "should show return change"
        );
        assert!(
            line.contains("body modified (+5 -2)"),
            "should show body change"
        );
    }

    #[test]
    fn diff_struct_returns_empty_phase1() {
        let src = "struct Foo { x: i32 }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(src, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let changes = AstDiffer::diff_struct(node, src, node, src);
        assert!(
            changes.is_empty(),
            "Phase 1: diff_struct should return empty"
        );
    }
}
