use clap::Parser;
use console::style;

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
    let cli = Cli::parse();

    let mut app = match App::new(cli) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("{} {}", style("error:").red().bold(), e);
            std::process::exit(1);
        }
    };

    if let Err(e) = app.run().await {
        match e {
            error::Error::Cancelled => {
                eprintln!("{}", style("Aborted.").dim());
                std::process::exit(0);
            }
            _ => {
                eprintln!("{} {}", style("error:").red().bold(), e);
                std::process::exit(1);
            }
        }
    }
}
