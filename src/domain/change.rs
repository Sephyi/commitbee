// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileCategory {
    Source,
    Test,
    Config,
    Docs,
    Build,
    Other,
}

impl FileCategory {
    #[must_use]
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
            || path.starts_with(".gitlab-ci")
            || path.starts_with(".circleci/")
            || path_str.contains("/.circleci/")
            || matches!(
                name,
                "Dockerfile"
                    | "Containerfile"
                    | "docker-compose.yml"
                    | "docker-compose.yaml"
                    | "podman-compose.yml"
                    | "podman-compose.yaml"
                    | "compose.yml"
                    | "compose.yaml"
                    | "Makefile"
                    | "justfile"
                    | ".dockerignore"
                    | ".containerignore"
                    | "Jenkinsfile"
                    | "Procfile"
                    | "CMakeLists.txt"
                    | "Makefile.am"
                    | "configure.ac"
                    | ".travis.yml"
                    | "azure-pipelines.yml"
                    | "netlify.toml"
                    | "vercel.json"
                    | "fly.toml"
                    | "render.yaml"
                    | "railway.toml"
                    | "Earthfile"
                    | "Tiltfile"
                    | "skaffold.yaml"
                    | "helmfile.yaml"
                    | "Vagrantfile"
            )
            || ext == "dockerfile"
            || ext == "containerfile"
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
                | "biome.json"
                | "biome.jsonc"
                | "deno.json"
                | "deno.jsonc"
                | ".eslintrc"
                | ".eslintrc.json"
                | ".eslintrc.js"
                | ".prettierrc"
                | ".prettierrc.json"
                | "ruff.toml"
                | ".ruff.toml"
                | "setup.py"
                | "setup.cfg"
                | "tox.ini"
                | "Pipfile"
                | "Pipfile.lock"
                | "uv.lock"
                | "Gemfile"
                | "Gemfile.lock"
                | "Rakefile"
                | "pom.xml"
                | "build.gradle"
                | "build.gradle.kts"
                | "settings.gradle"
                | "settings.gradle.kts"
                | "mix.exs"
                | "pubspec.yaml"
                | "pubspec.lock"
                | "REUSE.toml"
                | ".editorconfig"
                | "flake.nix"
                | "flake.lock"
                | "renovate.json"
                | "dependabot.yml"
        ) {
            return Self::Config;
        }

        // Dotfiles with config extensions
        if name.starts_with('.') && matches!(ext, "json" | "yaml" | "yml" | "toml" | "ini" | "cfg")
        {
            return Self::Config;
        }

        // By extension - source code
        match ext {
            "rs" | "ts" | "js" | "py" | "go" | "tsx" | "jsx" | "java" | "kt" | "c" | "cpp"
            | "h" | "hpp" | "cs" | "rb" | "swift" | "scala" | "ex" | "exs" | "php" | "r"
            | "lua" | "zig" | "nim" | "dart" | "vue" | "svelte" | "ml" | "mli" | "hs" | "clj"
            | "cljs" | "erl" | "hrl" | "pl" | "pm" | "sh" | "bash" | "zsh" => Self::Source,
            _ => Self::Other,
        }
    }

    #[must_use]
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
    pub diff: Arc<str>,
    pub additions: usize,
    pub deletions: usize,
    pub category: FileCategory,
    pub is_binary: bool,
    /// Original path before rename (only set when status is Renamed)
    pub old_path: Option<PathBuf>,
    /// Similarity percentage for renames (0-100)
    pub rename_similarity: Option<u8>,
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
    #[must_use]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Get files sorted by category priority (source first)
    #[must_use]
    pub fn files_by_priority(&self) -> Vec<&FileChange> {
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort_by_key(|f| f.category.priority());
        files
    }

    /// Create a subset containing only files matching the given paths.
    /// Recomputes DiffStats from the subset.
    #[must_use]
    pub fn subset(&self, paths: &[PathBuf]) -> StagedChanges {
        let files: Vec<FileChange> = self
            .files
            .iter()
            .filter(|f| paths.contains(&f.path))
            .cloned()
            .collect();

        let stats = DiffStats {
            files_changed: files.len(),
            insertions: files.iter().map(|f| f.additions).sum(),
            deletions: files.iter().map(|f| f.deletions).sum(),
        };

        StagedChanges { files, stats }
    }
}
