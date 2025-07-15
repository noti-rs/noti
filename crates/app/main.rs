mod cli;

use clap::Parser;

use cli::Args;

fn main() -> anyhow::Result<()> {
    setup_logger();

    let args = Args::parse();
    args.process()
}

fn setup_logger() {
    const ENV_NAME: &str = "NOTI_LOG";
    env_logger::Builder::from_env(env_logger::Env::default().filter_or(ENV_NAME, "info")).init();
}
