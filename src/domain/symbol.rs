// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Class,
    Interface,
    Const,
    Type,
}

/// How a symbol was affected by the change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[allow(dead_code)]
pub(crate) enum SymbolChangeType {
    /// Symbol only exists in staged content (new symbol)
    Added,
    /// Symbol only exists in HEAD content (removed symbol)
    Removed,
    /// Symbol exists in both, body changed (non-whitespace)
    ModifiedBody,
    /// Symbol exists in both, signature changed (parameters, return type, etc.)
    ModifiedSignature,
    /// Symbol exists in both, only whitespace/indentation changed within span
    TouchedOnly,
}

#[derive(Debug, Clone)]
pub struct CodeSymbol {
    pub kind: SymbolKind,
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub end_line: usize,
    pub is_public: bool,
    pub is_added: bool,
    /// For symbols that exist in both HEAD and staged, indicates if only whitespace changed.
    /// None = symbol is purely added or removed, not a modification.
    pub is_whitespace_only: Option<bool>,
    /// Full signature extracted from tree-sitter AST (everything before the body).
    /// e.g., "pub fn connect(host: &str, timeout: Duration) -> Result<Connection>"
    /// None for languages or constructs where signature extraction isn't supported.
    pub signature: Option<String>,
}

impl CodeSymbol {
    /// Determine the change type for this symbol.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn change_type(&self) -> SymbolChangeType {
        match (self.is_added, self.is_whitespace_only) {
            (true, None) => SymbolChangeType::Added,
            (false, None) => SymbolChangeType::Removed,
            (true, Some(true)) => SymbolChangeType::TouchedOnly,
            (true, Some(false)) => SymbolChangeType::ModifiedBody,
            (false, Some(true)) => SymbolChangeType::TouchedOnly,
            (false, Some(false)) => SymbolChangeType::ModifiedBody,
        }
    }
}

impl std::fmt::Display for CodeSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let action = if self.is_added { "+" } else { "-" };
        if let Some(sig) = &self.signature {
            write!(
                f,
                "[{}] {} ({}:{})",
                action,
                sig,
                self.file.display(),
                self.line
            )
        } else {
            let visibility = if self.is_public { "pub " } else { "" };
            write!(
                f,
                "[{}] {}{:?} {} ({}:{})",
                action,
                visibility,
                self.kind,
                self.name,
                self.file.display(),
                self.line
            )
        }
    }
}
