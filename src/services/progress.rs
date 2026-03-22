// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::io::IsTerminal;

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

/// Contextual progress indicator that shows spinners during pipeline phases.
///
/// Suppressed automatically in non-TTY environments (pipes, git hooks).
/// Falls back to simple status messages when indicatif is not appropriate.
pub(crate) struct Progress {
    bar: Option<ProgressBar>,
}

impl Default for Progress {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Progress {
    /// Create a new progress indicator. Only shows spinners in interactive terminals.
    /// If verbose is true, spinners are disabled to avoid conflict with debug logs.
    pub fn new(verbose: bool) -> Self {
        let is_tty = std::io::stderr().is_terminal();
        Self {
            bar: if is_tty && !verbose {
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.cyan} {msg}")
                        .expect("valid template")
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
                );
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                Some(pb)
            } else {
                None
            },
        }
    }

    /// Update the spinner message to indicate the current phase.
    pub fn phase(&self, msg: &str) {
        if let Some(ref bar) = self.bar {
            bar.set_message(msg.to_string());
        } else {
            eprintln!("{} {}", style("→").cyan(), msg);
        }
    }

    /// Print an info message below the spinner without disrupting it.
    pub fn info(&self, msg: &str) {
        if let Some(ref bar) = self.bar {
            bar.suspend(|| {
                eprintln!("{} {}", style("info:").cyan(), msg);
            });
        } else {
            eprintln!("{} {}", style("info:").cyan(), msg);
        }
    }

    /// Print a warning message below the spinner without disrupting it.
    pub fn warning(&self, msg: &str) {
        if let Some(ref bar) = self.bar {
            bar.suspend(|| {
                eprintln!("{} {}", style("warning:").yellow().bold(), msg);
            });
        } else {
            eprintln!("{} {}", style("warning:").yellow().bold(), msg);
        }
    }

    /// Finish and clear the spinner.
    pub fn finish(&self) {
        if let Some(ref bar) = self.bar {
            bar.finish_and_clear();
        }
    }

    /// Take ownership of the underlying progress bar (for sending to spawned tasks).
    /// After calling this, the `Progress` struct will no longer display or clear the bar.
    pub fn take_bar(&mut self) -> Option<ProgressBar> {
        self.bar.take()
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        if let Some(ref bar) = self.bar {
            bar.finish_and_clear();
        }
    }
}
