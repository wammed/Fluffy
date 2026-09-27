use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::fs::PermissionsExt,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
};

use crate::error::{FluffyError, Result};
use super::protocol::{RequestEnvelope, ResponseEnvelope, MAX_REQUEST_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);

pub struct PendingRequest {
    pub client_id: ClientId,
    pub request: RequestEnvelope,
}

pub struct IpcServer {
    socket_path: PathBuf,
    listener: UnixListener,
    clients: HashMap<ClientId, UnixStream>,
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
                    let _ = fs::remove_file(&socket_path);
                }
            }
        }

        let listener = UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;

        // Ensure restricted permissions (0600 - user only)
        if let Ok(metadata) = fs::metadata(&socket_path) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600);
            let _ = fs::set_permissions(&socket_path, permissions);
        }

        println!("[IPC] Server listening on Unix socket: {:?}", socket_path);

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
                    self.clients.insert(id, stream);
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

        // 2. Poll existing clients for data
        for (&client_id, stream) in self.clients.iter_mut() {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();

            match reader.read_line(&mut line) {
                Ok(0) => {
                    // EOF: client closed connection
                    disconnected.push(client_id);
                }
                Ok(n) => {
                    if n > MAX_REQUEST_SIZE {
                        // Oversized request error
                        let resp = ResponseEnvelope::failure(
                            0,
                            format!("Request exceeded max size of {} bytes", MAX_REQUEST_SIZE),
                        );
                        let _ = Self::send_response_to_stream(reader.get_mut(), &resp);
                        disconnected.push(client_id);
                        continue;
                    }

                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<RequestEnvelope>(trimmed) {
                        Ok(req) => {
                            if let Err(err) = req.validate() {
                                let resp = ResponseEnvelope::failure(req.request_id, err.to_string());
                                let _ = Self::send_response_to_stream(reader.get_mut(), &resp);
                                disconnected.push(client_id);
                            } else {
                                pending.push(PendingRequest {
                                    client_id,
                                    request: req,
                                });
                            }
                        }
                        Err(e) => {
                            let resp = ResponseEnvelope::failure(0, format!("Malformed JSON: {e}"));
                            let _ = Self::send_response_to_stream(reader.get_mut(), &resp);
                            disconnected.push(client_id);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data ready right now
                }
                Err(_) => {
                    disconnected.push(client_id);
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
        if let Some(mut stream) = self.clients.remove(&client_id) {
            Self::send_response_to_stream(&mut stream, response)?;
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
            println!("[IPC] Removing socket file: {:?}", self.socket_path);
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::ipc::protocol::CommandType;
    use crate::ipc::client::IpcClient;

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
