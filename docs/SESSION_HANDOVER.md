# COSMIC Video Wallpaper Manager --- Session Handover

<p align="center">
  <a href="SESSION_HANDOVER.md">English</a> | <a href="SESSION_HANDOVER.ja.md">日本語</a> | <a href="PORTAL.md">📚 Portal</a>
</p>

**Status:** Phase 8 (Refactoring, Optimizations & Fullscreen Auto-Pause) COMPLETED (Real-hardware-tested) --- All Core Phases COMPLETE.\
**Last updated:** 2026-10-04\
**Current phase summary:**
- **Phase 1: Wayland / GStreamer PoC** --- **COMPLETE** (Real-hardware-tested)
- **Phase 2: Playback Core** --- **COMPLETE** (Real-hardware-tested)
- **Phase 3: IPC & Daemon** --- **COMPLETE** (Real-hardware-tested)
- **Phase 4: Cache / Import / Normalize** --- **COMPLETE** (Real-hardware-tested)
- **Phase 5: Multi-output Management** --- **COMPLETE** (Real-hardware-tested)
- **Phase 6: Settings GUI (libcosmic)** --- **COMPLETE** (Real-hardware-tested)
- **Phase 7: Hardening, systemd & Benchmarks** --- **COMPLETE** (Real-hardware-tested)
- **Phase 8: Optimizations, Safety & Fullscreen Auto-Pause** --- **COMPLETE** (Real-hardware-tested)

------------------------------------------------------------------------

## 1. Project Summary

A lightweight loop-video wallpaper manager for COSMIC Desktop / Wayland.

The system is intentionally split into:

``` text
User / autostart / CLI
      |
      v
Wallpaper daemon (resident, minimal CPU/RAM overhead)
      |
      +-- Wayland layer-shell (Layer::Bottom non-destructive overlay)
      |
      +-- GStreamer playback core (dual-pipeline seamless switching, EOS seek loop)
      |
      +-- Unix domain socket IPC ($XDG_RUNTIME_DIR/fluffy.sock)
      |
      +-- Storage & Import Subsystem (~/.local/share/fluffy/storage/)
      |     +-- ffprobe 4K validation
      |     +-- ffmpeg H.264/yuv420p/30fps normalization
      |     +-- SHA-256 atomic storage & instant reuse
      |
      +-- Multi-output Manager
            ├── ManagedOutput [DP-1] (LayerSurface + GstVideoPlayer + Gen)
            └── ManagedOutput [DP-2] (LayerSurface + GstVideoPlayer + Gen)
```

Settings are handled by a separate `libcosmic` GUI that connects via IPC and exits immediately when configuration is done. The daemon continues playback independently.

------------------------------------------------------------------------

## 2. Current State & Verification Level

Per project documentation standards, items are tracked by five clear verification levels:
`Implemented` -> `Compiled` -> `Unit-tested` -> `Integration-tested` -> `Real-hardware-tested`.

### 2.1 Verified on Real Hardware (`Real-hardware-tested`)

**Test Environment:**
- OS: CachyOS / Arch Linux
- Compositor: COSMIC Desktop (`cosmic-comp`, `wayland-1`, `XDG_CURRENT_DESKTOP=COSMIC`)
- GPU: NVIDIA GeForce RTX 3080 (Driver 615.71.09)
- Outputs detected: `DP-1` (2560x1440), `DP-2` (2560x1440)
- Stack: Rust 1.98.1 (Edition 2024), GStreamer 1.28.7, `waylandsink`, `nvh264dec`, `ffmpeg` n9.0.2, `ffprobe`

#### Phase 1: Wayland / GStreamer PoC [COMPLETE]
- [x] **Wayland display handle passing**: Verified `GstWaylandDisplayHandleContext` (`gst_wl_display_handle_context_new`) links application Wayland connection to GStreamer sink.
- [x] **Layer surface mapping**: Verified `zwlr_layer_surface_v1` on `Layer::Bottom` covering target output.
- [x] **Subsurface visibility**: Verified parent layer surface must attach an initial base buffer (`Argb8888`) so that GStreamer subsurface becomes visible to compositor.
- [x] **Explicit render rectangle**: Verified `overlay.set_render_rectangle(0, 0, w, h)` is required for external surface binding.
- [x] **Hardware decoding**: Hardware decoding is preferred and auto-detected depending on the GStreamer environment; verified with automatic `nvh264dec` hardware acceleration on test rig (NVIDIA GeForce RTX 3080). Software decoding (`avdec_h264`) serves as documented fallback.

#### Phase 2: Playback Core [COMPLETE]
- [x] **Codebase modularization**: Clean architecture under `src/error.rs`, `src/wayland/`, `src/playback/`.
- [x] **VideoPlayer trait & GstVideoPlayer**:
  - `play(video)`: Instant playback start.
  - `pause()`: Video frames freeze smoothly.
  - `resume()`: Smooth resume without frame drops.
  - `stop()`: Clean pipeline transition to `NULL`.
- [x] **Zero-flicker seamless video switching**: Dual-pipeline pre-roll architecture verified. Transition from video A (`test.mp4`) to video B (`test2.mp4`) renders completely flicker-free without blank frames or black flashes.
- [x] **Seamless loop playback**: GStreamer `about-to-finish` gapless pre-roll transition and FFmpeg `xfade` head/tail crossfade normalization (`loop_crossfade_ms`). Maintains safe EOS seek fallback without recreating pipeline or surface.

#### Phase 3: IPC & Daemon Architecture [COMPLETE]
- [x] **Unix domain socket IPC**:
  - Bound at `$XDG_RUNTIME_DIR/fluffy.sock` (or `/run/user/1000/fluffy.sock`).
  - Strict 0600 file permissions (owner only).
  - Stale socket detection: Automatically clears dead socket files on daemon startup and rebinds cleanly.
  - Non-blocking server dispatch: IPC polling integrated into the Wayland/GStreamer event loop without thread blocking.
- [x] **Protocol implementation**:
  - Request/Response envelope protocol via JSON Lines.
  - Maximum request size limit enforced (64 KB).
  - Subcommands: `status`, `set-video`, `pause`, `resume`, `stop`, `reload`.
- [x] **Full CLI suite (`fluffy`)**:
  - `fluffy daemon`: Resident background daemon.
  - `fluffy status`: Queries and outputs real-time state, video path, generation, loop count.
  - `fluffy set-video <path>`: Switches wallpaper via IPC.
  - `fluffy pause` / `resume` / `stop`: Controls playback state via IPC.
- [x] **Generation semantics (Section 9.3)**:
  - Verified rejection of stale/outdated requests: Request with `--generation 0` sent to active daemon (generation 2) is blocked with:
    `Error: Ipc("Stale request generation: 0 < current 2")`.
- [x] **Clean shutdown & restoration**:
  - On SIGINT (Ctrl+C), players stop, layer surface unmaps, socket file is deleted, and the desktop wallpaper managed by `cosmic-bg` is immediately and cleanly restored.

#### Phase 4: Cache / Import / Normalization [COMPLETE]
- [x] **`ffprobe` Video Validation (`src/cache/probe.rs`)**:
  - Extracted resolution, fps, codec, duration, audio presence.
  - Enforced 4K boundary policy (`width <= 3840 && height <= 2160`).
  - Verified rejection of oversized 5K input (`5120x1440`) immediately with `Error: Probe("Video dimensions 5120x1440 exceed maximum allowed 4K boundary (3840x2160)")` before executing any expensive encoding.
- [x] **`ffmpeg` Normalization (`src/cache/normalize.rs`)**:
  - Transcoding to standard playback profile: H.264 (`libx264`), `yuv420p`, 30 fps, no audio (`-an`), no subtitles (`-sn`), even dimensions (`normalize_even_dimensions`).
  - Executed directly with argument arrays (no shell interpolation).
  - Immediate cleanup of temporary files on conversion failure.
- [x] **Atomic Storage Management (`src/cache/manager.rs`)**:
  - Storage under `~/.local/share/fluffy/storage/videos/<sha256>.mp4` and `metadata/<sha256>.json`.
  - Atomic installation via unique temporary filenames (`.tmp.<pid>.<timestamp>.<hash>.mp4`) and `fs::rename`.
  - Content SHA-256 hash calculation verified on real files.
  - Storage hit reuse verified: Second import of `test.mp4` completes in <10ms by detecting existing video and metadata without re-encoding.
- [x] **Daemon & CLI Integration**:
  - `fluffy import <path>` subcommand available for pre-importing videos into storage.
  - `fluffy set-video <path>` in daemon automatically validates and normalizes video via `CacheManager` before triggering playback.
  - Tested on live daemon over IPC: `[Storage] Storage hit -> reusing ...` log confirmed on real hardware.

#### Phase 5: Multi-output Management [COMPLETE]
- [x] **`OutputManager` architecture (`src/daemon/output_manager.rs`)**:
  - Concurrent management of multiple physical displays (`1 output = 1 layer surface = 1 GStreamer pipeline`).
  - Dynamic resolution detection per display (`WlOutput` logical size / current mode).
- [x] **Concurrent dual playback on real hardware (`DP-1` + `DP-2`)**:
  - Both `DP-1` (2560x1440) and `DP-2` (2560x1440) simultaneously created `Layer::Bottom` surfaces and hardware decoder pipelines.
  - Both displays maintained smooth independent loop playback without cross-pipeline interference.
- [x] **Global vs. Per-output IPC control**:
  - Global: `fluffy set-video test.mp4` applied to both `DP-1` and `DP-2` simultaneously.
  - Per-output: `fluffy set-video test2.mp4 --output DP-2` switched only `DP-2`, while `DP-1` continued playing `test.mp4` undisturbed.
  - Independent pause/resume/stop: `pause --output DP-1` paused only `DP-1` while `DP-2` continued playing; `stop --output DP-2` stopped only `DP-2` while `DP-1` continued playing.
- [x] **Clean multi-surface teardown**:
  - SIGINT cleanly tore down both surfaces and restored desktop wallpapers on both monitors.

#### Phase 6: Settings GUI (libcosmic) [COMPLETE]
- [x] **Standalone client isolation (`fluffy-settings`)**:
  - Configured as a dedicated binary target (`src/bin/fluffy-settings.rs`) gated by `[features] gui = ["dep:libcosmic", "dep:tokio", "dep:rfd"]`.
  - Resident daemon binary (`fluffy`) remains lightweight (48MB unoptimized debug vs. 459MB GUI binary), free from GUI dependencies.
  - Exposes core Fluffy library (`src/lib.rs`) for IPC client, cache, and error types.
- [x] **Dynamic daemon status & output query (Real-hardware-tested)**:
  - `IpcClient::status()` invoked on launch and queries active outputs (`DP-1`, `DP-2`), their current wallpaper URI, generation, and loop counter.
  - Dual daemon launch prevention verified on hardware: second daemon safely aborted with `Error: Ipc("Another Fluffy daemon instance is already running at \"/run/user/1000/fluffy.sock\"")`.
- [x] **Target display selector (Real-hardware-tested)**:
  - Selectable "All Displays (Global)" vs. specific detected monitors (`DP-1`, `DP-2`).
- [x] **Native file picker & normalization (Real-hardware-tested)**:
  - Integrated `rfd::FileDialog` for native Wayland file picking (`*.mp4`, `*.webm`, `*.mkv`, `*.mov`).
  - Asynchronous background task imports and normalizes video via `CacheManager::import_video()` (verified: H.264/yuv420p/30fps transcode to `~/.local/share/fluffy/storage/videos/`).
  - Storage hit verified: Subsequent selection reuses storage normalization instantly (`Storage hit -> reusing ...`).
- [x] **IPC command triggers on real hardware (Real-hardware-tested)**:
  - `Apply Video Wallpaper` (Global): Both `DP-1` and `DP-2` simultaneously updated to `8a9a7c...mp4`.
  - `Apply Video Wallpaper` (Per-output): `DP-2` updated to `597799...mp4` while `DP-1` continued playing undisturbed (loop #10, #11, ...).
  - Quick action playback controls: `Pause` (both paused), `Resume` (both resumed), `Stop` (both stopped), then re-applied without flaw.
- [x] **Zero-flicker seamless transition triggered from GUI (Real-hardware-tested)**:
  - Verified `Preroll completed: res=Ok(Success), current=Paused, pending=VoidPending` and smooth transition to playing state on `DP-2`.
- [x] **Non-blocking lifecycle (Real-hardware-tested)**:
  - GUI is an ephemeral on-demand IPC client; opening/closing does not interrupt daemon playback.

#### Phase 7: Hardening, systemd & Benchmarks [COMPLETE]
- [x] **systemd `--user` Service Unit (`data/systemd/fluffy.service`)**:
  - Bound to `graphical-session.target` (`PartOf`, `After`, `Requisite`).
  - Strict lifecycle ownership: `Restart=on-failure`, `RestartSec=3`, `TimeoutStopSec=5`.
  - Pass-through of required Wayland & desktop environment variables (`WAYLAND_DISPLAY`, `XDG_CURRENT_DESKTOP`, etc.).
- [x] **Structured Logging & journald Integration (`tracing` + `tracing-subscriber`)**:
  - Unified structured logging across `main`, `daemon`, `playback`, `cache`, `wayland`.
  - Configurable via `RUST_LOG` (`fluffy=info,gstreamer=warn` default), outputting cleanly to `journald` or terminal without polluting CLI subcommands.
- [x] **Desktop Entry & Application Integration (`data/desktop/com.github.fluffy.Fluffy.desktop`)**:
  - Registered as standard desktop application for COSMIC Application Library (`Settings;DesktopSettings;HardwareSettings;`).
  - One-click installer script provided (`scripts/install-desktop-integration.sh`).
- [x] **Hotplug & Dynamic Output Management (Real-hardware-tested)**:
  - SCTK `OutputHandler` events (`new_output`, `update_output`, `output_destroyed`) queued and processed in daemon step loop.
  - Automatically initializes layer surfaces and attaches players to newly plugged displays with existing wallpaper.
  - Safely stops players and unmaps surfaces upon display disconnect without crashing resident daemon.
- [x] **Systematic Performance Benchmarks on Real Hardware (Real-hardware-tested)**:
  - Benchmark suite script (`scripts/benchmark.sh`) verified across NVIDIA RTX 3080 & COSMIC desktop:
    - **Daemon Idle**: 0.2% CPU, 40.5 MB RSS, 0.0% GPU Decoder.
    - **1080p30 Single Output (DP-1)**: 10.2% CPU, 312.7 MB RSS, 9.9% GPU Decoder.
    - **1080p30 Dual Output (DP-1 + DP-2)**: 18.0% CPU, 514.3 MB RSS, 10.5% GPU Decoder.
    - **1440p30 Dual Output (DP-1 + DP-2)**: 27.4% CPU, 615.5 MB RSS, 26.6% GPU Decoder.
    - **4K30 Dual Output (DP-1 + DP-2)**: 43.9% CPU, 854.1 MB RSS, 50.6% GPU Decoder.
  - Full details archived in [`docs/BENCHMARK_REPORT.md`](BENCHMARK_REPORT.md).
- [x] **Binary Size Optimization (Verified)**:
  - Resident daemon release binary: **4.7 MB** (**3.3 MB** stripped, zero GUI bloat).
  - Settings GUI release binary: **30 MB** (**21 MB** stripped).

### 2.2 Future Polish Items (Non-blocking / Backlog)
1. **Direct client binding of `wp_viewporter`** (currently handled internally by `waylandsink` and layer-shell 4-edge anchors).
2. **Fractional scaling** fine-tuning under COSMIC compositors.
3. **Ultrawide / mixed aspect ratio cropping policies**.

------------------------------------------------------------------------

## 3. Post-Review Hardening Deliverables (P1, P2, P3 Complete)

Following external technical reviews, the following architectural hardening items have been implemented, verified, and integrated into the core codebase:

### 3.1 P1 Hardening: Reliability & Race-Condition Prevention
- **Container Format Validation**: `ffprobe` probes `format_name` in addition to stream codec. Only true MP4 containers (`mp4`/`mov`) bypass transcoding; MKV, WebM, and other containers are normalized to standard MP4 even if the underlying stream is H.264, preventing mislabeled non-MP4 files in storage.
- **Request Generation Monotonic Reservation**: Generation IDs are reserved immediately at the moment `set-video` arrives at the daemon controller (`self.generation += 1`), not when transcoding finishes. If a newer request arrives while a transcode is running, the finished transcode is detected as stale (`job_gen < current_gen`) and cleanly discarded without clobbering newer playback.
- **systemd Absolute Path**: `fluffy.service` uses `ExecStart=%h/.local/bin/fluffy daemon`, ensuring reliable startup under `systemd --user` regardless of session `PATH` variations.

### 3.2 P2 Hardening: Multi-Job Tracking & Dynamic Output Geometry
- **JobManager with 8-State Lifecycle**: Replaced single-job status with `HashMap<JobId, TranscodeJob>` tracking 8 distinct states: `Queued`, `Probing`, `Transcoding`, `Installing`, `Completed`, `Failed`, `Cancelled`, `Stale`. Multiple concurrent conversions no longer clobber daemon status.
- **Dynamic OutputGeometry Tracking**: `OutputManager` captures logical dimensions, scale factor, and position from `wl_output` events (`update_output`), dynamically resizing surfaces and viewport rectangles (`overlay.set_render_rectangle`) upon resolution or scale changes.

### 3.3 P3 Hardening: Concurrency Optimization & Best-Effort Semantics
- **In-Flight Deduplication**: Duplicate or concurrent `set-video` requests for identical content hashes reuse active background transcode jobs rather than spawning redundant `ffmpeg`/`ffprobe` processes.
- **Best-Effort All-Output Semantics**: Multi-display video updates apply on a best-effort basis; partial failures (e.g. DP-1 succeeds, DP-2 fails) are reported per-output via `SetVideoResult` (`outputs: Vec<OutputApplyResult>`), avoiding fragile full-pipeline rollbacks.
- **Fully Async Settings GUI IPC**: `fluffy-settings` runs all IPC queries and commands through asynchronous tasks (`iced::Task`), keeping the user interface completely fluid and non-blocking under all conditions.

### 3.4 Brand Identity, Legal & Compliance Clearance
- **Application Icon Redesign (`images/fluffy-icon.svg`)**: Revised icon to remove official COSMIC branding resemblance, establishing an independent design centered on a custom flowing "F" frame with minimal display and play functional glyphs in a matte finish.
- **Full Asset Synchronization**: Synchronized `images/fluffy-icon.svg` with desktop assets (`data/icons/hicolor/scalable/apps/fluffy-icon.svg`, `com.github.fluffy.Fluffy.svg`, `com.github.wammed.fluffy.settings.svg`), compiled Settings GUI binary (`include_bytes!`), and local user icon cache.
- **IP & License Documentation (`legal/`)**: Consolidated all compliance and licensing records in `legal/`, including icon provenance (`legal/IP_COMPLIANCE.md` / `legal/IP_COMPLIANCE.ja.md`), dependency and runtime audits (`legal/THIRD_PARTY_LICENSES.md` / `legal/THIRD_PARTY_LICENSES.ja.md`), and design history (`legal/ICON_DESIGN_HISTORY.md` / `legal/ICON_DESIGN_HISTORY.ja.md`). Added `deny.toml` for `cargo-deny` license and security validation.

------------------------------------------------------------------------

## 4. Key Architectural Decisions Established

### 4.1 Non-Destructive Overlay Model (Coexistence with `cosmic-bg`)
- Native `cosmic-bg` is **never** terminated or modified. It remains idle (0% CPU, 0% GPU) on `Layer::Background`.
- Fluffy renders on `Layer::Bottom` directly covering `cosmic-bg`, underneath desktop icons, docks, and windows.
- Any crash, exit, or stop immediately unmaps Fluffy's surface, showing the original wallpaper instantly.

### 4.2 Dual-Pipeline Preroll-Before-Switch
- Changing URI on a single pipeline causes `gstwaylandsink` to render `NULL` buffer on `PAUSED -> READY`, flashing desktop background.
- Fluffy instantiates a secondary pipeline attached to the same parent surface, pauses it until preroll finishes, switches to `PLAYING`, and tears down the old pipeline. Result: 100% flicker-free.

### 4.3 Stale Request Rejection (Generation Semantics)
- All state changes increment a monotonic generation counter per output.
- Requests with older generation IDs are rejected over IPC, preventing race conditions during rapid switching.

### 4.4 Deterministic Media Normalization & Atomic Caching
- Any arbitrary video input is validated with `ffprobe` (4K boundary check `width <= 3840 && height <= 2160`).
- Oversized input is rejected before transcoding.
- Normalization produces consistent H.264 / `yuv420p` / 30fps / no-audio / even dimensions.
- SHA-256 content addressing guarantees zero redundant transcoding overhead.

### 4.5 Concurrent Multi-Output Independence & Best-Effort Semantics
- Each physical monitor receives its own dedicated layer-shell surface and its own GStreamer playback pipeline (`1 output = 1 surface = 1 pipeline`).
- Operations on one output do not block, interrupt, or tear down playback on another output. All-output updates apply best-effort per monitor.

------------------------------------------------------------------------

## 5. Repository Layout (as of Phase 6 implementation)

``` text
Fluffy/
├── Cargo.toml                  (Features: default (daemon/cli), gui (libcosmic))
├── Cargo.lock
├── deny.toml                   (cargo-deny dependency license & security configuration)
├── build.rs
├── README.md                   (English root documentation)
├── README.ja.md                (Japanese root documentation)
├── legal/
│   ├── IP_COMPLIANCE.md        (Icon provenance & IP due diligence record: English)
│   ├── IP_COMPLIANCE.ja.md     (Icon provenance & IP due diligence record: Japanese)
│   ├── THIRD_PARTY_LICENSES.md (Dependency audit & runtime license record: English)
│   ├── THIRD_PARTY_LICENSES.ja.md (Dependency audit & runtime license record: Japanese)
│   ├── ICON_DESIGN_HISTORY.md  (Multi-app icon design history: English)
│   └── ICON_DESIGN_HISTORY.ja.md (Multi-app icon design history: Japanese)
├── images/
│   ├── fluffy-icon.svg         (App icon master SVG)
│   └── fluffy-banner.png       (Project header banner PNG)
├── data/
│   ├── systemd/
│   │   └── fluffy.service      (systemd --user service unit)
│   ├── desktop/
│   │   └── com.github.fluffy.Fluffy.desktop (COSMIC Desktop application entry)
│   └── icons/
│       └── hicolor/scalable/apps/
│           ├── fluffy-icon.svg
│           ├── com.github.fluffy.Fluffy.svg
│           └── com.github.wammed.fluffy.settings.svg
├── scripts/
│   ├── install-desktop-integration.sh (Desktop integration & icon installer)
│   └── benchmark.sh            (Hardware benchmark suite)
├── docs/
│   ├── PORTAL.md / PORTAL.ja.md
│   ├── BENCHMARK_REPORT.md / BENCHMARK_REPORT.ja.md
│   ├── TECHNICAL_DESIGN.md / TECHNICAL_DESIGN.ja.md
│   └── SESSION_HANDOVER.md / SESSION_HANDOVER.ja.md
├── src/
│   ├── lib.rs                  (Exposes modules for binaries & GUI)
│   ├── main.rs                 (CLI subcommands & daemon entry)
│   ├── bin/
│   │   └── fluffy-settings.rs  (libcosmic Settings GUI binary)
│   ├── error.rs                (Consolidated FluffyError)
│   ├── cache/                  (Cache & Normalization subsystem)
│   │   ├── mod.rs
│   │   ├── probe.rs            (ffprobe wrapper & 4K validator)
│   │   ├── normalize.rs        (ffmpeg transcoder & even dimension)
│   │   └── manager.rs          (CacheManager, atomic write, hash key)
│   ├── daemon/                 (Daemon controller & multi-output manager)
│   │   ├── mod.rs
│   │   ├── controller.rs       (Main daemon loop & IPC dispatch)
│   │   └── output_manager.rs   (OutputManager & ManagedOutput collection)
│   ├── ipc/                    (Unix domain socket IPC subsystem)
│   │   ├── mod.rs
│   │   ├── protocol.rs         (JSON-RPC envelopes & validation)
│   │   ├── server.rs           (Non-blocking IpcServer & stale cleanup)
│   │   └── client.rs           (Timeout-safe IpcClient)
│   ├── playback/               (GStreamer playback core)
│   │   ├── mod.rs
│   │   ├── pipeline.rs         (PipelineHandle & Wayland context bind)
│   │   ├── player.rs           (VideoPlayer trait & GstVideoPlayer)
│   │   └── state.rs            (PlaybackState)
│   └── wayland/                (Wayland client core)
│       ├── mod.rs
│       ├── connection.rs       (WaylandContext & registry)
│       └── layer_surface.rs    (WallpaperSurface on Layer::Bottom)
├── test.mp4
└── test2.mp4
```

------------------------------------------------------------------------

## 6. Updated Test Matrix

### Layer Shell
- [x] `Layer::Bottom` surface appears (Real-hardware-tested)
- [x] Surface covers output geometry (Real-hardware-tested)
- [x] Surface disappears cleanly and restores wallpaper (Real-hardware-tested)
- [x] Multi-monitor concurrent surfaces (`DP-1` + `DP-2`) (Real-hardware-tested)
- [x] Output removal / hotplug dynamic handling (Real-hardware-tested)
- [x] Dynamic OutputGeometry tracking and surface update on resolution/scale change (Unit-tested & Real-hardware-tested)

### GStreamer Playback Core
- [x] H.264 playback via hardware decoder (`nvh264dec`) (Real-hardware-tested)
- [x] `about-to-finish` gapless loop transition without decoder stall (Unit-tested & Real-hardware-tested)
- [x] FFmpeg `xfade` seamless head/tail crossfading (`loop_crossfade_ms`) (Unit-tested)
- [x] Safe EOS seek fallback without recreation (Real-hardware-tested)
- [x] Pause and resume state transitions (Real-hardware-tested)
- [x] Dual-pipeline flicker-free switching (Real-hardware-tested)
- [x] Stop & unmap (Real-hardware-tested)
- [x] Dual-output independent pipelines (`DP-1` + `DP-2`) (Real-hardware-tested)

### IPC & Daemon
- [x] Unix socket bind & non-blocking poll (Real-hardware-tested)
- [x] Stale socket cleanup on crash/restart (Real-hardware-tested)
- [x] JSON-RPC serialization & deserialization (Unit-tested & Real-hardware-tested)
- [x] Malformed JSON & oversized request rejection (Unit-tested)
- [x] `status` command (Real-hardware-tested)
- [x] `set-video` command (global and per-output) (Real-hardware-tested)
- [x] `pause` / `resume` / `stop` / `reload` commands (global and per-output) (Real-hardware-tested)
- [x] Generation semantics & stale request rejection (Unit-tested & Real-hardware-tested)
- [x] Monotonic generation reservation at request arrival time (Unit-tested)
- [x] JobManager 8-state lifecycle management (Unit-tested)
- [x] In-flight transcode deduplication by content hash (Unit-tested)
- [x] Per-subscriber target output & generation separation in deduplicated jobs (`JobSubscriber`) (Unit-tested)
- [x] Non-blocking off-main-thread SHA-256 computation to preserve Wayland dispatch latency (Unit-tested & Real-hardware-tested)
- [x] Best-effort multi-output status reporting (`SetVideoResult`) (Unit-tested)
- [x] Timeout-safe client execution (Integration-tested & Real-hardware-tested)

### Cache & Import (Phase 4 COMPLETE)
- [x] `ffprobe` stream & codec inspection (Integration-tested & Real-hardware-tested)
- [x] Container format validation (MP4 bypass vs MKV/WebM normalization) (Unit-tested)
- [x] 4K resolution limit rejection (`5120x1440` rejected immediately) (Unit-tested & Real-hardware-tested)
- [x] Odd-dimension normalization (`normalize_even_dimensions`) (Unit-tested)
- [x] `ffmpeg` normalization to H.264/yuv420p/30fps/even dimensions (Integration-tested & Real-hardware-tested)
- [x] Atomic file write via temporary file rename (Integration-tested & Real-hardware-tested)
- [x] Cache metadata tracking and SHA-256 reuse (Integration-tested & Real-hardware-tested)
- [x] Dual storage integrity & rollback on metadata failure (Unit-tested)
- [x] Transcoding failure cleanup (Integration-tested)

### Settings GUI (Phase 6 COMPLETE)
- [x] Feature-gated `fluffy-settings` binary target (`--features gui`) (Compiled & Real-hardware-tested)
- [x] Resident daemon zero-overhead isolation (daemon binary unbloated) (Compiled & Real-hardware-tested)
- [x] COSMIC UI theme & layout (`libcosmic` Application) (Compiled & Real-hardware-tested)
- [x] Asynchronous daemon status subscription & query (Compiled & Real-hardware-tested)
- [x] Fully async IPC command execution via `iced::Task` (Compiled & Real-hardware-tested)
- [x] Native Wayland file picker integration (`rfd`) (Compiled & Real-hardware-tested)
- [x] Display selector (`All Displays`, `DP-1`, `DP-2`) (Compiled & Real-hardware-tested)
- [x] IPC triggers (`set-video`, `pause`, `resume`, `stop`) (Compiled & Real-hardware-tested)
- [x] Real-hardware manual interaction test in COSMIC session (Real-hardware-tested)

### Hardening & Desktop Integration (Phase 7 COMPLETE)
- [x] systemd `--user` service unit with `graphical-session.target` binding (`data/systemd/fluffy.service`) (Compiled & Tested)
- [x] Absolute path `%h/.local/bin/fluffy` in `fluffy.service` (Tested)
- [x] Desktop Entry for COSMIC Application Library (`data/desktop/com.github.fluffy.Fluffy.desktop`) (Tested)
- [x] Structured logging (`tracing` + `tracing-subscriber`) with journald support across daemon, IPC, playback, cache, and Wayland (Real-hardware-tested)
- [x] Unified diagnostics & error handling: zero raw `println!` in resident daemon/subsystems, robust propagation, and categorized UI error display (Unit-tested & Real-hardware-tested)
- [x] Dynamic display hotplug (attach & detach event handling) (Real-hardware-tested)
- [x] Robust CLI argument parser supporting options anywhere (Unit-tested & Real-hardware-tested)
- [x] Systematic performance benchmarks (Idle, 1080p, 1440p, 4K across dual displays) (Real-hardware-tested)
- [x] Automated integration installer (`scripts/install-desktop-integration.sh`) (Tested)

### Optimizations, Refactoring & Fullscreen Auto-Pause (Phase 8 COMPLETE)
- [x] Auto-pause and resume playback upon fullscreen window detection (`pause_on_fullscreen`) (Real-hardware-tested)
- [x] COSMIC Desktop native protocol (`zcosmic_toplevel_info_v1`) auto-detection and binding (Real-hardware-tested)
- [x] wlroots common protocol (`zwlr_foreign_toplevel_manager_v1`) auto-detection and fallback (Tested)
- [x] Real-time compositor support status indicator in settings GUI (`🟢 コンポジター対応` / `⚠️ コンポジター非対応`) (Real-hardware-tested)
- [x] Graceful disabled toggler state on unsupported compositors (Real-hardware-tested)
- [x] Adaptive event loop sleep intervals (5ms transcoding / 16ms playback / 50ms idle) (Unit-tested & Real-hardware-tested)
- [x] Atomic cancellation of superseded in-flight transcode jobs (`cancel_token` + `child.kill()`) (Unit-tested & Real-hardware-tested)
- [x] Hardlink fast-path for compatible video cache imports (Unit-tested)
- [x] `SlotPool` geometry-aware dynamic buffer capacity scaling avoiding overflow crashes (Real-hardware-tested)
- [x] NULL-pointer safety validation for GStreamer Wayland display handle (Unit-tested)
- [x] Replaced unsafe `libc` FFI with safe Rust `rustix` (Unit-tested)
- [x] Declarative CLI argument parser via `clap` derive (Unit-tested & Real-hardware-tested)

### Surface Scaling & Wayland Protocols (Status)
- [x] Surface scaling via 4-edge anchors & waylandsink render rectangle (Real-hardware-tested)
- [ ] Direct client binding of `wp_viewporter` protocol extension (Unimplemented / Backlog)
- [ ] Ultrawide / mixed aspect ratio cropping policies (Unimplemented / Backlog)

------------------------------------------------------------------------

## 7. Current Project State

1. **Current State:**
   All 7 Planned Phases are **COMPLETE** (Real-hardware-tested on NVIDIA RTX 3080 & COSMIC Desktop).
2. **Key Deliverables:**
   - Resident wallpaper daemon: `target/release/fluffy` (4.7 MB / 3.3 MB stripped)
   - COSMIC Settings GUI: `target/release/fluffy-settings` (30 MB / 21 MB stripped)
   - systemd service unit: `data/systemd/fluffy.service`
   - Desktop entry: `data/desktop/com.github.fluffy.Fluffy.desktop`
   - Install helper: `scripts/install-desktop-integration.sh`
   - Benchmark suite: `scripts/benchmark.sh` & `docs/BENCHMARK_REPORT.md`
3. **Guardrails Preserved:**
   - GUI is strictly an ephemeral on-demand IPC client; it never owns layer surfaces or GStreamer pipelines.
   - Non-destructive overlay architecture (`Layer::Bottom` non-destructive overlay over `cosmic-bg`).
