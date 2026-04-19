// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

#![forbid(unsafe_code)]

use clap::Parser;
use commitbee::{App, Cli, error};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Under --porcelain, disable console's global color state — this governs
    // indicatif spinners, dialoguer prompts, and every `style(...)` call via
    // the `console` crate. stdout must remain exclusively the commit message.
    if cli.porcelain {
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }

    // Install miette hook with porcelain-aware rendering. Errors still flow to
    // stderr on failure, but under --porcelain we strip ANSI colors and OSC8
    // hyperlinks so stderr stays grep-friendly for scripting consumers.
    let porcelain = cli.porcelain;
    miette::set_hook(Box::new(move |_| {
        let mut opts = miette::MietteHandlerOpts::new()
            .context_lines(2)
            .terminal_links(!porcelain);
        if porcelain {
            opts = opts.graphical_theme(miette::GraphicalTheme::unicode_nocolor());
        }
        Box::new(opts.build())
    }))
    .ok();

    let filter = if cli.porcelain {
        EnvFilter::new("off")
    } else if cli.verbose {
        EnvFilter::new("commitbee=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("commitbee=warn"))
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(!cli.porcelain && std::env::var("NO_COLOR").is_err())
        .without_time()
        .init();

    let mut app = match App::new(cli) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("{:?}", miette::Report::new(e));
            std::process::exit(1);
        }
    };

    if let Err(e) = app.run().await {
        match e {
            error::Error::Cancelled => {
                eprintln!("Aborted.");
                std::process::exit(130);
            }
            _ => {
                eprintln!("{:?}", miette::Report::new(e));
                std::process::exit(1);
            }
        }
    }
}
