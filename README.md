<div align="center">

# 🎬 Fluffy
### Lightweight, Non-Destructive Video Wallpaper Manager for COSMIC Desktop / Wayland

[![Built with libcosmic](https://img.shields.io/badge/libcosmic-Pop!_OS_COSMIC-24C8D8?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/pop-os/libcosmic)
[![Rust](https://img.shields.io/badge/Rust-1.80+-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Wayland](https://img.shields.io/badge/Wayland-Native-5277C3?style=for-the-badge&logo=wayland&logoColor=white)](https://wayland.freedesktop.org/)
[![GStreamer](https://img.shields.io/badge/GStreamer-1.24+-E95420?style=for-the-badge&logo=gstreamer&logoColor=white)](https://gstreamer.freedesktop.org/)
[![Platform](https://img.shields.io/badge/Platform-Linux_(COSMIC_/_Wayland)-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://www.kernel.org/)
[![Vibe Coding](https://img.shields.io/badge/Built_with-AI_Vibe_Coding-8A2BE2?style=for-the-badge&logo=sparkles&logoColor=white)](#-about-this-project-ai-vibe-coding)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)

<p align="center">
  <strong>Wayland Native Layer-Shell × Zero-Flicker Pre-Roll × Hardware-Accelerated NVDEC × SHA-256 Cache × Independent Multi-Monitor</strong><br>
  A high-performance, non-destructive live video wallpaper manager tailored for COSMIC Desktop and Wayland compositors.
</p>

<p align="center">
  <a href="README.md">English</a> | <a href="README.ja.md">日本語</a> | <a href="docs/PORTAL.md">📚 Documentation Portal</a> | <a href="docs/BENCHMARK_REPORT.md">📊 Performance Benchmarks</a>
</p>

</div>

---

## 🌟 Highlights

- **Non-Destructive Coexistence (`Layer::Bottom`)**: Fluffy never modifies or kills `cosmic-bg`. It paints as an overlay on `Layer::Bottom`, below docks, panels, and windows. If Fluffy stops, your original COSMIC desktop background instantly reappears without desktop disruption.
- **Zero-Flicker Seamless Switching**: Features dual-pipeline pre-roll architecture. Transitions between loop videos render seamlessly with zero black frames, flashing, or compositor resizes.
- **Hardware-Accelerated Decoding**: Automatically offloads video playback to GPU ASICs (e.g. NVIDIA NVDEC via `nvh264dec` / VA-API), ensuring virtually 0% CPU consumption during idle state and minimal power draw.
- **Architectural Isolation (Daemon vs. GUI)**:
  - **Resident Daemon (`fluffy`)**: Micro-footprint of only **3.2 MB** binary size and **40 MB RSS** RAM at idle.
  - **Settings GUI (`fluffy-settings`)**: Ephemeral client built with `libcosmic` that connects over Unix domain socket IPC and exits immediately when done, leaving the daemon completely unbloated.
- **Deterministic 4K Normalization & Atomic Caching**: Validates arbitrary video files via `ffprobe` (strictly enforcing 4K boundaries) and transcodes to standard H.264/30fps profiles with SHA-256 content addressing in `~/.cache/fluffy/`.
- **Dynamic Display Hotplug**: Automatically detects monitor connections and disconnections in real-time, assigning wallpapers without daemon restarts.
- **systemd `--user` Integration**: Fully integrated into `graphical-session.target` with automated crash recovery (`Restart=on-failure`).

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
2. Installs binaries into `~/.cargo/bin/` (or `~/.local/bin/`).
3. Installs `fluffy.service` into `~/.config/systemd/user/`.
4. Installs the desktop launcher into `~/.local/share/applications/com.github.fluffy.Fluffy.desktop`.

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
    help, --help         Print help information

OPTIONS:
    --socket <PATH>      Target daemon Unix socket path (default: $XDG_RUNTIME_DIR/fluffy.sock)
    --output <NAME>      Target specific display output (e.g. DP-1, DP-2; default: all displays)
    --generation <NUM>   Monotonic generation number for race condition prevention
    --video <PATH>       (daemon only) Start playback immediately with specified video
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
```

---

## 🏛️ Architecture Overview

```text
User / Autostart / Settings GUI / CLI
       │
       ▼ (Unix Domain Socket IPC: $XDG_RUNTIME_DIR/fluffy.sock)
┌─────────────────────────────────────────────────────────────────┐
│ Wallpaper Daemon (Resident Process: ~3.2 MB binary, ~40 MB RSS) │
│                                                                 │
│  ├─ Cache & Normalization Subsystem (~/.cache/fluffy/)          │
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

---

## 🤖 About This Project (AI Vibe Coding)

This project was built from ground-up using advanced **AI Vibe Coding** paired with rigorous real-hardware verification on Linux Wayland. Every milestone adheres to strict verification levels:
`Implemented` ➔ `Compiled` ➔ `Unit-tested` ➔ `Integration-tested` ➔ `Real-hardware-tested`.

---

## 📄 License

Licensed under the [MIT License](LICENSE).
