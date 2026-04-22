// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Contract tests for the `--porcelain` output mode.
//!
//! The contract: stdout contains exactly the sanitized commit message plus a
//! single trailing `\n`; nothing else. These tests defend that contract from
//! three angles:
//!
//! 1. **Structural lint** — count every `println!`/`print!` call in `src/` and
//!    fail when the count changes. Forces a reviewer to look at any new stdout
//!    writer and decide whether it violates the porcelain contract.
//! 2. **Argument-parse rejections** — verify clap rejects every incompatible
//!    flag combination at parse time (exit code 2, empty stdout).
//! 3. **Runtime rejections and safety smoke tests** — verify subcommand
//!    combinations fail early and that porcelain invocations exit within a
//!    reasonable timeout under error conditions (guards against future
//!    interactive prompts slipping into the porcelain code path and hanging
//!    on piped stdin).
//!
//! End-to-end byte-equality happy-path testing (wiremock + git fixture) is
//! deferred to a follow-up — it requires the full pipeline setup and belongs
//! in its own test file.

// Integration tests are synchronous and legitimately use `std::process::Command`
// to shell out to `git` / the `commitbee` binary; the `disallowed_methods` rule
// in clippy.toml targets async-context misuse, which does not apply here.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ─── Structural lint ─────────────────────────────────────────────────────────

/// Walks `src/` and counts every `println!` / `print!` macro call per file.
/// Fails if the count changes from the pinned allowlist. When this test fails,
/// a new stdout writer has appeared — inspect it and confirm it cannot reach
/// stdout under `--porcelain`, then update `EXPECTED` below.
///
/// This is the strongest single guard against future regressions: the contract
/// is "stdout writes are limited to a handful of known sites"; a drift triggers
/// human review every time.
#[test]
fn stdout_writers_stay_on_allowlist() {
    // Known-good allowlist — every file in `src/` that contains `println!` or
    // `print!` calls, with the current count. All listed call sites are either:
    //   - the porcelain stdout write itself (single `println!` in the dry-run
    //     branch), or
    //   - gated by flags that `--porcelain` does not set (e.g. `--clipboard`), or
    //   - reachable only via a subcommand (which `--porcelain` rejects at runtime).
    //
    // If you add or remove a `println!`/`print!` in `src/`, this test will fail
    // with a diff. Update the expected count only after verifying the new/removed
    // call respects the porcelain contract.
    let expected: BTreeMap<String, usize> = [
        // src/app.rs: clipboard / dry-run / non-TTY fallback / Init / Config
        // (~18 fields) / split dry-run / candidate display.
        ("app.rs".to_string(), 24_usize),
    ]
    .into_iter()
    .collect();

    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = collect_rs_files(&src_dir);
    let mut actual: BTreeMap<String, usize> = BTreeMap::new();
    for file in files {
        let content = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let count = count_stdout_macros(&content);
        if count > 0 {
            let rel = file
                .strip_prefix(&src_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            actual.insert(rel, count);
        }
    }

    assert_eq!(
        actual, expected,
        "\n\nStdout writer count in src/ changed.\n\n\
         If you added a `println!`/`print!` call: verify it cannot leak to stdout \
         under --porcelain (either flag-gated or reached only via a subcommand, \
         both of which --porcelain rejects), then update the EXPECTED map in this \
         test. If you removed one: just update the count.\n\n\
         The porcelain contract is \"stdout contains only the sanitized commit \
         message + one trailing newline.\" Every stdout writer is an opportunity \
         to violate that.\n"
    );
}

fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read_dir src/") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_rs_files(&path));
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    out
}

fn count_stdout_macros(content: &str) -> usize {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("println!(") || trimmed.starts_with("print!(")
        })
        .count()
}

// ─── Argument-parse rejection tests (clap-level conflicts) ───────────────────

/// `--porcelain --yes` must be rejected — `--yes` commits for real, porcelain
/// only generates and prints. Silently applying one while the user requested
/// the other would be deceptive.
#[test]
fn porcelain_plus_yes_rejected() {
    assert_parse_conflict(&["--porcelain", "--yes"]);
}

/// `--porcelain --clipboard` must be rejected at parse time with an empty
/// stdout and a non-zero exit.
#[test]
fn porcelain_plus_clipboard_rejected() {
    assert_parse_conflict(&["--porcelain", "--clipboard"]);
}

/// `--porcelain --show-prompt` must be rejected — silently dropping a debug
/// flag would be deceptive.
#[test]
fn porcelain_plus_show_prompt_rejected() {
    assert_parse_conflict(&["--porcelain", "--show-prompt"]);
}

/// `--porcelain --verbose` must be rejected — silently dropping the user's
/// verbosity choice would be deceptive.
#[test]
fn porcelain_plus_verbose_rejected() {
    assert_parse_conflict(&["--porcelain", "--verbose"]);
}

/// `--porcelain -n 3` must be rejected — multi-candidate generation has no
/// picker UI in porcelain mode and would silently discard all but the first.
#[test]
fn porcelain_plus_generate_rejected() {
    assert_parse_conflict(&["--porcelain", "-n", "3"]);
}

fn assert_parse_conflict(args: &[&str]) {
    let out = commitbee_cmd()
        .args(args)
        .output()
        .expect("spawn commitbee");
    assert!(
        !out.status.success(),
        "expected non-zero exit for {args:?}, got: {:?}",
        out.status
    );
    assert_eq!(
        out.stdout,
        b"",
        "expected empty stdout for {args:?}, got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    // clap typically exits with code 2 for usage errors; accept either 2 or 1
    // to stay resilient to clap-version defaults across the dependency range.
    match out.status.code() {
        Some(1 | 2) => {}
        other => panic!("expected exit code 1 or 2 for {args:?}, got: {other:?}"),
    }
}

// ─── Runtime rejection test (subcommand combination) ─────────────────────────

/// `--porcelain` with any subcommand must be rejected in `App::new`. clap
/// cannot cleanly express "this flag conflicts with every subcommand at once",
/// so the check is a runtime one. `config` is used here as the probe subcommand
/// because it requires no network or external state.
#[test]
fn porcelain_plus_subcommand_rejected() {
    let out = commitbee_cmd()
        .args(["--porcelain", "config"])
        .output()
        .expect("spawn commitbee");
    assert!(
        !out.status.success(),
        "expected non-zero exit for --porcelain + subcommand, got: {:?}",
        out.status
    );
    assert_eq!(
        out.stdout,
        b"",
        "expected empty stdout for --porcelain + subcommand, got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("porcelain") && stderr.to_lowercase().contains("subcommand"),
        "expected stderr to mention --porcelain + subcommand incompatibility, got: {stderr}"
    );
}

// ─── Safety smoke test (no hang on piped stdin) ──────────────────────────────

/// Porcelain invocations must never hang waiting on stdin. This test runs
/// `commitbee --porcelain` in a freshly-`git init`'d tempdir with no staged
/// changes, pipes an empty stdin, and asserts the process exits within 10s.
///
/// `git init` is required so gix discovery stops inside the tempdir rather
/// than walking up to an ancestor repo (e.g. the commitbee checkout when the
/// OS tempdir is inside the project tree). A dead `COMMITBEE_OLLAMA_HOST`
/// ensures that if any future change adds an LLM round-trip to the error
/// path, it also fails fast instead of timing out on a real Ollama instance.
///
/// Today all interactive prompts are gated behind `!self.cli.yes` (which
/// `--porcelain` sets); this test guards against a future change that adds a
/// new `dialoguer` call in a code path reachable from porcelain.
#[test]
fn porcelain_exits_within_timeout_with_no_staged_changes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let git_init = Command::new("git")
        .arg("init")
        .current_dir(tmp.path())
        .output()
        .expect("run `git init` in tempdir");
    assert!(
        git_init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&git_init.stderr)
    );

    let mut child = commitbee_cmd()
        .arg("--porcelain")
        .current_dir(tmp.path())
        // Point Ollama at a dead address so any unintended LLM round-trip
        // fails at TCP-connect time (ECONNREFUSED) rather than succeeding
        // or timing out on a real local Ollama.
        .env("COMMITBEE_OLLAMA_HOST", "http://127.0.0.1:1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn commitbee");

    // Close stdin immediately — a future regression that tries to read from
    // stdin would block on EOF rather than EINTR, keeping the failure mode
    // visible.
    drop(child.stdin.take());

    // 30s deadline — generous enough to absorb cold-start overhead (macOS
    // Gatekeeper / keyring prompts on a freshly-rebuilt binary can add ~10s
    // on the first spawn). The test's signal is "does it hang forever", not
    // "is it fast", so the extra headroom costs nothing semantically.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Non-zero exit is expected (no git repo / no staged changes).
                assert!(
                    !status.success(),
                    "expected non-zero exit, got success (stdout should be empty)"
                );
                let mut stdout = Vec::new();
                if let Some(mut s) = child.stdout.take() {
                    use std::io::Read;
                    s.read_to_end(&mut stdout).ok();
                }
                assert_eq!(
                    stdout,
                    b"",
                    "expected empty stdout on error, got: {}",
                    String::from_utf8_lossy(&stdout)
                );
                return;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    panic!(
                        "commitbee --porcelain did not exit within 30s. A new \
                         interactive prompt may have been added to a code path \
                         reachable from porcelain mode."
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("try_wait failed: {e}"),
        }
    }
}

// ─── Shared helpers ──────────────────────────────────────────────────────────

/// Build a `Command` for the `commitbee` binary under test. Preserves `PATH`
/// and `HOME` from the surrounding environment (so git and temp-dir lookups
/// work) while clearing `COMMITBEE_*` and `RUST_LOG` to isolate tests from the
/// developer's local setup.
fn commitbee_cmd() -> Command {
    let bin = commitbee_bin_path();
    let mut cmd = Command::new(bin);
    // Inherit PATH so the `git` binary is discoverable by `GitService`.
    // Inherit HOME/USERPROFILE so keyring and config discovery paths exist.
    // Strip anything that could leak into the test: COMMITBEE_*, RUST_LOG,
    // COMMITBEE_LOG, NO_COLOR, FORCE_COLOR, CLICOLOR_FORCE.
    for (k, _) in std::env::vars() {
        if k.starts_with("COMMITBEE_")
            || k == "RUST_LOG"
            || k == "NO_COLOR"
            || k == "FORCE_COLOR"
            || k == "CLICOLOR_FORCE"
        {
            cmd.env_remove(&k);
        }
    }
    cmd
}

/// Locate the `commitbee` binary that Cargo built for this test. Prefers the
/// env var Cargo sets when running tests (`CARGO_BIN_EXE_commitbee`); falls
/// back to the manifest-dir-relative target path so the test also works when
/// invoked manually.
fn commitbee_bin_path() -> PathBuf {
    if let Some(p) = option_env!("CARGO_BIN_EXE_commitbee") {
        return PathBuf::from(p);
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    #[cfg(debug_assertions)]
    let profile = "debug";
    #[cfg(not(debug_assertions))]
    let profile = "release";
    let ext = if cfg!(windows) { ".exe" } else { "" };
    manifest
        .join("target")
        .join(profile)
        .join(format!("commitbee{ext}"))
}
