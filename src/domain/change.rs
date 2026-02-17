// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCategory {
    Source,
    Test,
    Config,
    Docs,
    Build,
    Other,
}

impl FileCategory {
    pub fn from_path(path: &std::path::Path) -> Self {
        let path_str = path.to_string_lossy();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Test detection
        if name.contains("_test.")
            || name.contains(".test.")
            || name.contains("_spec.")
            || path.starts_with("tests/")
            || path_str.contains("/tests/")
            || path_str.contains("/test/")
        {
            return Self::Test;
        }

        // Docs detection
        if path.starts_with("docs/")
            || path_str.contains("/docs/")
            || matches!(ext, "md" | "rst" | "txt")
        {
            return Self::Docs;
        }

        // Build/CI detection
        if path.starts_with(".github/")
            || path_str.contains("/.github/")
            || matches!(
                name,
                "Dockerfile" | "docker-compose.yml" | "Makefile" | "justfile" | ".dockerignore"
            )
            || ext == "dockerfile"
        {
            return Self::Build;
        }

        // Config files
        if matches!(
            name,
            "Cargo.toml"
                | "Cargo.lock"
                | "package.json"
                | "package-lock.json"
                | "tsconfig.json"
                | "pyproject.toml"
                | ".gitignore"
                | ".env.example"
                | "go.mod"
                | "go.sum"
                | "bun.lockb"
        ) {
            return Self::Config;
        }

        // By extension - source code
        match ext {
            "rs" | "ts" | "js" | "py" | "go" | "tsx" | "jsx" | "java" | "kt" | "c" | "cpp"
            | "h" | "hpp" => Self::Source,
            _ => Self::Other,
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            Self::Source => 0,
            Self::Test => 1,
            Self::Config => 2,
            Self::Docs => 3,
            Self::Build => 4,
            Self::Other => 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub status: ChangeStatus,
    pub diff: String,
    pub additions: usize,
    pub deletions: usize,
    pub category: FileCategory,
    pub is_binary: bool,
}

#[derive(Debug, Default)]
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

#[derive(Debug)]
pub struct StagedChanges {
    pub files: Vec<FileChange>,
    pub stats: DiffStats,
}

impl StagedChanges {
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Get files sorted by category priority (source first)
    pub fn files_by_priority(&self) -> Vec<&FileChange> {
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort_by_key(|f| f.category.priority());
        files
    }
}
