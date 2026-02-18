// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct CodeSymbol {
    pub kind: SymbolKind,
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub is_public: bool,
    pub is_added: bool,
}

impl std::fmt::Display for CodeSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let visibility = if self.is_public { "pub " } else { "" };
        let action = if self.is_added { "+" } else { "-" };
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
