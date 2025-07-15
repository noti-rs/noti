mod cli;

use clap::Parser;

use cli::Args;

#[cfg(feature = "libc_alloc")]
#[global_allocator]
static GLOBAL: libc_alloc::LibcAlloc = libc_alloc::LibcAlloc;

fn main() -> anyhow::Result<()> {
    setup_logger();

    let args = Args::parse();
    args.process()
}

fn setup_logger() {
    const ENV_NAME: &str = "NOTI_LOG";
    env_logger::Builder::from_env(env_logger::Env::default().filter_or(ENV_NAME, "info")).init();
}
