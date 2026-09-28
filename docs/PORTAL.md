# 📚 Fluffy Documentation Portal

Welcome to the official documentation portal for **Fluffy**.  
Fluffy is a lightweight, non-destructive, hardware-accelerated video wallpaper manager built natively for Pop!_OS COSMIC Desktop and Linux Wayland compositors.

Use this portal to navigate specifications, architecture design records, benchmark evidence, and operational guides.

<p align="center">
  <a href="PORTAL.md">English</a> | <a href="PORTAL.ja.md">日本語</a>
</p>

---

## 🧭 Master Documentation Index

| Document | Primary Topics | Recommended Audience |
| :--- | :--- | :--- |
| **[Root README](../README.md)** | Quick start, installation, feature highlights, basic CLI & GUI usage | First-time users and operators installing Fluffy |
| **[Technical Design Document](TECHNICAL_DESIGN.md)** | System architecture, IPC JSON-RPC protocol, cache rules, layer-shell integration | Developers, architects, and technical contributors |
| **[Session Handover & Progress](SESSION_HANDOVER.md)** | Engineering roadmap, phase completion status, hardware verification matrix | Developers continuing project implementation |
| **[Performance Benchmark Report](BENCHMARK_REPORT.md)** | Real-hardware CPU, RSS RAM, GPU 3D, and NVDEC decoder metrics across resolutions | Performance engineers and desktop integrators |
| **[Icon Design & IP Compliance](../IP_COMPLIANCE.md)** | Icon provenance, design audit, and trademark / IP clearance verification | Package maintainers, desktop integrators, and contributors |

---

## 🎯 Task-Oriented Navigation

### 1. Installation & Desktop Integration
- **Quick Installation**: Follow the [Quick Start](../README.md#-quick-start) in the root README.
- **Application Icon & Desktop Branding**: Scalable SVG icons are located in `images/fluffy-icon.svg` and `data/icons/hicolor/scalable/apps/`. Review [Icon Design & IP Compliance](../IP_COMPLIANCE.md) for provenance details.
- **systemd `--user` Service**: Run `scripts/install-desktop-integration.sh` to install `fluffy.service`, desktop entry, and application icons.
- **Managing via systemctl**: Learn service lifecycle management in [Root README: Desktop Setup](../README.md#%EF%B8%8F-desktop--systemd-user-service-setup).

### 2. Controlling Playback via CLI & GUI
- **CLI Commands**: See [Root README: CLI Usage](../README.md#-cli-usage) for `fluffy set-video`, `fluffy pause`, `fluffy resume`, `fluffy stop`, and `fluffy status`.
- **GUI Control**: Launch `fluffy-settings` to visually select target monitors, browse videos, and apply live wallpapers. An animated indicator displays during normalization.
- **Video Specifications**: Review [Root README: Video Specifications](../README.md#-video-specifications--persistent-storage) for details on zero-CPU compliant profiles (H.264/yuv420p/30fps) and non-blocking background normalization.
- **Persistent Storage (`~/.local/share/fluffy/storage`)**: Wallpapers are kept in persistent storage until explicitly deleted.

### 3. Understanding the Architecture & Protocols
- **Non-Destructive Overlay Model**: Read [TECHNICAL_DESIGN.md: Section 3 & 4](TECHNICAL_DESIGN.md#3-core-architecture) to understand how Fluffy renders on `Layer::Bottom` without interfering with `cosmic-bg`.
- **Zero-Black-Screen Pre-Roll & Async Normalization**: Check [TECHNICAL_DESIGN.md: Section 8 & 11](TECHNICAL_DESIGN.md) for details on the dual-pipeline seamless switching model and non-blocking worker threads.
- **IPC Protocol**: Review [TECHNICAL_DESIGN.md: Section 9](TECHNICAL_DESIGN.md#9-ipc-specification) for the JSON Lines Unix domain socket protocol specification.

### 4. Reviewing Performance & Hardware Evidence
- **Benchmark Evidence**: Consult [BENCHMARK_REPORT.md](BENCHMARK_REPORT.md) for actual metrics recorded on an NVIDIA RTX 3080 with dual 1440p displays under 1080p, 1440p, and 4K loads.
- **Verification Levels**: Check [SESSION_HANDOVER.md: Section 2](SESSION_HANDOVER.md#2-current-state--verification-level) to see hardware test proof across Phases 1 through 7.

---

<p align="center">
  <a href="../README.md">← Back to Root README</a> | <a href="PORTAL.ja.md">日本語ポータルへ →</a>
</p>
