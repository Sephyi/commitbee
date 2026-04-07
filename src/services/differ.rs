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

        // Compare unsafe (Rust)
        let old_unsafe = Self::has_keyword(old_node, "unsafe");
        let new_unsafe = Self::has_keyword(new_node, "unsafe");
        if !old_unsafe && new_unsafe {
            changes.push(ChangeDetail::UnsafeAdded);
        } else if old_unsafe && !new_unsafe {
            changes.push(ChangeDetail::UnsafeRemoved);
        }

        // Compare derive attributes (Rust)
        let old_derives = Self::extract_derives(old_node, old_source);
        let new_derives = Self::extract_derives(new_node, new_source);
        Self::diff_derives(&old_derives, &new_derives, &mut changes);

        // Compare mutability (Rust)
        let old_has_mut = Self::has_mut_params(old_node, old_source);
        let new_has_mut = Self::has_mut_params(new_node, new_source);
        if old_has_mut != new_has_mut {
            changes.push(ChangeDetail::MutabilityChanged);
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

    /// Compare old and new versions of a struct, enum, or class definition.
    pub fn diff_struct(
        old_node: tree_sitter::Node,
        old_source: &str,
        new_node: tree_sitter::Node,
        new_source: &str,
    ) -> Vec<ChangeDetail> {
        let mut changes = Vec::new();

        // Compare visibility
        let old_vis = Self::extract_visibility(old_node, old_source);
        let new_vis = Self::extract_visibility(new_node, new_source);
        if old_vis != new_vis {
            changes.push(ChangeDetail::VisibilityChanged {
                old: old_vis,
                new: new_vis,
            });
        }

        // Compare derive attributes (Rust)
        let old_derives = Self::extract_derives(old_node, old_source);
        let new_derives = Self::extract_derives(new_node, new_source);
        Self::diff_derives(&old_derives, &new_derives, &mut changes);

        // Compare fields or variants
        let old_fields = Self::extract_fields(old_node, old_source);
        let new_fields = Self::extract_fields(new_node, new_source);
        Self::diff_fields(&old_fields, &new_fields, &mut changes);

        // Check for generic parameter changes
        let old_generics = Self::extract_generics(old_node, old_source);
        let new_generics = Self::extract_generics(new_node, new_source);
        if old_generics != new_generics
            && let (Some(o), Some(n)) = (old_generics, new_generics)
        {
            changes.push(ChangeDetail::GenericChanged { old: o, new: n });
        }

        changes
    }

    fn diff_derives(old: &[String], new: &[String], changes: &mut Vec<ChangeDetail>) {
        let added: Vec<String> = new.iter().filter(|d| !old.contains(d)).cloned().collect();
        if !added.is_empty() {
            changes.push(ChangeDetail::DeriveAdded(added));
        }
        let removed: Vec<String> = old.iter().filter(|d| !new.contains(d)).cloned().collect();
        if !removed.is_empty() {
            changes.push(ChangeDetail::DeriveRemoved(removed));
        }
    }

    fn extract_fields(node: tree_sitter::Node, source: &str) -> Vec<(String, String)> {
        let mut fields = Vec::new();
        // Find body-like node that contains field or variant definitions
        let body = (0..node.child_count())
            .filter_map(|i| {
                #[allow(clippy::cast_possible_truncation)]
                node.child(i as u32)
            })
            .find(|c| {
                matches!(
                    c.kind(),
                    "field_declaration_list"
                        | "ordered_field_declaration_list"
                        | "enum_variant_list"
                        | "enum_member_declaration_list"
                        | "class_body"
                        | "interface_body"
                        | "enum_body"
                )
            });

        if let Some(b) = body {
            for i in 0..b.child_count() {
                #[allow(clippy::cast_possible_truncation)]
                if let Some(child) = b.child(i as u32) {
                    // Skip symbols/punc
                    if child.is_extra() || !child.is_named() {
                        continue;
                    }

                    if matches!(
                        child.kind(),
                        "field_declaration"
                            | "public_field_definition"
                            | "property_declaration"
                            | "variable_declaration"
                    ) {
                        let name = child
                            .child_by_field_name("name")
                            .or_else(|| {
                                child
                                    .child_by_field_name("declarations")
                                    .and_then(|d| d.child(0))
                                    .and_then(|n| n.child_by_field_name("name"))
                            })
                            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("")
                            .to_string();
                        let typ = child
                            .child_by_field_name("type")
                            .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("")
                            .to_string();
                        if !name.is_empty() {
                            fields.push((name, typ));
                        }
                    } else if matches!(
                        child.kind(),
                        "enum_variant" | "enum_member_declaration" | "enum_constant"
                    ) {
                        let name = child
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("")
                            .to_string();
                        if !name.is_empty() {
                            fields.push((name, String::new()));
                        }
                    }
                }
            }
        }
        fields
    }

    fn diff_fields(
        old: &[(String, String)],
        new: &[(String, String)],
        changes: &mut Vec<ChangeDetail>,
    ) {
        let old_names: HashSet<&str> = old.iter().map(|(n, _)| n.as_str()).collect();
        let new_names: HashSet<&str> = new.iter().map(|(n, _)| n.as_str()).collect();

        // Added fields/variants
        for (name, typ) in new {
            if !old_names.contains(name.as_str()) && !name.is_empty() {
                let desc = if typ.is_empty() {
                    name.clone()
                } else {
                    format!("{name}: {typ}")
                };
                changes.push(ChangeDetail::FieldAdded(desc));
            }
        }

        // Removed fields/variants
        for (name, typ) in old {
            if !new_names.contains(name.as_str()) && !name.is_empty() {
                let desc = if typ.is_empty() {
                    name.clone()
                } else {
                    format!("{name}: {typ}")
                };
                changes.push(ChangeDetail::FieldRemoved(desc));
            }
        }

        // Type changes for fields that exist in both
        for (name, old_typ) in old {
            if let Some((_, new_typ)) = new.iter().find(|(n, _)| n == name)
                && old_typ != new_typ
                && !old_typ.is_empty()
                && !new_typ.is_empty()
            {
                changes.push(ChangeDetail::FieldTypeChanged {
                    name: name.clone(),
                    old_type: old_typ.clone(),
                    new_type: new_typ.clone(),
                });
            }
        }
    }

    fn extract_generics(node: tree_sitter::Node, source: &str) -> Option<String> {
        // Look for type_parameters child (Rust, TS, Java)
        (0..node.child_count())
            .filter_map(|i| {
                #[allow(clippy::cast_possible_truncation)]
                node.child(i as u32)
            })
            .find(|c| matches!(c.kind(), "type_parameters" | "type_parameter_list"))
            .and_then(|g| g.utf8_text(source.as_bytes()).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
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

    /// Check if a node or its `function_modifiers` child contains a keyword.
    ///
    /// In tree-sitter Rust, `unsafe fn` produces a `function_modifiers` child
    /// containing the `unsafe` token, rather than a bare `unsafe` child.
    fn has_keyword(node: tree_sitter::Node, keyword: &str) -> bool {
        (0..node.child_count())
            .filter_map(|i| {
                #[allow(clippy::cast_possible_truncation)]
                node.child(i as u32)
            })
            .any(|c| {
                if c.kind() == keyword {
                    return true;
                }
                // Check inside modifier wrapper nodes (e.g., function_modifiers)
                if c.kind().ends_with("_modifiers") || c.kind() == "modifiers" {
                    return (0..c.child_count())
                        .filter_map(|j| {
                            #[allow(clippy::cast_possible_truncation)]
                            c.child(j as u32)
                        })
                        .any(|gc| gc.kind() == keyword);
                }
                false
            })
    }

    /// Extract derive trait names from `#[derive(...)]` attributes on a node.
    fn extract_derives(node: tree_sitter::Node, source: &str) -> Vec<String> {
        let mut derives = Vec::new();
        for i in 0..node.child_count() {
            #[allow(clippy::cast_possible_truncation)]
            let Some(child) = node.child(i as u32) else {
                continue;
            };
            if !matches!(child.kind(), "attribute_item" | "attribute") {
                continue;
            }
            let Ok(raw) = child.utf8_text(source.as_bytes()) else {
                continue;
            };
            let text = raw.trim();
            if let Some(start) = text.find("derive(") {
                let inner = &text[start + 7..];
                if let Some(end) = inner.find(')') {
                    for item in inner[..end].split(',') {
                        let trimmed = item.trim();
                        if !trimmed.is_empty() {
                            derives.push(trimmed.to_string());
                        }
                    }
                }
            }
        }
        derives
    }

    fn has_mut_params(node: tree_sitter::Node, source: &str) -> bool {
        node.child_by_field_name("parameters")
            .map(|params| {
                params
                    .utf8_text(source.as_bytes())
                    .map(|text| text.contains("mut "))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
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
    fn diff_struct_detects_added_field() {
        let old_src = "struct Config { timeout: u64 }";
        let new_src = "struct Config { timeout: u64, retry: u32 }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let old_tree = parser.parse(old_src, None).unwrap();
        let new_tree = parser.parse(new_src, None).unwrap();
        let old_node = old_tree.root_node().child(0).unwrap();
        let new_node = new_tree.root_node().child(0).unwrap();
        let changes = AstDiffer::diff_struct(old_node, old_src, new_node, new_src);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ChangeDetail::FieldAdded(f) if f.contains("retry"))),
            "should detect added field 'retry': {changes:?}"
        );
    }

    #[test]
    fn diff_enum_detects_added_variant() {
        let old_src = "enum Color { Red, Green }";
        let new_src = "enum Color { Red, Green, Blue }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let old_tree = parser.parse(old_src, None).unwrap();
        let new_tree = parser.parse(new_src, None).unwrap();
        let old_node = old_tree.root_node().child(0).unwrap();
        let new_node = new_tree.root_node().child(0).unwrap();
        let changes = AstDiffer::diff_struct(old_node, old_src, new_node, new_src);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ChangeDetail::FieldAdded(f) if f == "Blue")),
            "should detect added variant 'Blue': {changes:?}"
        );
    }

    #[test]
    fn diff_struct_returns_empty_if_no_changes() {
        let src = "struct Foo { x: i32 }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(src, None).unwrap();
        let node = tree.root_node().child(0).unwrap();
        let changes = AstDiffer::diff_struct(node, src, node, src);
        assert!(changes.is_empty());
    }

    #[test]
    fn detect_unsafe_added() {
        let old_src = "fn process() {\n    do_thing();\n}\n";
        let new_src = "unsafe fn process() {\n    do_thing();\n}\n";
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
                .any(|c| matches!(c, ChangeDetail::UnsafeAdded)),
            "should detect unsafe added: {changes:?}"
        );
    }

    #[test]
    fn detect_mutability_change() {
        let old_src = "fn process(items: Vec<i32>) {\n    items.len();\n}\n";
        let new_src = "fn process(mut items: Vec<i32>) {\n    items.push(1);\n}\n";
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
                .any(|c| matches!(c, ChangeDetail::MutabilityChanged)),
            "should detect mutability change: {changes:?}"
        );
    }

    #[test]
    fn format_short_marker_variants() {
        assert_eq!(ChangeDetail::UnsafeAdded.format_short(), "+unsafe");
        assert_eq!(ChangeDetail::UnsafeRemoved.format_short(), "-unsafe");
        assert_eq!(
            ChangeDetail::DeriveAdded(vec!["Debug".into(), "Clone".into()]).format_short(),
            "+derive(Debug, Clone)"
        );
        assert_eq!(ChangeDetail::ExportAdded.format_short(), "+export");
        assert_eq!(
            ChangeDetail::MutabilityChanged.format_short(),
            "mutability changed"
        );
    }
}
