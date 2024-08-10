mod cli;

use clap::Parser;

use cli::Args;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    args.process().await
}
