use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{FluffyError, Result};

pub const MAX_REQUEST_SIZE: usize = 65536; // 64 KB

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    Status,
    #[serde(alias = "set-video")]
    SetVideo,
    Pause,
    Resume,
    Stop,
    Reload,
    Mark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub request_id: u64,
    #[serde(default)]
    pub generation: Option<u64>,
    pub command: CommandType,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub label: Option<String>,
}

impl RequestEnvelope {
    pub fn new(request_id: u64, command: CommandType) -> Self {
        Self {
            request_id,
            generation: None,
            command,
            output: None,
            path: None,
            label: None,
        }
    }

    pub fn with_generation(mut self, generation: u64) -> Self {
        self.generation = Some(generation);
        self
    }

    pub fn with_output<S: Into<String>>(mut self, output: S) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn with_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_label<S: Into<String>>(mut self, label: S) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.command == CommandType::SetVideo && self.path.is_none() {
            return Err(FluffyError::Ipc(
                "'set_video' command requires a valid 'path' field".to_string(),
            ));
        }
        if self.command == CommandType::Mark && self.label.is_none() {
            return Err(FluffyError::Ipc(
                "'mark' command requires a valid 'label' field".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub request_id: u64,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ResponseEnvelope {
    pub fn success(request_id: u64, data: Option<serde_json::Value>) -> Self {
        Self {
            request_id,
            success: true,
            data,
            error: None,
        }
    }

    pub fn failure<S: Into<String>>(request_id: u64, error: S) -> Self {
        Self {
            request_id,
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputStatus {
    pub name: String,
    pub state: String,
    pub current_video: Option<String>,
    pub generation: u64,
    pub loop_count: u64,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub scale: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputApplyResult {
    pub name: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetVideoResult {
    pub generation: u64,
    pub video_path: PathBuf,
    pub outputs: Vec<OutputApplyResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Probing,
    Transcoding,
    Installing,
    Completed,
    Failed,
    Cancelled,
    Stale,
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Probing => write!(f, "probing"),
            Self::Transcoding => write!(f, "transcoding"),
            Self::Installing => write!(f, "installing"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Stale => write!(f, "stale"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversionJobInfo {
    pub job_id: u64,
    pub source: String,
    pub generation: u64,
    pub target_output: Option<String>,
    pub state: JobState,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub daemon_version: String,
    pub outputs: Vec<OutputStatus>,
    #[serde(default)]
    pub is_converting: bool,
    #[serde(default)]
    pub converting_file: Option<String>,
    #[serde(default)]
    pub active_jobs: Vec<ConversionJobInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_set_video() {
        let json = r#"{
            "request_id": 123,
            "generation": 42,
            "command": "set_video",
            "output": "DP-1",
            "path": "/home/user/video.mp4"
        }"#;

        let req: RequestEnvelope = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.request_id, 123);
        assert_eq!(req.generation, Some(42));
        assert_eq!(req.command, CommandType::SetVideo);
        assert_eq!(req.output.as_deref(), Some("DP-1"));
        assert_eq!(
            req.path.as_deref(),
            Some(std::path::Path::new("/home/user/video.mp4"))
        );

        req.validate().expect("Validation failed");
    }

    #[test]
    fn test_deserialize_alias_kebab_case() {
        let json = r#"{
            "request_id": 1,
            "command": "set-video",
            "path": "/video.mp4"
        }"#;

        let req: RequestEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(req.command, CommandType::SetVideo);
    }

    #[test]
    fn test_set_video_missing_path_fails_validation() {
        let req = RequestEnvelope {
            request_id: 1,
            generation: None,
            command: CommandType::SetVideo,
            output: None,
            path: None,
            label: None,
        };

        assert!(req.validate().is_err());
    }

    #[test]
    fn test_status_response_serialization() {
        let status = DaemonStatus {
            daemon_version: "0.1.0".to_string(),
            outputs: vec![OutputStatus {
                name: "DP-1".to_string(),
                state: "PLAYING".to_string(),
                current_video: Some("/path/to/test.mp4".to_string()),
                generation: 1,
                loop_count: 5,
                width: 2560,
                height: 1440,
                scale: 1,
            }],
            is_converting: false,
            converting_file: None,
            active_jobs: vec![ConversionJobInfo {
                job_id: 1,
                source: "/path/to/test.mp4".to_string(),
                generation: 1,
                target_output: Some("DP-1".to_string()),
                state: JobState::Transcoding,
                error: None,
            }],
        };

        let val = serde_json::to_value(&status).unwrap();
        let res = ResponseEnvelope::success(999, Some(val));
        let serialized = serde_json::to_string(&res).unwrap();

        let deserialized: ResponseEnvelope = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.request_id, 999);
        assert!(deserialized.success);
        assert!(deserialized.error.is_none());
        assert!(deserialized.data.is_some());
    }

    #[test]
    fn test_malformed_json_fails() {
        let malformed = "{ invalid json }";
        let res: std::result::Result<RequestEnvelope, _> = serde_json::from_str(malformed);
        assert!(res.is_err());
    }

    #[test]
    fn test_generation_semantics() {
        let current_gen = 10;
        let stale_req_gen = 9;
        let fresh_req_gen = 11;

        assert!(
            stale_req_gen < current_gen,
            "Stale request must be strictly less than current generation"
        );
        assert!(
            fresh_req_gen >= current_gen,
            "Fresh request must be greater than or equal to current generation"
        );
    }

    #[test]
    fn test_mark_command_serialization() {
        let req = RequestEnvelope::new(42, CommandType::Mark).with_label("browser-start");
        assert!(req.validate().is_ok());

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"command\":\"mark\""));
        assert!(json.contains("\"label\":\"browser-start\""));

        let deserialized: RequestEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.command, CommandType::Mark);
        assert_eq!(deserialized.label.as_deref(), Some("browser-start"));
    }

    #[test]
    fn test_mark_missing_label_validation() {
        let req = RequestEnvelope::new(42, CommandType::Mark);
        assert!(req.validate().is_err());
    }
}
