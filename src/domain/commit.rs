// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[allow(dead_code)]
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
    /// All valid commit type strings — single source of truth.
    pub const ALL: &[&str] = &[
        "feat", "fix", "refactor", "chore", "docs", "test", "style", "perf", "build", "ci",
        "revert",
    ];

    #[must_use]
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

    #[must_use]
    #[allow(dead_code)]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "feat" => Some(Self::Feat),
            "fix" => Some(Self::Fix),
            "refactor" => Some(Self::Refactor),
            "docs" => Some(Self::Docs),
            "test" => Some(Self::Test),
            "chore" => Some(Self::Chore),
            "style" => Some(Self::Style),
            "perf" => Some(Self::Perf),
            "build" => Some(Self::Build),
            "ci" => Some(Self::Ci),
            "revert" => Some(Self::Revert),
            _ => None,
        }
    }
}

impl std::fmt::Display for CommitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
