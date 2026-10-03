<div align="center">

<img src="./images/fluffy-icon.svg" width="96" height="96" alt="Fluffy Icon" />

# 🎬 Fluffy
### Lightweight, Non-Destructive Video Wallpaper Manager for COSMIC Desktop / Wayland

![Banner](./images/fluffy-banner.png)

[![Built with libcosmic](https://img.shields.io/badge/libcosmic-Pop!_OS_COSMIC-24C8D8?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/pop-os/libcosmic)
[![Rust](https://img.shields.io/badge/Rust-1.80+-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Wayland](https://img.shields.io/badge/Wayland-Native-5277C3?style=for-the-badge&logo=wayland&logoColor=white)](https://wayland.freedesktop.org/)
[![GStreamer](https://img.shields.io/badge/GStreamer-1.28+-E95420?style=for-the-badge&logo=gstreamer&logoColor=white)](https://gstreamer.freedesktop.org/)
[![Platform](https://img.shields.io/badge/Platform-Linux_(COSMIC_/_Wayland)-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://www.kernel.org/)
[![Vibe Coding](https://img.shields.io/badge/Built_with-AI_Vibe_Coding-8A2BE2?style=for-the-badge&logo=sparkles&logoColor=white)](#-about-this-project-ai-vibe-coding)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)

<p align="center">
  <strong>Wayland Native Layer-Shell × Zero-Flicker Pre-Roll × Hardware-Accelerated NVDEC × SHA-256 Cache × Independent Multi-Monitor</strong><br>
  A high-performance, non-destructive live video wallpaper manager tailored for COSMIC Desktop and Wayland compositors.
</p>

<p align="center">
  <a href="README.md">English</a> | <a href="README.ja.md">日本語</a> | <a href="docs/PORTAL.md">📚 Documentation Portal</a> | <a href="docs/BENCHMARK_REPORT.md">📊 Performance Benchmarks</a> | <a href="legal/IP_COMPLIANCE.md">🎨 Icon & IP Compliance</a>
</p>

</div>

---

## 🌟 Highlights

- **Non-Destructive Coexistence (`Layer::Bottom`)**: Fluffy never modifies or kills `cosmic-bg`. It paints as an overlay on `Layer::Bottom`, below docks, panels, and windows. If Fluffy stops, your original COSMIC desktop background instantly reappears without desktop disruption.
- **Zero-Flicker Seamless Switching**: Features dual-pipeline pre-roll architecture. Transitions between loop videos render seamlessly with zero black frames, flashing, or compositor resizes (verified on physical testbed).
- **Hardware-Accelerated Decoding (Preferred & Auto-Detected)**: Hardware decode is preferred; the actual decoder depends on the GStreamer environment and installed drivers. Verified with NVDEC (`nvh264dec`) on the project testbed (NVIDIA GeForce RTX 3080), maintaining minimal CPU utilization and heat, with transparent software fallback (`avdec_h264`).
- **Architectural Isolation (Daemon vs. GUI)**:
  - **Resident Daemon (`fluffy`)**: Micro-footprint of only **4.7 MB** (stripped: **3.3 MB**) binary size and **40 MB RSS** RAM at idle.
  - **Settings GUI (`fluffy-settings`)**: Ephemeral client built with `libcosmic` that connects over Unix domain socket IPC and exits immediately when done, leaving the daemon completely unbloated.
- **Deterministic 4K Normalization & Atomic Storage**: Validates arbitrary video files via `ffprobe` (strictly enforcing container format and 4K boundaries) and transcodes to standard H.264/30fps profiles with SHA-256 content addressing in `~/.local/share/fluffy/storage/`.
- **Dynamic Display Hotplug & Geometry Tracking**: Automatically detects monitor connections, disconnections, and resolution/scale changes in real-time without daemon restarts, with dynamic shared memory buffer pool recalculation to prevent crashes.
- **Auto-Pause on Fullscreen (Dual Wayland Protocol)**: Automatically pauses playback during fullscreen gaming or media playback to preserve CPU/GPU resources. Dual support for COSMIC native (`zcosmic_toplevel_info_v1`) and wlroots (`zwlr_foreign_toplevel_manager_v1`), with real-time compositor compatibility reporting in the settings GUI.
- **Adaptive Sleep & In-Flight Job Cancellation**: Dynamically throttles main event loop polling based on state (5ms transcoding / 16ms playback / 50ms idle), atomically kills superseded ffmpeg transcoding subprocesses on rapid switching, and uses hardlink fast-paths for compatible videos.
- **systemd `--user` Integration**: Fully integrated into `graphical-session.target` with absolute binary pathing (`%h/.local/bin/fluffy`) and automated crash recovery (`Restart=on-failure`).

---

## 📊 Hardware-Verified Performance Benchmarks

Tested on real hardware (**CachyOS / Arch Linux**, **NVIDIA GeForce RTX 3080**, **COSMIC Desktop `cosmic-comp`**, dual 1440p displays `DP-1` + `DP-2`):

| Test Scenario | CPU Usage (%) | RSS Memory (MB) | GPU 3D Util (%) | GPU Video Decoder (NVDEC) |
| :--- | :--- | :--- | :--- | :--- |
| **Daemon Idle** (0 videos) | **0.2%** | **40.5 MB** | 33.2% | **0.0%** |
| **1080p30** (Single Display: DP-1) | **10.2%** | **312.7 MB** | 31.4% | **9.9%** |
| **1080p30** (Dual Displays: DP-1 + DP-2) | **18.0%** | **514.3 MB** | 25.2% | **10.5%** |
| **1440p30** (Dual Displays: DP-1 + DP-2) | **27.4%** | **615.5 MB** | 33.0% | **26.6%** |
| **4K30** (Dual Displays: DP-1 + DP-2) | **43.9%** | **854.1 MB** | 32.1% | **50.6%** |

*See full details in the [Performance Benchmark Report](docs/BENCHMARK_REPORT.md).*

---

## 🚀 Quick Start

### Prerequisites

Ensure you have the required GStreamer and system dependencies installed:

```bash
# Arch Linux / CachyOS / Manjaro
sudo pacman -S gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav ffmpeg
```

### Build & Run

```bash
# 1. Clone repository
git clone https://github.com/wammed/Fluffy.git
cd Fluffy

# 2. Build release binaries
cargo build --release --features gui

# 3. Start the background daemon
./target/release/fluffy daemon &

# 4. Set a wallpaper video via CLI
./target/release/fluffy set-video /path/to/wallpaper.mp4

# 5. Or launch the native COSMIC Settings GUI
./target/release/fluffy-settings
```

---

## 🖥️ Desktop & systemd User Service Setup

Fluffy provides an automated installer for desktop integration:

```bash
./scripts/install-desktop-integration.sh
```

This script:
1. Compiles optimized release binaries (`fluffy` and `fluffy-settings`).
2. Installs binaries into `~/.local/bin/`.
3. Installs `fluffy.service` into `~/.config/systemd/user/`.
4. Installs application icons into `~/.local/share/icons/hicolor/scalable/apps/`.
5. Installs the desktop launcher into `~/.local/share/applications/com.github.fluffy.Fluffy.desktop`.

### Managing via systemd:

```bash
# Enable and start daemon on user login
systemctl --user enable --now fluffy.service

# Check daemon health & logs
systemctl --user status fluffy.service
journalctl --user -u fluffy.service -f

# Stop or restart daemon
systemctl --user stop fluffy.service
systemctl --user restart fluffy.service
```

---

## 💻 CLI Usage

```text
Fluffy Video Wallpaper Manager

USAGE:
    fluffy [COMMAND] [OPTIONS]

COMMANDS:
    daemon, run          Run the wallpaper daemon (resident background process)
    status               Query daemon and display status via IPC
    set-video <PATH>     Change video wallpaper (validates & normalizes to cache)
    import <PATH>        Validate and import video into cache without playing
    pause                Pause video playback
    resume               Resume video playback
    stop                 Stop video playback (unmaps surface, restoring desktop)
    reload               Reload current wallpaper video
    config               Show or update startup and wallpaper settings
    help, --help         Print help information

OPTIONS:
    --socket <PATH>      Target daemon Unix socket path (default: $XDG_RUNTIME_DIR/fluffy.sock)
    --output <NAME>      Target specific display output (e.g. DP-1, DP-2; default: all displays)
    --generation <NUM>   Monotonic generation number for race condition prevention
    --timeout <SECS>     IPC response timeout (default: 60s for set-video, 5s for others)
    --video <PATH>       (daemon only) Start playback immediately with specified video

OPTIONS for 'config':
    --restore-on-startup <BOOL>  Restore last wallpaper on daemon startup (true/false)
    --autostart <BOOL>           Enable/disable daemon autostart on login via systemd (true/false)
    --pause-fullscreen <BOOL>    Configure pause on fullscreen windows (true/false)
```

### Examples

```bash
# Set wallpaper on all connected displays
fluffy set-video ~/Videos/cyberpunk_city.mp4

# Set wallpaper only on second monitor
fluffy set-video ~/Videos/nature.mp4 --output DP-2

# Pause wallpaper on DP-1
fluffy pause --output DP-1

# Query status of all monitors
fluffy status

# View current configuration and saved wallpaper state
fluffy config

# Enable wallpaper restoration on startup/login (opt-in)
fluffy config --restore-on-startup true

# Enable autostart of fluffy daemon via systemd
fluffy config --autostart true
```

---

## ⚙️ Configuration & State Storage

Fluffy conforms strictly to the XDG Base Directory specification:

- **Configuration File**: `$XDG_CONFIG_HOME/fluffy/config.json` (default: `~/.config/fluffy/config.json`)
  - `restore_on_startup`: Automatically restores saved wallpapers on daemon launch / user login (default: `false`, stateless mode).
  - `autostart_daemon`: Tracks login autostart state with systemd user service.
  - `pause_on_fullscreen`: Pauses playback when an application window is in fullscreen. The settings GUI (`fluffy-settings`) automatically checks and displays compositor compatibility (`🟢 コンポジター対応` / `⚠️ コンポジター非対応`) in real time.
- **State File**: `$XDG_STATE_HOME/fluffy/state.json` (default: `~/.local/state/fluffy/state.json`)
  - Records the last applied normalized video path per display output for zero-delay startup restoration.

---

## 🎬 Video Specifications & Persistent Storage

### 1. Compliant Video Profile (Instant Playback & Zero CPU Transcoding)
Video files satisfying the standard profile completely **bypass transcoding** and are instantly registered to persistent storage and played with near-zero latency and zero fan noise:

| Property | Compliant Standard | Description |
| :--- | :--- | :--- |
| **Container** | MP4 (`.mp4`, format containing `mp4` or `mov`) | Native MP4 container (MKV/WebM files, even if containing H.264 streams, are normalized to genuine MP4) |
| **Video Codec** | H.264 / AVC (`h264`, `avc1`) | Maximum decoding efficiency across all GPUs and CPUs |
| **Pixel Format** | `yuv420p` | Widely compatible 8-bit YUV |
| **Resolution** | Even width & height, up to 4K (3840×2160) | Odd pixel dimensions cause Wayland / GStreamer rendering bugs |
| **Frame Rate** | Up to 30 fps (23.976, 24, 25, 29.97, 30 fps) | Guaranteed low CPU/GPU power consumption and minimal heat |
| **Audio** | Any (automatically muted with silent sink) | Wallpaper playback is strictly non-intrusive |

### 2. Automatic Normalization for Non-Compliant Videos
Non-compliant videos (e.g. HEVC/H.265, AV1, VP9, 60fps+, odd dimensions, MKV, WebM) are fully supported via automatic background normalization:
- **First-run Background Transcode**: On initial selection, a background worker thread managed by `JobManager` normalizes the video to the standard profile (H.264/yuv420p/30fps).
  - **High CPU Notice**: Initial transcoding requires CPU rendering power, temporarily increasing CPU usage and fan speed.
  - **Zero Black Screen Guarantee**: Transcoding is completely non-blocking; the daemon's event loop and ongoing wallpaper playback keep running smoothly, meaning **no black screen occurs while waiting**.
  - **Concurrency & Race Protection**: Request generations are monotonically reserved at arrival time to prevent stale conversions from clobbering newer requests. Identical in-flight conversions are automatically deduplicated.
- **Instant Playback Thereafter**: Once normalized, the video is saved in persistent storage; subsequent playback skips transcoding and starts instantly.

### 3. Persistent Storage Directory (`~/.local/share/fluffy/storage`)
Normalized and compliant videos are stored in dedicated persistent storage rather than an ephemeral cache that could be wiped by cleanup tools:
- **Videos Directory**: `$XDG_DATA_HOME/fluffy/storage/videos/` (default: `~/.local/share/fluffy/storage/videos/<hash>.mp4`)
- **Metadata Directory**: `$XDG_DATA_HOME/fluffy/storage/metadata/` (default: `~/.local/share/fluffy/storage/metadata/<hash>.json`)
- **Deduplication**: Files are indexed by SHA-256 content hashes, avoiding duplicate storage.

### 4. Visual Conversion Indicator
- **Settings GUI (`fluffy-settings`)**: Displays an animated spinner (`⚙️ ↑`) alongside real-time status while conversion is active.
- **CLI (`fluffy status`)**: Displays the active background normalization task under `Background Task`.

---

## 🏛️ Architecture Overview


```text
User / Autostart / Settings GUI / CLI
       │
       ▼ (Unix Domain Socket IPC: $XDG_RUNTIME_DIR/fluffy.sock)
┌─────────────────────────────────────────────────────────────────┐
│ Wallpaper Daemon (Resident Process: ~4.7 MB binary, ~40 MB RSS) │
│                                                                 │
│  ├─ Storage & Normalization Subsystem (~/.local/share/fluffy/storage/) │
│  │   ├─ ffprobe validation (4K resolution boundary check)       │
│  │   ├─ ffmpeg transcoding (H.264 / yuv420p / 30fps / no audio) │
│  │   └─ SHA-256 atomic caching & deduplication                  │
│  │                                                              │
│  ├─ Wayland Layer-Shell Controller                              │
│  │   ├─ Layer::Bottom non-destructive overlay                    │
│  │   ├─ Dynamic output hotplug detection (attach/detach events) │
│  │   └─ ARGB8888 initial base buffer                            │
│  │                                                              │
│  └─ Multi-Output Manager                                        │
│      ├── ManagedOutput [DP-1] (LayerSurface + GstVideoPlayer)   │
│      └── ManagedOutput [DP-2] (LayerSurface + GstVideoPlayer)   │
│            └─ Dual-Pipeline Zero-Flicker Pre-Roll Core          │
└─────────────────────────────────────────────────────────────────┘
```

For in-depth architectural and protocol details, see [docs/TECHNICAL_DESIGN.md](docs/TECHNICAL_DESIGN.md).

---

## 📚 Documentation Portal

| Document | Description |
| :--- | :--- |
| **[Documentation Portal](docs/PORTAL.md)** | Index and guide to all Fluffy documentation |
| **[Technical Design Document](docs/TECHNICAL_DESIGN.md)** | Comprehensive architectural specification and protocol reference |
| **[Session Handover](docs/SESSION_HANDOVER.md)** | Engineering roadmap, phase progress, and hardware verification matrix |
| **[Performance Benchmark Report](docs/BENCHMARK_REPORT.md)** | Hardware resource metrics across resolutions and monitor configurations |
| **[IP Compliance](legal/IP_COMPLIANCE.md)** | Icon provenance and intellectual property due diligence record |
| **[Third-Party Licenses](legal/THIRD_PARTY_LICENSES.md)** | Dependency licensing audit, runtime obligations, and packaging notes |

---

## 🤖 About This Project (AI Vibe Coding)

This project was built from ground-up using advanced **AI Vibe Coding** paired with rigorous real-hardware verification on Linux Wayland. Every milestone adheres to strict verification levels:
`Implemented` ➔ `Compiled` ➔ `Unit-tested` ➔ `Integration-tested` ➔ `Real-hardware-tested`.

---

## 📄 License

Fluffy is licensed under the [MIT License](LICENSE).

See [LICENSE](LICENSE) for the full license text.

For third-party dependency licenses, external runtime licensing information,
and asset provenance, see:

* [Third-Party Licenses](legal/THIRD_PARTY_LICENSES.md)
* [IP Compliance](legal/IP_COMPLIANCE.md)
* [Icon Design History](legal/ICON_DESIGN_HISTORY.md)

Fluffy does not bundle or redistribute GStreamer or FFmpeg. Users and
distributions provide their own multimedia runtime packages.