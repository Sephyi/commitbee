// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
// SPDX-License-Identifier: GPL-3.0-only

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitType {
    Feat,
    Fix,
    Refactor,
    Docs,
    Test,
    Chore,
    Style,
    Perf,
    Build,
    Ci,
    Revert,
}

impl CommitType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Feat => "feat",
            Self::Fix => "fix",
            Self::Refactor => "refactor",
            Self::Docs => "docs",
            Self::Test => "test",
            Self::Chore => "chore",
            Self::Style => "style",
            Self::Perf => "perf",
            Self::Build => "build",
            Self::Ci => "ci",
            Self::Revert => "revert",
        }
    }
}
