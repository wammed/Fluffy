use std::path::PathBuf;

use clap::{Parser, Subcommand};
use fluffy::{
    error::Result,
    ipc::{IpcClient, default_socket_path},
};

#[derive(Parser, Debug)]
#[command(
    name = "fluffy-bench",
    author,
    version,
    about = "Fluffy Benchmark Utility",
    long_about = None
)]
pub struct BenchCli {
    /// Target daemon Unix socket path
    #[arg(long, global = true)]
    pub socket: Option<PathBuf>,

    #[command(subcommand)]
    pub command: BenchCommands,
}

#[derive(Subcommand, Debug)]
pub enum BenchCommands {
    /// Record a benchmark workload marker event in Fluffy's event log
    Mark {
        /// Label for benchmark marker
        label: String,
    },
}

fn main() -> Result<()> {
    let cli = BenchCli::parse();
    let socket = cli.socket.unwrap_or_else(default_socket_path);

    match cli.command {
        BenchCommands::Mark { label } => {
            let client = IpcClient::new(&socket);
            client.mark(&label)?;
            println!("[Benchmark] Mark recorded: {}", label);
        }
    }

    Ok(())
}
