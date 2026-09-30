use std::{env, path::PathBuf};

use fluffy::{
    error::{FluffyError, Result},
    ipc::{IpcClient, default_socket_path},
};

fn print_help() {
    println!(
        r#"Fluffy Benchmark Utility

USAGE:
    fluffy-bench [COMMAND] [OPTIONS]

COMMANDS:
    mark <LABEL>         Record a benchmark workload marker event in Fluffy's event log
    help, --help         Print this help message

OPTIONS:
    --socket <PATH>      Target daemon Unix socket path
"#
    );
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let raw_args = &args[1..];

    if raw_args.is_empty()
        || raw_args[0] == "help"
        || raw_args[0] == "--help"
        || raw_args[0] == "-h"
    {
        print_help();
        return Ok(());
    }

    let mut socket = default_socket_path();
    let mut label = None;
    let mut i = 0;

    while i < raw_args.len() {
        if raw_args[i] == "--socket" && i + 1 < raw_args.len() {
            socket = PathBuf::from(&raw_args[i + 1]);
            i += 2;
            continue;
        }
        if raw_args[i] == "mark" {
            if i + 1 < raw_args.len() && !raw_args[i + 1].starts_with("--") {
                label = Some(raw_args[i + 1].clone());
                i += 2;
                continue;
            }
        } else if !raw_args[i].starts_with("--") && label.is_none() {
            label = Some(raw_args[i].clone());
        }
        i += 1;
    }

    let label = label.ok_or_else(|| {
        FluffyError::Ipc(
            "Missing label for benchmark marker. Usage: fluffy-bench mark <LABEL>".to_string(),
        )
    })?;

    let client = IpcClient::new(&socket);
    client.mark(&label)?;
    println!("[Benchmark] Mark recorded: {}", label);
    Ok(())
}
