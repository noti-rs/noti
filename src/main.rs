mod backend;
mod cli;
mod config;
mod data;
mod dbus;

use clap::Parser;

use cli::Args;
use data::aliases::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    args.process().await
}
