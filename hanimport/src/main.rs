mod cli;
mod config;
mod delegate;
mod paths;
mod unpack;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(err) = cli::run(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
