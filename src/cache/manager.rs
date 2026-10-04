use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::normalize::{DEFAULT_FPS, normalize_even_dimensions, transcode_video_with_crossfade};

use super::probe::probe_video;
use crate::error::{FluffyError, Result};

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
    #[serde(default)]
    pub crossfade_ms: Option<u32>,
}

pub struct CacheManager {
    root_dir: PathBuf,
    videos_dir: PathBuf,
    metadata_dir: PathBuf,
}

pub type StorageManager = CacheManager;

fn is_file_older_than(path: &Path, duration: std::time::Duration) -> bool {
    if let Ok(metadata) = fs::metadata(path)
        && let Ok(modified) = metadata.modified()
        && let Ok(elapsed) = modified.elapsed()
    {
        return elapsed >= duration;
    }
    false
}

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

    /// Cleans up orphaned temporary files (`.tmp.*`) created by previous crashed
    /// or interrupted normalization tasks. Avoids removing files owned by active
    /// processes or files created very recently.
    pub fn cleanup_stale_temp_files(&self) -> Result<usize> {
        let mut cleaned = 0;
        let current_pid = std::process::id();

        for dir in &[&self.videos_dir, &self.metadata_dir] {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if !name_str.starts_with(".tmp.") {
                        continue;
                    }

                    // Filename pattern: .tmp.{pid}.{now}.{hash}.{ext}
                    let parts: Vec<&str> = name_str.split('.').collect();
                    let should_remove = if parts.len() >= 3 {
                        if let Ok(file_pid) = parts[2].parse::<u32>() {
                            if file_pid == current_pid {
                                // Never delete temp files created by the current process
                                false
                            } else {
                                // If the process that created this file is no longer alive, it's stale
                                !Path::new(&format!("/proc/{file_pid}")).exists()
                            }
                        } else {
                            // If PID not parsable, check file age (older than 10 minutes)
                            is_file_older_than(&entry.path(), std::time::Duration::from_secs(600))
                        }
                    } else {
                        // Fallback: check file age
                        is_file_older_than(&entry.path(), std::time::Duration::from_secs(600))
                    };

                    if should_remove && fs::remove_file(entry.path()).is_ok() {
                        cleaned += 1;
                    }
                }
            }
        }
        Ok(cleaned)
    }

    /// Returns the default persistent storage directory for video wallpapers:
    /// `$XDG_DATA_HOME/fluffy/storage` (defaults to `~/.local/share/fluffy/storage`).
    /// Unlike ~/.cache, files here are preserved indefinitely until explicitly removed by the user.
    pub fn default_storage_dir() -> PathBuf {
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(data_home).join("fluffy").join("storage")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("fluffy")
                .join("storage")
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
    ///    bypasses transcoding and directly copies/hardlinks to storage (instantaneous, no transcoding CPU cost).
    /// 5. Otherwise, transcodes via ffmpeg to normalized standard profile.
    /// 6. Atomically moves temporary file to videos/<hash>.mp4.
    /// 7. Writes metadata/<hash>.json.
    /// 8. Returns the final path of the stored MP4.
    pub fn import_video<P: AsRef<Path>>(&self, source_path: P) -> Result<PathBuf> {
        self.import_video_with_cancel(source_path, None)
    }

    /// Imports a video file into persistent storage with optional cancellation support.
    pub fn import_video_with_cancel<P: AsRef<Path>>(
        &self,
        source_path: P,
        cancel_token: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<PathBuf> {
        self.import_video_with_options(source_path, cancel_token, None)
    }

    /// Imports a video with seamless loop crossfade (specified in milliseconds).
    pub fn import_video_with_crossfade<P: AsRef<Path>>(
        &self,
        source_path: P,
        crossfade_ms: u32,
    ) -> Result<PathBuf> {
        self.import_video_with_options(source_path, None, Some(crossfade_ms))
    }

    /// Imports a video file into persistent storage with optional cancellation and loop crossfade:
    /// 1. Probes and validates video dimensions (4K boundary check).
    /// 2. Computes the source content hash.
    /// 3. Checks if an existing valid storage object exists; if so, reuses it immediately.
    /// 4. If video is already a compatible profile (and no crossfade is requested):
    ///    attempts instant zero-copy hardlink first, falling back to copy.
    /// 5. Otherwise, transcodes via ffmpeg to normalized standard profile (with optional loop crossfade).
    /// 6. Atomically moves temporary file to storage.
    /// 7. Writes metadata JSON.
    /// 8. Returns the final path of the stored MP4.
    pub fn import_video_with_options<P: AsRef<Path>>(
        &self,
        source_path: P,
        cancel_token: Option<&std::sync::atomic::AtomicBool>,
        crossfade_ms: Option<u32>,
    ) -> Result<PathBuf> {
        use std::sync::atomic::Ordering;

        if let Some(token) = cancel_token
            && token.load(Ordering::Relaxed)
        {
            return Err(FluffyError::JobCancelled);
        }

        let source_path = source_path.as_ref();
        let canonical_source = source_path.canonicalize().map_err(|e| {
            FluffyError::Cache(format!("Cannot resolve path {:?}: {e}", source_path))
        })?;

        // 1. Probe & Validate (will reject > 4K before transcoding)
        let probe_info = probe_video(&canonical_source)?;

        if let Some(token) = cancel_token
            && token.load(Ordering::Relaxed)
        {
            return Err(FluffyError::JobCancelled);
        }

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

        if let Some(token) = cancel_token
            && token.load(Ordering::Relaxed)
        {
            return Err(FluffyError::JobCancelled);
        }

        let effective_crossfade = crossfade_ms.filter(|&ms| ms > 0);
        let (video_filename, metadata_filename) = if let Some(ms) = effective_crossfade {
            (
                format!("{hash}_xfade_{ms}.mp4"),
                format!("{hash}_xfade_{ms}.json"),
            )
        } else {
            (format!("{hash}.mp4"), format!("{hash}.json"))
        };

        let final_video_path = self.videos_dir.join(&video_filename);
        let final_metadata_path = self.metadata_dir.join(&metadata_filename);

        // 3. Check existing storage reuse (or legacy storage migration)
        if final_video_path.exists()
            && final_metadata_path.exists()
            && let Ok(meta) = fs::metadata(&final_video_path)
            && meta.len() > 0
        {
            tracing::info!(
                operation = "cache_lookup",
                source = ?canonical_source,
                destination = ?final_video_path,
                "[Storage] Cache hit: reusing existing normalized video in storage"
            );
            return Ok(final_video_path);
        }

        // Backward compatibility: check if it was cached in root_dir/objects
        let legacy_cached = self.root_dir.join("objects").join(&video_filename);
        let legacy_meta = self.root_dir.join("metadata").join(&metadata_filename);
        if legacy_cached.exists()
            && let Ok(meta) = fs::metadata(&legacy_cached)
            && meta.len() > 0
        {
            if let Err(e) = fs::copy(&legacy_cached, &final_video_path) {
                tracing::warn!(operation = "legacy_migration", error = %e, "[Storage] Failed to copy legacy cache video to storage");
            } else {
                tracing::info!(
                    operation = "legacy_migration",
                    legacy = ?legacy_cached,
                    destination = ?final_video_path,
                    "[Storage] Migrated legacy cache to persistent storage"
                );
            }
            if legacy_meta.exists()
                && !final_metadata_path.exists()
                && let Err(e) = fs::copy(&legacy_meta, &final_metadata_path)
            {
                tracing::debug!(operation = "legacy_migration", error = %e, "[Storage] Failed to copy legacy metadata");
            }
            return Ok(final_video_path);
        }

        // 4. Create unique temporary file for atomic installation
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let tmp_video_filename = format!(".tmp.{pid}.{now}.{hash}.mp4");
        let tmp_video_path = self.videos_dir.join(&tmp_video_filename);

        let cleanup_tmp = |path: &Path| {
            if let Err(e) = fs::remove_file(path)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                tracing::debug!(operation = "cleanup", error = %e, path = ?path, "[Storage] Failed to remove temporary file");
            }
        };

        // Check compatibility (if crossfade is requested, transcoding is required)
        let is_compatible = probe_info.is_compatible_profile() && effective_crossfade.is_none();
        let transcoded = if is_compatible {
            tracing::info!(
                operation = "cache_import",
                container = probe_info.container_format.as_deref().unwrap_or("mp4"),
                codec = %probe_info.codec,
                width = probe_info.width,
                height = probe_info.height,
                fps = probe_info.fps,
                "[Storage] Video is already compatible profile and no crossfade requested; attempting hardlink, falling back to copy"
            );
            // Optimization 3-1: Try instant zero-copy hardlink first if on the same filesystem
            if let Err(hardlink_err) = fs::hard_link(&canonical_source, &tmp_video_path) {
                tracing::debug!(
                    operation = "hardlink_fallback",
                    error = %hardlink_err,
                    "[Storage] Hardlink unavailable; falling back to full file copy"
                );
                fs::copy(&canonical_source, &tmp_video_path).map_err(|e| {
                    cleanup_tmp(&tmp_video_path);
                    FluffyError::Cache(format!("Failed to copy compatible video to storage: {e}"))
                })?;
            } else {
                tracing::info!(
                    operation = "cache_import_hardlink",
                    "[Storage] Created instant zero-copy hardlink in storage"
                );
            }
            false
        } else {
            tracing::info!(
                operation = "cache_import",
                container = ?probe_info.container_format,
                codec = %probe_info.codec,
                width = probe_info.width,
                height = probe_info.height,
                fps = probe_info.fps,
                crossfade_ms = ?effective_crossfade,
                "[Storage] Video requires transcoding (normalization or loop crossfade); transcoding to temporary storage file"
            );
            // Optimization 3-2: Support cancellation during ffmpeg transcoding
            let fade_sec = effective_crossfade.map(|ms| ms as f64 / 1000.0);
            if let Err(e) = transcode_video_with_crossfade(
                &canonical_source,
                &tmp_video_path,
                &probe_info,
                cancel_token,
                fade_sec,
            ) {
                cleanup_tmp(&tmp_video_path);
                return Err(e);
            }
            true
        };

        // 5. Atomic rename to destination
        if let Err(e) = fs::rename(&tmp_video_path, &final_video_path) {
            cleanup_tmp(&tmp_video_path);
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
                fps: if is_compatible {
                    probe_info.fps.round() as u32
                } else {
                    DEFAULT_FPS
                },
                audio: false,
                width: target_w,
                height: target_h,
            },
            output: video_filename,
            transcoded,
            crossfade_ms: effective_crossfade,
        };

        let tmp_meta_filename = format!(".tmp.{pid}.{now}.{hash}.json");
        let tmp_meta_path = self.metadata_dir.join(&tmp_meta_filename);
        let meta_json = serde_json::to_string_pretty(&metadata)?;
        if let Err(e) = fs::write(&tmp_meta_path, meta_json) {
            cleanup_tmp(&final_video_path);
            return Err(FluffyError::Cache(format!("Failed to write metadata: {e}")));
        }
        if let Err(e) = fs::rename(&tmp_meta_path, &final_metadata_path) {
            cleanup_tmp(&tmp_meta_path);
            cleanup_tmp(&final_video_path);
            return Err(FluffyError::Cache(format!(
                "Failed to atomically install metadata: {e}"
            )));
        }

        tracing::info!(
            operation = "cache_install",
            destination = ?final_video_path,
            transcoded,
            "[Storage] Successfully installed video and metadata to storage"
        );

        Ok(final_video_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

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
            crossfade_ms: None,
        };

        let serialized = serde_json::to_string(&meta).unwrap();
        let deserialized: CacheMetadata = serde_json::from_str(&serialized).unwrap();
        assert_eq!(meta, deserialized);
    }

    #[test]
    fn test_cache_import_atomic_and_reuse() {
        let temp_dir =
            std::env::temp_dir().join(format!("fluffy_cache_test_{}", std::process::id()));
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
        let temp_dir =
            std::env::temp_dir().join(format!("fluffy_bypass_test_{}", std::process::id()));
        let manager = CacheManager::new(&temp_dir).expect("Failed to create cache manager");

        let source = Path::new("test.mp4");
        if !source.exists() {
            return;
        }

        // test.mp4 is 1920x1080 30fps H.264 yuv420p -> compatible!
        let stored = manager
            .import_video(source)
            .expect("Import compatible video failed");
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

    #[test]
    fn test_metadata_rename_failure_rolls_back_video() {
        let temp_dir =
            std::env::temp_dir().join(format!("fluffy_meta_fail_test_{}", std::process::id()));
        let manager = CacheManager::new(&temp_dir).expect("Failed to create cache manager");

        let source = Path::new("test.mp4");
        if !source.exists() {
            return;
        }

        let hash = CacheManager::compute_source_hash(source).unwrap();
        let final_meta_path = manager.metadata_dir().join(format!("{hash}.json"));
        // Create a directory where the metadata file should be.
        // In Linux/Unix, fs::rename of a regular file onto a directory path fails with EISDIR.
        fs::create_dir_all(&final_meta_path).unwrap();

        // import_video must fail due to metadata rename error
        let res = manager.import_video(source);
        assert!(
            res.is_err(),
            "Expected import_video to fail when metadata rename fails"
        );

        // Verify that the video file was cleaned up (rolled back) and not left in storage
        let final_video_path = manager.objects_dir().join(format!("{hash}.mp4"));
        assert!(
            !final_video_path.exists(),
            "Final video file must be rolled back if metadata installation fails"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cleanup_stale_temp_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("fluffy_clean_tmp_test_{}", std::process::id()));
        let manager = CacheManager::new(&temp_dir).expect("Failed to create cache manager");

        let stale_vid = manager.videos_dir().join(".tmp.9999.12345.dummy.mp4");
        let stale_meta = manager.metadata_dir().join(".tmp.9999.12345.dummy.json");
        let permanent_vid = manager.videos_dir().join("permanent.mp4");

        fs::write(&stale_vid, b"stale video").unwrap();
        fs::write(&stale_meta, b"stale meta").unwrap();
        fs::write(&permanent_vid, b"keep this").unwrap();

        assert!(stale_vid.exists());
        assert!(stale_meta.exists());
        assert!(permanent_vid.exists());

        let cleaned = manager.cleanup_stale_temp_files().unwrap();
        assert_eq!(cleaned, 2);

        assert!(!stale_vid.exists());
        assert!(!stale_meta.exists());
        assert!(permanent_vid.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cache_already_exists_bypasses_work() {
        let temp_dir =
            std::env::temp_dir().join(format!("fluffy_cache_hit_test_{}", std::process::id()));
        let manager = CacheManager::new(&temp_dir).expect("Failed to create cache manager");

        let source = Path::new("test.mp4");
        if !source.exists() {
            return;
        }

        // 1. Initial import
        let p1 = manager.import_video(source).unwrap();
        assert!(p1.exists());

        let hash = p1.file_stem().unwrap().to_str().unwrap();
        let meta_path = manager.metadata_dir().join(format!("{hash}.json"));
        assert!(meta_path.exists());

        // 2. Second import for identical content -> must immediately return existing path
        let p2 = manager.import_video(source).unwrap();
        assert_eq!(p1, p2);

        // 3. Cache integrity check: if metadata is missing, it must NOT be considered a cache hit
        fs::remove_file(&meta_path).unwrap();
        assert!(!meta_path.exists());
        // Now re-import should recognize incomplete cache and recreate metadata
        let p3 = manager.import_video(source).unwrap();
        assert_eq!(p1, p3);
        assert!(meta_path.exists(), "Metadata must be restored");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cache_import_with_crossfade() {
        let temp_dir =
            std::env::temp_dir().join(format!("fluffy_cache_xfade_test_{}", std::process::id()));
        let manager = CacheManager::new(&temp_dir).expect("Failed to create cache manager");

        let source = temp_dir.join("source.mp4");
        let gen_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=4:size=320x240:rate=30",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&source)
            .status();

        let Ok(status) = gen_status else {
            return;
        };
        if !status.success() {
            let _ = fs::remove_dir_all(&temp_dir);
            return;
        }

        // 1. Import with 500ms crossfade
        let p1 = manager.import_video_with_crossfade(&source, 500).unwrap();
        assert!(p1.exists());
        let file_name = p1.file_name().unwrap().to_str().unwrap();
        assert!(file_name.contains("_xfade_500.mp4"));

        let meta_filename = file_name.replace(".mp4", ".json");
        let meta_path = manager.metadata_dir().join(&meta_filename);
        assert!(meta_path.exists());

        let meta_content = fs::read_to_string(&meta_path).unwrap();
        let meta: CacheMetadata = serde_json::from_str(&meta_content).unwrap();
        assert_eq!(meta.crossfade_ms, Some(500));
        assert!(meta.transcoded);

        // 2. Cache hit on second call
        let p2 = manager.import_video_with_crossfade(&source, 500).unwrap();
        assert_eq!(p1, p2);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
