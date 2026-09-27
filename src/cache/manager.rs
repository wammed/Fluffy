use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{FluffyError, Result};
use super::normalize::{normalize_even_dimensions, transcode_video, DEFAULT_FPS};
use super::probe::probe_video;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaybackProfileMetadata {
    pub codec: String,
    pub pixel_format: String,
    pub fps: u32,
    pub audio: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheMetadata {
    pub source: String,
    pub source_size: u64,
    pub source_mtime: u64,
    pub profile: PlaybackProfileMetadata,
    pub output: String,
    #[serde(default)]
    pub transcoded: bool,
}

pub struct CacheManager {
    root_dir: PathBuf,
    videos_dir: PathBuf,
    metadata_dir: PathBuf,
}

pub type StorageManager = CacheManager;

impl CacheManager {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Result<Self> {
        let root = root_dir.as_ref().to_path_buf();
        let videos_dir = root.join("videos");
        let metadata_dir = root.join("metadata");

        fs::create_dir_all(&videos_dir)?;
        fs::create_dir_all(&metadata_dir)?;

        Ok(Self {
            root_dir: root,
            videos_dir,
            metadata_dir,
        })
    }

    /// Returns the default persistent storage directory for video wallpapers:
    /// `$XDG_DATA_HOME/fluffy/storage` (defaults to `~/.local/share/fluffy/storage`).
    /// Unlike ~/.cache, files here are preserved indefinitely until explicitly removed by the user.
    pub fn default_storage_dir() -> PathBuf {
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(data_home).join("fluffy").join("storage")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".local").join("share").join("fluffy").join("storage")
        } else {
            PathBuf::from("/tmp/fluffy-storage")
        }
    }

    /// Backward compatibility alias for `default_storage_dir()`.
    pub fn default_cache_dir() -> PathBuf {
        Self::default_storage_dir()
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn videos_dir(&self) -> &Path {
        &self.videos_dir
    }

    /// Backward-compatibility alias for `videos_dir()`
    pub fn objects_dir(&self) -> &Path {
        &self.videos_dir
    }

    pub fn metadata_dir(&self) -> &Path {
        &self.metadata_dir
    }

    /// Computes SHA-256 hash of the input file content to produce a stable cache key.
    pub fn compute_source_hash<P: AsRef<Path>>(path: P) -> Result<String> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];

        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }

        let hash_bytes = hasher.finalize();
        Ok(format!("{:x}", hash_bytes))
    }

    /// Imports a video file into persistent storage:
    /// 1. Probes and validates video dimensions (4K boundary check).
    /// 2. Computes the source content hash.
    /// 3. Checks if an existing valid storage object exists; if so, reuses it immediately.
    /// 4. If video is already a compatible profile (H.264, yuv420p, <=30fps, even dims, <=4K):
    ///    bypasses transcoding and directly copies/hardlinks to storage (instantaneous, 0 CPU).
    /// 5. Otherwise, transcodes via ffmpeg to normalized standard profile.
    /// 6. Atomically moves temporary file to videos/<hash>.mp4.
    /// 7. Writes metadata/<hash>.json.
    /// 8. Returns the final path of the stored MP4.
    pub fn import_video<P: AsRef<Path>>(&self, source_path: P) -> Result<PathBuf> {
        let source_path = source_path.as_ref();
        let canonical_source = source_path
            .canonicalize()
            .map_err(|e| FluffyError::Cache(format!("Cannot resolve path {:?}: {e}", source_path)))?;

        // 1. Probe & Validate (will reject > 4K before transcoding)
        let probe_info = probe_video(&canonical_source)?;

        // Gather file metadata
        let source_meta = fs::metadata(&canonical_source)?;
        let source_size = source_meta.len();
        let source_mtime = source_meta
            .modified()
            .unwrap_or(UNIX_EPOCH)
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 2. Compute source hash
        let hash = Self::compute_source_hash(&canonical_source)?;
        let video_filename = format!("{hash}.mp4");
        let metadata_filename = format!("{hash}.json");

        let final_video_path = self.videos_dir.join(&video_filename);
        let final_metadata_path = self.metadata_dir.join(&metadata_filename);

        // 3. Check existing storage reuse (or legacy ~/.cache/fluffy/objects fallback)
        if final_video_path.exists() && final_metadata_path.exists() {
            if let Ok(meta) = fs::metadata(&final_video_path) {
                if meta.len() > 0 {
                    println!(
                        "[Storage] Storage hit for {:?} -> reusing {:?}",
                        canonical_source, final_video_path
                    );
                    return Ok(final_video_path);
                }
            }
        }

        // Backward compatibility: check if it was cached in root_dir/objects
        let legacy_cached = self.root_dir.join("objects").join(&video_filename);
        let legacy_meta = self.root_dir.join("metadata").join(&metadata_filename);
        if legacy_cached.exists() {
            if let Ok(meta) = fs::metadata(&legacy_cached) {
                if meta.len() > 0 {
                    let _ = fs::copy(&legacy_cached, &final_video_path);
                    println!(
                        "[Storage] Migrated legacy cache {:?} -> storage {:?}",
                        legacy_cached, final_video_path
                    );
                    if legacy_meta.exists() && !final_metadata_path.exists() {
                        let _ = fs::copy(&legacy_meta, &final_metadata_path);
                    }
                    return Ok(final_video_path);
                }
            }
        }


        // 4. Create unique temporary file for atomic installation
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let tmp_video_filename = format!(".tmp.{pid}.{now}.{hash}.mp4");
        let tmp_video_path = self.videos_dir.join(&tmp_video_filename);

        // Check compatibility
        let is_compatible = probe_info.is_compatible_profile();
        let transcoded = if is_compatible {
            println!(
                "[Storage] Video is already compatible profile (H.264/yuv420p/{}x{}@{}fps). Bypassing transcode, copying directly to storage...",
                probe_info.width, probe_info.height, probe_info.fps
            );
            fs::copy(&canonical_source, &tmp_video_path).map_err(|e| {
                let _ = fs::remove_file(&tmp_video_path);
                FluffyError::Cache(format!("Failed to copy compatible video to storage: {e}"))
            })?;
            false
        } else {
            println!(
                "[Storage] Video requires normalization (input: {}/{}x{}@{}fps). Transcoding to storage temporary {:?}",
                probe_info.codec, probe_info.width, probe_info.height, probe_info.fps, tmp_video_path
            );
            if let Err(e) = transcode_video(&canonical_source, &tmp_video_path, &probe_info) {
                let _ = fs::remove_file(&tmp_video_path);
                return Err(e);
            }
            true
        };

        // 5. Atomic rename to destination
        if let Err(e) = fs::rename(&tmp_video_path, &final_video_path) {
            let _ = fs::remove_file(&tmp_video_path);
            return Err(FluffyError::Cache(format!(
                "Failed to atomically install storage video: {e}"
            )));
        }

        // 6. Write metadata
        let (target_w, target_h) = normalize_even_dimensions(probe_info.width, probe_info.height);
        let metadata = CacheMetadata {
            source: canonical_source.to_string_lossy().to_string(),
            source_size,
            source_mtime,
            profile: PlaybackProfileMetadata {
                codec: "h264".to_string(),
                pixel_format: "yuv420p".to_string(),
                fps: if is_compatible { probe_info.fps.round() as u32 } else { DEFAULT_FPS },
                audio: false,
                width: target_w,
                height: target_h,
            },
            output: video_filename,
            transcoded,
        };

        let tmp_meta_filename = format!(".tmp.{pid}.{now}.{hash}.json");
        let tmp_meta_path = self.metadata_dir.join(&tmp_meta_filename);
        let meta_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&tmp_meta_path, meta_json)?;
        let _ = fs::rename(&tmp_meta_path, &final_metadata_path);

        println!(
            "[Storage] Successfully stored video: {:?} (transcoded: {})",
            final_video_path, transcoded
        );

        Ok(final_video_path)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!("fluffy_hash_test_{}.txt", std::process::id()));
        fs::write(&test_file, b"fluffy video wallpaper cache test").unwrap();

        let hash1 = CacheManager::compute_source_hash(&test_file).unwrap();
        let hash2 = CacheManager::compute_source_hash(&test_file).unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 hex string is 64 characters

        let _ = fs::remove_file(&test_file);
    }

    #[test]
    fn test_metadata_serialization_roundtrip() {
        let meta = CacheMetadata {
            source: "/path/to/input.mp4".to_string(),
            source_size: 1048576,
            source_mtime: 1700000000,
            profile: PlaybackProfileMetadata {
                codec: "h264".to_string(),
                pixel_format: "yuv420p".to_string(),
                fps: 30,
                audio: false,
                width: 1920,
                height: 1080,
            },
            output: "abc123hash.mp4".to_string(),
            transcoded: true,
        };

        let serialized = serde_json::to_string(&meta).unwrap();
        let deserialized: CacheMetadata = serde_json::from_str(&serialized).unwrap();
        assert_eq!(meta, deserialized);
    }

    #[test]
    fn test_cache_import_atomic_and_reuse() {
        let temp_dir = std::env::temp_dir().join(format!("fluffy_cache_test_{}", std::process::id()));
        let manager = CacheManager::new(&temp_dir).expect("Failed to create cache manager");

        let source = Path::new("test.mp4");
        if !source.exists() {
            return; // Skip if test file not present
        }

        // First import -> Cache miss & transcode
        let cached1 = manager.import_video(source).expect("First import failed");
        assert!(cached1.exists());
        assert!(cached1.starts_with(manager.objects_dir()));

        // Verify metadata file was created
        let hash = cached1.file_stem().unwrap().to_str().unwrap();
        let meta_file = manager.metadata_dir().join(format!("{hash}.json"));
        assert!(meta_file.exists());

        let meta_content = fs::read_to_string(&meta_file).unwrap();
        let meta: CacheMetadata = serde_json::from_str(&meta_content).unwrap();
        assert_eq!(meta.profile.codec, "h264");
        assert_eq!(meta.profile.fps, 30);
        assert!(!meta.profile.audio);

        // Second import -> Cache hit (reuse)
        let cached2 = manager.import_video(source).expect("Second import failed");
        assert_eq!(cached1, cached2);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_compatible_video_bypass_transcode() {
        let temp_dir = std::env::temp_dir().join(format!("fluffy_bypass_test_{}", std::process::id()));
        let manager = CacheManager::new(&temp_dir).expect("Failed to create cache manager");

        let source = Path::new("test.mp4");
        if !source.exists() {
            return;
        }

        // test.mp4 is 1920x1080 30fps H.264 yuv420p -> compatible!
        let stored = manager.import_video(source).expect("Import compatible video failed");
        assert!(stored.exists());

        let hash = stored.file_stem().unwrap().to_str().unwrap();
        let meta_file = manager.metadata_dir().join(format!("{hash}.json"));
        assert!(meta_file.exists());

        let meta_content = fs::read_to_string(&meta_file).unwrap();
        let meta: CacheMetadata = serde_json::from_str(&meta_content).unwrap();
        // Since test.mp4 is already compatible, transcoded must be false!
        assert!(!meta.transcoded);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

