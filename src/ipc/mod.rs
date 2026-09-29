pub mod client;
pub mod protocol;
pub mod server;

use std::path::PathBuf;

pub use client::IpcClient;
pub use protocol::{
    CommandType, DaemonStatus, MAX_REQUEST_SIZE, OutputApplyResult, OutputStatus, RequestEnvelope,
    ResponseEnvelope, SetVideoResult,
};
pub use server::IpcServer;

/// Returns the default Unix domain socket path for Fluffy.
///
/// Priority:
/// 1. `FLUFFY_SOCK` environment variable
/// 2. `$XDG_RUNTIME_DIR/fluffy.sock`
/// 3. `/tmp/fluffy-<uid>.sock`
pub fn default_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("FLUFFY_SOCK") {
        return PathBuf::from(path);
    }

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("fluffy.sock");
    }

    let uid = unsafe { libc_getuid() };
    PathBuf::from(format!("/tmp/fluffy-{uid}.sock"))
}

unsafe fn libc_getuid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}
