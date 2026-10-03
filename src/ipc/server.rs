use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use super::protocol::{MAX_REQUEST_SIZE, RequestEnvelope, ResponseEnvelope};
use crate::error::{FluffyError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);

pub struct PendingRequest {
    pub client_id: ClientId,
    pub request: RequestEnvelope,
}

struct ConnectedClient {
    stream: UnixStream,
    buffer: Vec<u8>,
    connected_at: Instant,
}

pub struct IpcServer {
    socket_path: PathBuf,
    listener: UnixListener,
    clients: HashMap<ClientId, ConnectedClient>,
    next_client_id: u64,
}

impl IpcServer {
    /// Binds the IPC server to the specified socket path.
    /// Safely cleans up stale sockets if a previous instance crashed.
    pub fn bind<P: AsRef<Path>>(socket_path: P) -> Result<Self> {
        let socket_path = socket_path.as_ref().to_path_buf();

        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if socket_path.exists() {
            // Check if another daemon is already running
            match UnixStream::connect(&socket_path) {
                Ok(_) => {
                    return Err(FluffyError::Ipc(format!(
                        "Another Fluffy daemon instance is already running at {:?}",
                        socket_path
                    )));
                }
                Err(_) => {
                    // Socket file exists but not responding; remove stale socket
                    if let Err(e) = fs::remove_file(&socket_path)
                        && e.kind() != std::io::ErrorKind::NotFound
                    {
                        tracing::warn!(operation = "socket_cleanup", error = %e, path = ?socket_path, "[IPC] Failed to remove stale socket file");
                    }
                }
            }
        }

        let listener = UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;

        // Ensure restricted permissions (0600 - user only)
        if let Ok(metadata) = fs::metadata(&socket_path) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600);
            if let Err(e) = fs::set_permissions(&socket_path, permissions) {
                tracing::warn!(operation = "socket_permissions", error = %e, path = ?socket_path, "[IPC] Failed to set permissions (0600) on socket file");
            }
        }

        tracing::info!(operation = "ipc_listen", socket = ?socket_path, "[IPC] Server listening on Unix socket");

        Ok(Self {
            socket_path,
            listener,
            clients: HashMap::new(),
            next_client_id: 1,
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Polls for incoming connections and incoming requests non-blockingly.
    pub fn poll_requests(&mut self) -> Result<Vec<PendingRequest>> {
        // 1. Accept any pending connections
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true)?;
                    let id = ClientId(self.next_client_id);
                    self.next_client_id += 1;
                    self.clients.insert(
                        id,
                        ConnectedClient {
                            stream,
                            buffer: Vec::new(),
                            connected_at: Instant::now(),
                        },
                    );
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => {
                    return Err(FluffyError::Io(e));
                }
            }
        }

        let mut pending = Vec::new();
        let mut disconnected = Vec::new();
        let idle_timeout = Duration::from_secs(60);

        // 2. Poll existing clients for data
        for (&client_id, client) in self.clients.iter_mut() {
            // Check idle connection timeout
            if client.connected_at.elapsed() > idle_timeout && client.buffer.is_empty() {
                tracing::debug!(
                    client_id = client_id.0,
                    "[IPC] Idle client connection timed out"
                );
                disconnected.push(client_id);
                continue;
            }

            let mut chunk = [0u8; 4096];
            match client.stream.read(&mut chunk) {
                Ok(0) => {
                    // EOF: client closed connection
                    disconnected.push(client_id);
                    continue;
                }
                Ok(n) => {
                    client.buffer.extend_from_slice(&chunk[..n]);

                    if client.buffer.len() > MAX_REQUEST_SIZE {
                        let resp = ResponseEnvelope::failure(
                            0,
                            format!("Request exceeded max size of {} bytes", MAX_REQUEST_SIZE),
                        );
                        let _ = Self::send_response_to_stream(&mut client.stream, &resp);
                        disconnected.push(client_id);
                        continue;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data ready right now
                }
                Err(_) => {
                    disconnected.push(client_id);
                    continue;
                }
            }

            // Check if buffer contains a complete newline-terminated line
            while let Some(newline_pos) = client.buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = client.buffer.drain(..=newline_pos).collect();
                let trimmed = String::from_utf8_lossy(&line_bytes);
                let trimmed = trimmed.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match serde_json::from_str::<RequestEnvelope>(trimmed) {
                    Ok(req) => {
                        if let Err(err) = req.validate() {
                            let resp = ResponseEnvelope::failure(req.request_id, err.to_string());
                            let _ = Self::send_response_to_stream(&mut client.stream, &resp);
                            disconnected.push(client_id);
                            break;
                        } else {
                            pending.push(PendingRequest {
                                client_id,
                                request: req,
                            });
                        }
                    }
                    Err(e) => {
                        let resp = ResponseEnvelope::failure(0, format!("Malformed JSON: {e}"));
                        let _ = Self::send_response_to_stream(&mut client.stream, &resp);
                        disconnected.push(client_id);
                        break;
                    }
                }
            }
        }

        // Clean up disconnected clients
        for id in disconnected {
            self.clients.remove(&id);
        }

        Ok(pending)
    }

    /// Sends a response to the specified client and closes the connection.
    pub fn respond(&mut self, client_id: ClientId, response: &ResponseEnvelope) -> Result<()> {
        if let Some(mut client) = self.clients.remove(&client_id) {
            Self::send_response_to_stream(&mut client.stream, response)?;
        }
        Ok(())
    }

    fn send_response_to_stream(stream: &mut UnixStream, response: &ResponseEnvelope) -> Result<()> {
        let mut data = serde_json::to_vec(response)?;
        data.push(b'\n');
        stream.write_all(&data)?;
        stream.flush()?;
        Ok(())
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        if self.socket_path.exists() {
            tracing::info!(operation = "socket_cleanup", socket = ?self.socket_path, "[IPC] Removing socket file during server shutdown");
            if let Err(e) = fs::remove_file(&self.socket_path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(operation = "socket_cleanup", error = %e, socket = ?self.socket_path, "[IPC] Failed to remove socket file on shutdown");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::client::IpcClient;
    use crate::ipc::protocol::CommandType;
    use std::time::Duration;

    #[test]
    fn test_ipc_server_client_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let socket_path = temp_dir.join(format!("test_fluffy_{}.sock", std::process::id()));

        // Start server
        let mut server = IpcServer::bind(&socket_path).expect("Failed to bind server");

        // Spawn client in another thread
        let client_socket = socket_path.clone();
        let client_handle = std::thread::spawn(move || {
            let client = IpcClient::new(client_socket);
            let req = RequestEnvelope::new(42, CommandType::Status);
            client.send(&req).expect("Failed to send request")
        });

        // Server receives and responds
        let mut received = false;
        for _ in 0..50 {
            let requests = server.poll_requests().unwrap();
            if let Some(req) = requests.into_iter().next() {
                assert_eq!(req.request.request_id, 42);
                assert_eq!(req.request.command, CommandType::Status);
                let resp = ResponseEnvelope::success(req.request.request_id, None);
                server.respond(req.client_id, &resp).unwrap();
                received = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(received, "Server should have received client request");
        let resp = client_handle.join().expect("Client thread panicked");
        assert_eq!(resp.request_id, 42);
        assert!(resp.success);
    }

    #[test]
    fn test_ipc_stale_socket_cleanup() {
        let temp_dir = std::env::temp_dir();
        let socket_path = temp_dir.join(format!("test_stale_{}.sock", std::process::id()));

        // Create a dummy file that looks like a stale socket
        fs::write(&socket_path, b"dummy").unwrap();
        assert!(socket_path.exists());

        // Binding should detect it's not a running server and remove it
        let server = IpcServer::bind(&socket_path).expect("Failed to bind over stale socket");
        assert!(socket_path.exists());
        drop(server);
        assert!(!socket_path.exists());
    }
}
