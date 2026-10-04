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

## 🚀 Quick Start

### 1. Prerequisites

```bash
# Arch Linux / CachyOS / Manjaro
sudo pacman -S gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav ffmpeg
```

### 2. Build & Run

```bash
# Clone and build release binaries
git clone https://github.com/wammed/Fluffy.git
cd Fluffy
cargo build --release --features gui

# Start the background daemon
./target/release/fluffy daemon &

# (Recommended) Configure seamless loop with 1.0s crossfade
./target/release/fluffy config --loop-crossfade-ms 1000

# Set a video wallpaper
./target/release/fluffy set-video /path/to/wallpaper.mp4

# Or launch the native COSMIC Settings GUI
./target/release/fluffy-settings
```

> **💡 Desktop Integration & Autostart**: Run `./scripts/install-desktop-integration.sh` to install desktop entries, application icons, and systemd user services. See the [Desktop & systemd Guide](docs/SYSTEMD.md) for details.

---

## 🌟 Highlights

- **Non-Destructive Coexistence (`Layer::Bottom`)**: Renders on `Layer::Bottom` without killing or modifying `cosmic-bg`. Exiting Fluffy immediately restores your static wallpaper.
- **Zero-Flicker Seamless Switching & Looping**: Dual-pipeline pre-roll architecture and loop crossfading eliminate black screens, flashing, and stutter at loop boundaries.
- **Hardware-Accelerated Playback (NVDEC / VA-API)**: Prioritizes GPU video decoders to minimize CPU utilization and heat, with transparent fallback to software decoding.
- **Architectural Isolation (Daemon vs. GUI)**: Micro-footprint resident daemon (~4.7MB / ~40MB RSS) loosely coupled with an ephemeral `libcosmic` settings GUI.
- **Auto-Pause on Fullscreen**: Automatically pauses playback during fullscreen gaming or media playback to preserve GPU/CPU resources (COSMIC & wlroots protocols).
- **Dynamic Hotplug & 4K Normalized Storage**: Automatically tracks monitor attachments and resolution changes, with SHA-256 deduplicated persistent storage.

---

## 💻 CLI & Basic Usage

```bash
# Set wallpaper (all displays / specific output)
fluffy set-video ~/Videos/wallpaper.mp4
fluffy set-video ~/Videos/sub.mp4 --output DP-2

# Playback controls (pause / resume / stop)
fluffy pause
fluffy resume
fluffy stop

# Status & configuration
fluffy status
fluffy config --restore-on-startup true
```

> 📖 **Full Command List, Options & Examples**: See the comprehensive [CLI Command Reference (docs/CLI.md)](docs/CLI.md).

---

## 📊 Performance Benchmarks

Verified on physical testbed (CachyOS / NVIDIA GeForce RTX 3080 / COSMIC Desktop), maintaining minimal CPU utilization and small memory footprint (idle 40MB RSS, hardware-accelerated NVDEC decoding) even during dual 4K playback.

> 📊 **Resolution-by-Resolution Metrics & Observations**: Consult the [Performance Benchmark Report (docs/BENCHMARK_REPORT.md)](docs/BENCHMARK_REPORT.md).

---

## 📚 Documentation Index

| Document | Primary Topics |
| :--- | :--- |
| **[Documentation Portal](docs/PORTAL.md)** | Master index and task-oriented navigation |
| **[CLI Command Reference](docs/CLI.md)** | Full syntax, options, and practical command-line workflows |
| **[Configuration Specifications](docs/CONFIGURATION.md)** | XDG configuration (`config.json`) and state persistence (`state.json`) |
| **[Video Formats & Storage](docs/STORAGE_AND_FORMATS.md)** | Compliant video standards, async transcoding, SHA-256 storage |
| **[Desktop & systemd Setup](docs/SYSTEMD.md)** | Automated desktop integration and systemd user service management |
| **[Performance Benchmark Report](docs/BENCHMARK_REPORT.md)** | Verified metrics across resolutions on dual-monitor testbed |
| **[Technical Design Document](docs/TECHNICAL_DESIGN.md)** | System architecture, IPC protocol specification, Layer-shell integration |
| **[Icon & IP Compliance](legal/IP_COMPLIANCE.md)** | Design audit, trademark provenance, and license clearance records |

---

## 📄 License

Fluffy is licensed under the [MIT License](LICENSE).  
For external runtime boundaries and third-party crate licensing, see [Third-Party Licenses](legal/THIRD_PARTY_LICENSES.md).