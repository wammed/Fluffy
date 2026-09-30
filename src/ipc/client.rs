use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::Duration,
};

use super::protocol::{CommandType, DaemonStatus, RequestEnvelope, ResponseEnvelope};
use crate::error::{FluffyError, Result};

pub struct IpcClient {
    socket_path: PathBuf,
    timeout: Duration,
}

impl IpcClient {
    pub fn new<P: AsRef<Path>>(socket_path: P) -> Self {
        Self::with_timeout(socket_path, Duration::from_secs(5))
    }

    pub fn with_timeout<P: AsRef<Path>>(socket_path: P, timeout: Duration) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            timeout,
        }
    }

    /// Sends a request to the daemon and waits for a response with a timeout.
    pub fn send(&self, request: &RequestEnvelope) -> Result<ResponseEnvelope> {
        let mut stream = UnixStream::connect(&self.socket_path).map_err(|e| {
            FluffyError::Ipc(format!(
                "Failed to connect to daemon at {:?}: {e}. Is the daemon running?",
                self.socket_path
            ))
        })?;

        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;

        let mut data = serde_json::to_vec(request)?;
        data.push(b'\n');
        stream.write_all(&data)?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        if response_line.trim().is_empty() {
            return Err(FluffyError::Ipc(
                "Daemon closed connection without response".to_string(),
            ));
        }

        let resp: ResponseEnvelope = serde_json::from_str(response_line.trim())?;
        Ok(resp)
    }

    pub fn status(&self) -> Result<DaemonStatus> {
        let req = RequestEnvelope::new(1, CommandType::Status);
        let resp = self.send(&req)?;

        if !resp.success {
            return Err(FluffyError::Ipc(
                resp.error
                    .unwrap_or_else(|| "Unknown daemon error".to_string()),
            ));
        }

        let data = resp
            .data
            .ok_or_else(|| FluffyError::Ipc("Empty status payload in response".to_string()))?;
        let status: DaemonStatus = serde_json::from_value(data)?;
        Ok(status)
    }

    pub fn set_video<P: AsRef<Path>>(
        &self,
        path: P,
        output: Option<&str>,
        generation: Option<u64>,
    ) -> Result<()> {
        let mut req = RequestEnvelope::new(1, CommandType::SetVideo).with_path(path.as_ref());
        if let Some(out) = output {
            req = req.with_output(out);
        }
        if let Some(target_gen) = generation {
            req = req.with_generation(target_gen);
        }

        let resp = self.send(&req)?;
        if !resp.success {
            return Err(FluffyError::Ipc(
                resp.error
                    .unwrap_or_else(|| "Failed to set video".to_string()),
            ));
        }
        Ok(())
    }

    pub fn pause(&self, output: Option<&str>) -> Result<()> {
        let mut req = RequestEnvelope::new(1, CommandType::Pause);
        if let Some(out) = output {
            req = req.with_output(out);
        }
        let resp = self.send(&req)?;
        if !resp.success {
            return Err(FluffyError::Ipc(
                resp.error
                    .unwrap_or_else(|| "Failed to pause playback".to_string()),
            ));
        }
        Ok(())
    }

    pub fn resume(&self, output: Option<&str>) -> Result<()> {
        let mut req = RequestEnvelope::new(1, CommandType::Resume);
        if let Some(out) = output {
            req = req.with_output(out);
        }
        let resp = self.send(&req)?;
        if !resp.success {
            return Err(FluffyError::Ipc(
                resp.error
                    .unwrap_or_else(|| "Failed to resume playback".to_string()),
            ));
        }
        Ok(())
    }

    pub fn stop(&self, output: Option<&str>) -> Result<()> {
        let mut req = RequestEnvelope::new(1, CommandType::Stop);
        if let Some(out) = output {
            req = req.with_output(out);
        }
        let resp = self.send(&req)?;
        if !resp.success {
            return Err(FluffyError::Ipc(
                resp.error
                    .unwrap_or_else(|| "Failed to stop playback".to_string()),
            ));
        }
        Ok(())
    }

    pub fn reload(&self) -> Result<()> {
        let req = RequestEnvelope::new(1, CommandType::Reload);
        let resp = self.send(&req)?;
        if !resp.success {
            return Err(FluffyError::Ipc(
                resp.error.unwrap_or_else(|| "Failed to reload".to_string()),
            ));
        }
        Ok(())
    }

    pub fn mark(&self, label: &str) -> Result<()> {
        let req = RequestEnvelope::new(1, CommandType::Mark).with_label(label);
        let resp = self.send(&req)?;
        if !resp.success {
            return Err(FluffyError::Ipc(
                resp.error
                    .unwrap_or_else(|| "Failed to record mark".to_string()),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        thread,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_socket_path() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("fluffy_test_{}_{}.sock", std::process::id(), id));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn test_ipc_daemon_unavailable() {
        let p = temp_socket_path();
        let client = IpcClient::with_timeout(&p, Duration::from_millis(100));
        let res = client.status();
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to connect to daemon"));
    }

    #[test]
    fn test_ipc_timeout() {
        let p = temp_socket_path();
        let listener = UnixListener::bind(&p).unwrap();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Server accepts connection but sleeps without replying
        let srv = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut reader = BufReader::new(&mut stream);
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                // Sleep longer than client timeout
                thread::sleep(Duration::from_millis(500));
            }
            running_clone.store(false, Ordering::SeqCst);
        });

        let client = IpcClient::with_timeout(&p, Duration::from_millis(100));
        let start = std::time::Instant::now();
        let res = client.status();
        let elapsed = start.elapsed();

        assert!(res.is_err(), "Request should fail due to timeout");
        assert!(
            elapsed >= Duration::from_millis(90),
            "Elapsed {:?} should be around 100ms",
            elapsed
        );
        assert!(
            elapsed < Duration::from_millis(450),
            "Should have timed out well before server finishes"
        );

        let _ = srv.join();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn test_ipc_normal_response() {
        let p = temp_socket_path();
        let listener = UnixListener::bind(&p).unwrap();

        let srv = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();

            let status_json = serde_json::json!({
                "daemon_version": "0.1.0",
                "outputs": [{
                    "name": "DP-1",
                    "state": "Playing",
                    "current_video": "/tmp/test.mp4",
                    "generation": 1,
                    "loop_count": 0,
                    "width": 2560,
                    "height": 1440,
                    "scale": 1
                }],
                "is_converting": false,
                "converting_file": null,
                "active_jobs": []
            });

            let resp = ResponseEnvelope::success(1, Some(status_json));
            let mut resp_data = serde_json::to_vec(&resp).unwrap();
            resp_data.push(b'\n');
            stream.write_all(&resp_data).unwrap();
            stream.flush().unwrap();
        });

        let client = IpcClient::with_timeout(&p, Duration::from_millis(500));
        let status = client.status().unwrap();
        assert_eq!(status.daemon_version, "0.1.0");
        assert_eq!(status.outputs.len(), 1);
        assert_eq!(status.outputs[0].name, "DP-1");
        assert_eq!(status.outputs[0].width, 2560);
        assert_eq!(status.outputs[0].height, 1440);

        let _ = srv.join();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn test_ipc_repeated_polling() {
        let p = temp_socket_path();
        let listener = UnixListener::bind(&p).unwrap();

        let srv = thread::spawn(move || {
            for i in 1..=5 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();

                let status_json = serde_json::json!({
                    "daemon_version": "0.1.0",
                    "outputs": [],
                    "is_converting": false,
                    "converting_file": null,
                    "active_jobs": []
                });

                let resp = ResponseEnvelope::success(i, Some(status_json));
                let mut resp_data = serde_json::to_vec(&resp).unwrap();
                resp_data.push(b'\n');
                stream.write_all(&resp_data).unwrap();
                stream.flush().unwrap();
            }
        });

        let client = IpcClient::with_timeout(&p, Duration::from_millis(500));
        for _ in 0..5 {
            let status = client.status();
            assert!(status.is_ok());
        }

        let _ = srv.join();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn test_ipc_concurrent_requests() {
        let p = temp_socket_path();
        let listener = UnixListener::bind(&p).unwrap();

        let srv = thread::spawn(move || {
            for _ in 0..6 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let req: RequestEnvelope = serde_json::from_str(line.trim()).unwrap();

                let resp = ResponseEnvelope::success(req.request_id, None);
                let mut resp_data = serde_json::to_vec(&resp).unwrap();
                resp_data.push(b'\n');
                stream.write_all(&resp_data).unwrap();
                stream.flush().unwrap();
            }
        });

        let mut handles = Vec::new();
        for id in 1..=6 {
            let sock = p.clone();
            handles.push(thread::spawn(move || {
                let client = IpcClient::with_timeout(&sock, Duration::from_millis(1000));
                let req = RequestEnvelope::new(id, CommandType::Pause);
                let resp = client.send(&req).unwrap();
                assert!(resp.success);
                assert_eq!(resp.request_id, id);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let _ = srv.join();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn test_ipc_mark_request() {
        let p = temp_socket_path();
        let listener = UnixListener::bind(&p).unwrap();

        let srv = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let req: RequestEnvelope = serde_json::from_str(line.trim()).unwrap();
            assert_eq!(req.command, CommandType::Mark);
            assert_eq!(req.label.as_deref(), Some("browser-start"));

            let resp = ResponseEnvelope::success(req.request_id, None);
            let mut resp_data = serde_json::to_vec(&resp).unwrap();
            resp_data.push(b'\n');
            stream.write_all(&resp_data).unwrap();
            stream.flush().unwrap();
        });

        let client = IpcClient::with_timeout(&p, Duration::from_millis(1000));
        assert!(client.mark("browser-start").is_ok());

        let _ = srv.join();
        let _ = std::fs::remove_file(&p);
    }
}
