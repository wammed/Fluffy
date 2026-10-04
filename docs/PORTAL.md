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
| **[Root README](../README.md)** | Quick start, highlights, basic usage summary | First-time users and operators installing Fluffy |
| **[CLI Reference](CLI.md)** | CLI syntax, command options, practical commands | Shell users, script authors, power users |
| **[Configuration Specifications](CONFIGURATION.md)** | XDG config keys (`config.json`), state file (`state.json`) | Administrators and power users configuring options |
| **[Video Formats & Storage](STORAGE_AND_FORMATS.md)** | Compliant specs, auto-transcoding, SHA-256 deduplicated storage | Content creators, operators managing video files |
| **[Desktop & systemd Guide](SYSTEMD.md)** | Automated desktop integration, systemd user service management | Desktop integrators and service administrators |
| **[Performance Benchmark Report](BENCHMARK_REPORT.md)** | Real-hardware CPU, RSS RAM, GPU 3D, and NVDEC decoder metrics | Performance engineers and desktop integrators |
| **[Technical Design Document](TECHNICAL_DESIGN.md)** | System architecture, IPC JSON-RPC protocol, cache rules, layer-shell integration | Developers, architects, and technical contributors |
| **[Session Handover & Progress](SESSION_HANDOVER.md)** | Engineering roadmap, phase completion status, hardware verification matrix | Developers continuing project implementation |
| **[Icon Design & IP Compliance](../legal/IP_COMPLIANCE.md)** | Icon provenance, design audit, and trademark / IP clearance verification | Package maintainers, desktop integrators, and contributors |
| **[Third-Party Licenses](../legal/THIRD_PARTY_LICENSES.md)** | Rust dependencies, GStreamer & FFmpeg runtime licensing, downstream packaging obligations | Downstream packagers, legal auditors, and distributors |
| **[Icon Design & IP Review History](../legal/ICON_DESIGN_HISTORY.md)** | Multi-app icon genesis, AI prompt history, and iterative audit logs | Maintainers and archivists |

---

## 🎯 Task-Oriented Navigation

### 1. Installation & Desktop Integration
- **Quick Installation**: Follow the [Quick Start](../README.md#-quick-start) in the root README.
- **Desktop & systemd Integration**: Read the [Desktop & systemd Setup Guide](SYSTEMD.md) for service control and auto-installer details.
- **Application Icon & Branding**: Scalable SVG icons are located in `images/fluffy-icon.svg` and `data/icons/`. Review [Icon Design & IP Compliance](../legal/IP_COMPLIANCE.md) for provenance details.

### 2. Playback Control & Configuration
- **CLI Commands**: Read the [CLI Reference](CLI.md) for full syntax, flags, and workflow examples.
- **GUI Control**: Launch `fluffy-settings` to visually select monitors, browse videos, and apply live wallpapers.
- **Configuration & States**: Read [Configuration Specifications](CONFIGURATION.md) for details on `~/.config/fluffy/config.json` and state files.
- **Video Specifications**: Review [Video Formats & Storage](STORAGE_AND_FORMATS.md) for details on compliant profiles (H.264/yuv420p/30fps) and non-blocking background normalization.

### 3. Understanding the Architecture & Protocols
- **Non-Destructive Overlay Model**: Read [TECHNICAL_DESIGN.md: Section 3 & 4](TECHNICAL_DESIGN.md#3-core-architecture) to understand how Fluffy renders on `Layer::Bottom` without interfering with `cosmic-bg`.
- **Zero-Black-Screen Pre-Roll & Async Normalization**: Check [TECHNICAL_DESIGN.md: Section 8 & 11](TECHNICAL_DESIGN.md) for details on the dual-pipeline seamless switching model and non-blocking worker threads.
- **IPC Protocol**: Review [TECHNICAL_DESIGN.md: Section 9](TECHNICAL_DESIGN.md#9-ipc-specification) for the JSON Lines Unix domain socket protocol specification.

### 4. Reviewing Performance & Hardware Evidence
- **Benchmark Evidence**: Consult [BENCHMARK_REPORT.md](BENCHMARK_REPORT.md) for actual metrics recorded on an NVIDIA RTX 3080 with dual 1440p displays under 1080p, 1440p, and 4K loads.
- **Verification Levels**: Check [SESSION_HANDOVER.md: Section 2](SESSION_HANDOVER.md#2-current-state--verification-level) to see hardware test proof across Phases 1 through 7.

### 5. Legal, Licensing & IP Provenance
- **Project License**: Fluffy is licensed under the MIT License ([LICENSE](../LICENSE)).
- **Third-Party & Runtime Licenses**: Fluffy does not bundle GStreamer or FFmpeg. Review [Third-Party Licenses](../legal/THIRD_PARTY_LICENSES.md) for detailed runtime boundaries and `cargo-deny` audit records.
- **Icon Design Due Diligence**: See [Icon Design & IP Compliance](../legal/IP_COMPLIANCE.md) and [Icon Design History](../legal/ICON_DESIGN_HISTORY.md) for design provenance.

---

<p align="center">
  <a href="../README.md">← Back to Root README</a> | <a href="PORTAL.ja.md">日本語ポータルへ →</a>
</p>
