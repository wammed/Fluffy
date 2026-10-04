# 🎬 Fluffy Video Format Specifications & Storage Architecture

<p align="center">
  <a href="STORAGE_AND_FORMATS.md">English</a> | <a href="STORAGE_AND_FORMATS.ja.md">日本語</a> | <a href="PORTAL.md">📚 Documentation Portal</a>
</p>

Fluffy enforces strict video format validation paired with content-addressed atomic storage to guarantee low-overhead playback, zero black-frame transitions, and system stability.

---

## 1. Zero-CPU Compliant Format (Immediate Playback without Transcoding)

Videos matching all criteria below bypass transcoding entirely. They are registered into persistent storage and played instantly with zero CPU load spikes or fan noise:

| Property | Compliant Standard | Notes |
| :--- | :--- | :--- |
| **Container Format** | MP4 (`.mp4`, format name `mp4` or `mov`) | Genuine MP4 container (MKV/WebM files with H.264 video streams are normalized to MP4) |
| **Video Codec** | H.264 / AVC (`h264`, `avc1`) | Best compatibility for hardware decoders across Wayland compositors |
| **Pixel Format** | `yuv420p` | Widely compatible 8-bit YUV format |
| **Resolution** | Even width × Even height, up to 4K (3840×2160) | Odd dimensions can cause stride mismatch issues in Wayland/GStreamer sinks |
| **Framerate** | 30 fps or lower (23.976, 24, 25, 29.97, 30 fps) | Guarantees minimal heat and power consumption during continuous desktop playback |
| **Audio Track** | Optional (automatically muted) | Audio playback is muted for wallpaper use |

---

## 2. Automatic Background Normalization for Non-Compliant Formats

Fluffy supports non-compliant formats (e.g. HEVC/H.265, AV1, VP9, >30fps framerates, odd dimensions, MKV, WebM) via automated transcoding:

- **Initial Run (Asynchronous Normalization)**:
  On first use, a background worker thread managed by `JobManager` transcodes the video to standard H.264/yuv420p/30fps.
  - **CPU Utilization**: CPU load increases temporarily while transcoding occurs.
  - **Zero Black Screen Guarantee**: The daemon's main thread and active playback pipeline remain unblocked. **The current wallpaper (previous video or desktop background) continues playing seamlessly without black screens or desktop freezing.**
  - **Concurrency & Deduplication**: Requests allocate monotonic generation numbers to ignore stale inputs. Identical file hashes are deduplicated in-flight.
- **Subsequent Playback**:
  Once normalized, the video is saved in persistent storage. Future switches reuse the stored file immediately without re-transcoding.

---

## 3. Persistent Storage Directory (`~/.local/share/fluffy/storage`)

Unlike transient caches that may be purged by system cleaners, Fluffy stores normalized videos in a dedicated persistent directory:

- **Videos Directory**: `$XDG_DATA_HOME/fluffy/storage/videos/` (default: `~/.local/share/fluffy/storage/videos/<hash>.mp4`)
- **Metadata Directory**: `$XDG_DATA_HOME/fluffy/storage/metadata/` (default: `~/.local/share/fluffy/storage/metadata/<hash>.json`)
- **Content Deduplication**: Files are addressed by SHA-256 hashes, preventing duplicate video storage.

---

## 4. Transcoding Progress Indicators

- **Settings GUI (`fluffy-settings`)**: An animated spinner (`⚙️ ↑`) and status panel display "Optimizing / Transcoding video..." in real time.
- **CLI (`fluffy status`)**: Displays active background tasks: `Background Tasks: ⚙️ N active conversion job(s)`.
