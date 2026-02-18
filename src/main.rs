// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: GPL-3.0-only

use clap::Parser;
use tracing_subscriber::EnvFilter;

mod app;
mod cli;
mod config;
mod domain;
mod error;
mod services;

use app::App;
use cli::Cli;

#[tokio::main]
async fn main() {
    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .context_lines(2)
                .build(),
        )
    }))
    .ok();

    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("commitbee=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("commitbee=warn"))
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(std::env::var("NO_COLOR").is_err())
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
                std::process::exit(0);
            }
            _ => {
                eprintln!("{:?}", miette::Report::new(e));
                std::process::exit(1);
            }
        }
    }
}
