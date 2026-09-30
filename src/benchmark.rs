use std::{
    path::Path,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;

#[repr(C)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: *const std::ffi::c_char,
}

unsafe extern "C" {
    fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
}

/// Formats timestamps in ISO-8601 with millisecond precision and timezone offset:
/// e.g. `2026-09-30T22:10:01.123+09:00`
#[derive(Clone, Copy, Debug, Default)]
pub struct Iso8601LocalTime;

impl FormatTime for Iso8601LocalTime {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = duration.as_secs() as i64;
        let millis = duration.subsec_millis();

        let mut tm = std::mem::MaybeUninit::<Tm>::zeroed();
        unsafe {
            localtime_r(&secs, tm.as_mut_ptr());
        }
        let tm = unsafe { tm.assume_init() };

        let year = tm.tm_year + 1900;
        let month = tm.tm_mon + 1;
        let day = tm.tm_mday;
        let hour = tm.tm_hour;
        let min = tm.tm_min;
        let sec = tm.tm_sec;

        let offset_sec = tm.tm_gmtoff;
        let (sign, offset_hour, offset_min) = if offset_sec >= 0 {
            ('+', offset_sec / 3600, (offset_sec % 3600) / 60)
        } else {
            ('-', (-offset_sec) / 3600, ((-offset_sec) % 3600) / 60)
        };

        write!(
            w,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}{}{:02}:{:02}",
            year, month, day, hour, min, sec, millis, sign, offset_hour, offset_min
        )
    }
}

static SESSION_ID: OnceLock<String> = OnceLock::new();

/// Generates or returns the benchmark session ID for the current daemon lifetime.
/// Format: `YYYYMMDDTHHMMSS-xxxx` (e.g. `20260930T221001-7f3a`)
pub fn session_id() -> &'static str {
    SESSION_ID.get_or_init(|| {
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = duration.as_secs() as i64;
        let mut tm = std::mem::MaybeUninit::<Tm>::zeroed();
        unsafe {
            localtime_r(&secs, tm.as_mut_ptr());
        }
        let tm = unsafe { tm.assume_init() };
        let year = tm.tm_year + 1900;
        let month = tm.tm_mon + 1;
        let day = tm.tm_mday;
        let hour = tm.tm_hour;
        let min = tm.tm_min;
        let sec = tm.tm_sec;

        let pid = std::process::id();
        let suffix = ((pid as u64 ^ duration.subsec_nanos() as u64) & 0xffff) as u16;

        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}-{:04x}",
            year, month, day, hour, min, sec, suffix
        )
    })
}

/// Sets explicit session ID (useful for testing).
#[cfg(test)]
pub fn set_test_session_id(id: &str) {
    let _ = SESSION_ID.set(id.to_string());
}

/// Computes a safe, short video identifier from a Path without leaking sensitive filesystem paths.
/// If the path stem is already a 64-character SHA-256 hash (from persistent cache),
/// uses the first 12 characters of the hash.
/// Otherwise, hashes the path string and takes the first 12 characters.
pub fn safe_video_id(path: &Path) -> String {
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        && stem.len() == 64
        && stem.chars().all(|c| c.is_ascii_hexdigit())
    {
        return stem[..12].to_string();
    }

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_format() {
        let sid = session_id();
        assert!(sid.contains('T'));
        assert!(sid.contains('-'));
        assert_eq!(sid.len(), 20); // YYYYMMDDTHHMMSS-xxxx
    }

    #[test]
    fn test_safe_video_id_cache_hash() {
        let cached = Path::new("/var/cache/fluffy/videos/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.mp4");
        let vid = safe_video_id(cached);
        assert_eq!(vid, "0123456789ab");
    }

    #[test]
    fn test_safe_video_id_arbitrary_path() {
        let p = Path::new("/home/secret/user/personal_video.mp4");
        let vid = safe_video_id(p);
        assert_eq!(vid.len(), 12);
        assert!(!vid.contains("secret"));
    }

    #[test]
    fn test_iso8601_localtime_formatting() {
        let timer = Iso8601LocalTime;
        let mut buf = String::new();
        let mut writer = Writer::new(&mut buf);
        timer.format_time(&mut writer).unwrap();

        // Must match YYYY-MM-DDTHH:MM:SS.mmm+HH:MM
        assert_eq!(buf.len(), 29);
        assert_eq!(&buf[10..11], "T");
        assert_eq!(&buf[19..20], ".");
        assert!(buf.contains('+') || buf.contains('-'));
    }

    #[test]
    fn test_generation_tracker_stale_rejection_logic() {
        use crate::daemon::output_manager::GenerationTracker;

        let mut tracker = GenerationTracker::new();
        tracker.register_output("DP-1");

        // Apply generation 10
        assert!(tracker.validate_and_apply(Some("DP-1"), 10).is_ok());
        assert_eq!(tracker.current_generation("DP-1"), Some(10));

        // Applying generation 9 must be rejected as stale
        let err = tracker.validate_and_apply(Some("DP-1"), 9);
        assert!(err.is_err());
        let err_msg = err.unwrap_err().to_string();
        assert!(err_msg.contains("Stale request generation"));

        // Applying generation 11 must succeed
        assert!(tracker.validate_and_apply(Some("DP-1"), 11).is_ok());
        assert_eq!(tracker.current_generation("DP-1"), Some(11));
    }
}


