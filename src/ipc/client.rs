use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::error::{FluffyError, Result};
use super::protocol::{CommandType, DaemonStatus, RequestEnvelope, ResponseEnvelope};

pub struct IpcClient {
    socket_path: PathBuf,
}

impl IpcClient {
    pub fn new<P: AsRef<Path>>(socket_path: P) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
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

        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        let mut data = serde_json::to_vec(request)?;
        data.push(b'\n');
        stream.write_all(&data)?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        if response_line.trim().is_empty() {
            return Err(FluffyError::Ipc("Daemon closed connection without response".to_string()));
        }

        let resp: ResponseEnvelope = serde_json::from_str(response_line.trim())?;
        Ok(resp)
    }

    pub fn status(&self) -> Result<DaemonStatus> {
        let req = RequestEnvelope::new(1, CommandType::Status);
        let resp = self.send(&req)?;

        if !resp.success {
            return Err(FluffyError::Ipc(
                resp.error.unwrap_or_else(|| "Unknown daemon error".to_string()),
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
                resp.error.unwrap_or_else(|| "Failed to set video".to_string()),
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
                resp.error.unwrap_or_else(|| "Failed to pause playback".to_string()),
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
                resp.error.unwrap_or_else(|| "Failed to resume playback".to_string()),
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
                resp.error.unwrap_or_else(|| "Failed to stop playback".to_string()),
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
}
