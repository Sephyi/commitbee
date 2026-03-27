// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::path::PathBuf;

/// Structured description of how a symbol changed between old and new versions.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SymbolDiff {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub parent_scope: Option<String>,
    pub changes: Vec<ChangeDetail>,
}

/// A single semantic change within a symbol.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum ChangeDetail {
    ParamAdded(String),
    ParamRemoved(String),
    ParamTypeChanged {
        name: String,
        old_type: String,
        new_type: String,
    },
    ReturnTypeChanged {
        old: String,
        new: String,
    },
    VisibilityChanged {
        old: Option<String>,
        new: Option<String>,
    },
    AttributeAdded(String),
    AttributeRemoved(String),
    AsyncChanged(bool),
    GenericChanged {
        old: String,
        new: String,
    },
    BodyModified {
        additions: usize,
        deletions: usize,
    },
    BodyUnchanged,
    FieldAdded(String),
    FieldRemoved(String),
    FieldTypeChanged {
        name: String,
        old_type: String,
        new_type: String,
    },
    /// Unsafe block/function added (Rust)
    UnsafeAdded,
    /// Unsafe block/function removed (Rust)
    UnsafeRemoved,
    /// Derive macro added: e.g., ["Debug", "Clone"]
    DeriveAdded(Vec<String>),
    /// Derive macro removed
    DeriveRemoved(Vec<String>),
    /// Decorator added: e.g., "@staticmethod" (Python), "@Override" (Java)
    DecoratorAdded(String),
    /// Decorator removed
    DecoratorRemoved(String),
    /// Export added (JS/TS `export`)
    ExportAdded,
    /// Export removed
    ExportRemoved,
    /// Mutability changed on parameter (Rust `mut`)
    MutabilityChanged,
    /// Generic constraint/where clause changed
    GenericConstraintChanged,
}

#[allow(dead_code)]
impl SymbolDiff {
    /// Format as a concise one-line description for the LLM prompt.
    #[must_use]
    pub fn format_oneline(&self) -> String {
        let scope = self
            .parent_scope
            .as_ref()
            .map(|s| format!("{s}::"))
            .unwrap_or_default();
        let changes: Vec<String> = self.changes.iter().map(|c| c.format_short()).collect();
        format!("  {scope}{}(): {}", self.name, changes.join(", "))
    }
}

#[allow(dead_code)]
impl ChangeDetail {
    #[must_use]
    pub fn format_short(&self) -> String {
        match self {
            Self::ParamAdded(p) => format!("+param {p}"),
            Self::ParamRemoved(p) => format!("-param {p}"),
            Self::ParamTypeChanged {
                name,
                old_type,
                new_type,
            } => {
                format!("param {name} {old_type} \u{2192} {new_type}")
            }
            Self::ReturnTypeChanged { old, new } => format!("return {old} \u{2192} {new}"),
            Self::VisibilityChanged { old, new } => format!(
                "visibility {} \u{2192} {}",
                old.as_deref().unwrap_or("private"),
                new.as_deref().unwrap_or("private")
            ),
            Self::AttributeAdded(a) => format!("+attr {a}"),
            Self::AttributeRemoved(a) => format!("-attr {a}"),
            Self::AsyncChanged(is_async) => {
                if *is_async {
                    "+async".into()
                } else {
                    "-async".into()
                }
            }
            Self::GenericChanged { old, new } => format!("generics {old} \u{2192} {new}"),
            Self::BodyModified {
                additions,
                deletions,
            } => format!("body modified (+{additions} -{deletions})"),
            Self::BodyUnchanged => "signature only".into(),
            Self::FieldAdded(f) => format!("+field {f}"),
            Self::FieldRemoved(f) => format!("-field {f}"),
            Self::FieldTypeChanged {
                name,
                old_type,
                new_type,
            } => {
                format!("field {name} {old_type} \u{2192} {new_type}")
            }
            Self::UnsafeAdded => "+unsafe".into(),
            Self::UnsafeRemoved => "-unsafe".into(),
            Self::DeriveAdded(derives) => format!("+derive({})", derives.join(", ")),
            Self::DeriveRemoved(derives) => format!("-derive({})", derives.join(", ")),
            Self::DecoratorAdded(d) => format!("+{d}"),
            Self::DecoratorRemoved(d) => format!("-{d}"),
            Self::ExportAdded => "+export".into(),
            Self::ExportRemoved => "-export".into(),
            Self::MutabilityChanged => "mutability changed".into(),
            Self::GenericConstraintChanged => "generic constraints changed".into(),
        }
    }
}
