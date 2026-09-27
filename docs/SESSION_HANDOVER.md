# COSMIC Video Wallpaper Manager --- Session Handover

**Status:** Phase 6 (Settings GUI) COMPLETED (Real-hardware-tested) --- Moving to Phase 7 (Hardening & systemd).\
**Last updated:** 2026-09-27\
**Current phase summary:**
- **Phase 1: Wayland / GStreamer PoC** --- **COMPLETE** (Real-hardware-tested)
- **Phase 2: Playback Core** --- **COMPLETE** (Real-hardware-tested)
- **Phase 3: IPC & Daemon** --- **COMPLETE** (Real-hardware-tested)
- **Phase 4: Cache / Import / Normalize** --- **COMPLETE** (Real-hardware-tested)
- **Phase 5: Multi-output Management** --- **COMPLETE** (Real-hardware-tested)
- **Phase 6: Settings GUI (libcosmic)** --- **COMPLETE** (Real-hardware-tested)
- Phase 7: Hardening & systemd --- NEXT

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
      +-- Cache & Import Subsystem (~/.cache/fluffy/)
      |     +-- ffprobe 4K validation
      |     +-- ffmpeg H.264/yuv420p/30fps normalization
      |     +-- SHA-256 atomic caching & instant reuse
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
- [x] **Hardware decoding**: Verified automatic `nvh264dec` hardware acceleration under GStreamer `playbin`.

#### Phase 2: Playback Core [COMPLETE]
- [x] **Codebase modularization**: Clean architecture under `src/error.rs`, `src/wayland/`, `src/playback/`.
- [x] **VideoPlayer trait & GstVideoPlayer**:
  - `play(video)`: Instant playback start.
  - `pause()`: Video frames freeze smoothly.
  - `resume()`: Smooth resume without frame drops.
  - `stop()`: Clean pipeline transition to `NULL`.
- [x] **Zero-flicker seamless video switching**: Dual-pipeline pre-roll architecture verified. Transition from video A (`test.mp4`) to video B (`test2.mp4`) renders completely flicker-free without blank frames or black flashes.
- [x] **Seamless loop playback**: EOS bus message triggers `seek_simple(ZERO)` without recreating pipeline or surface (verified across 17+ continuous loop cycles on real hardware).

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
- [x] **Atomic Cache Management (`src/cache/manager.rs`)**:
  - Cache storage under `~/.cache/fluffy/objects/<sha256>.mp4` and `metadata/<sha256>.json`.
  - Atomic installation via unique temporary filenames (`.tmp.<pid>.<timestamp>.<hash>.mp4`) and `fs::rename`.
  - Content SHA-256 hash calculation verified on real files.
  - Cache hit reuse verified: Second import of `test.mp4` completes in <10ms by detecting existing object and metadata without re-encoding.
- [x] **Daemon & CLI Integration**:
  - `fluffy import <path>` subcommand available for pre-importing videos into cache.
  - `fluffy set-video <path>` in daemon automatically validates and normalizes video via `CacheManager` before triggering playback.
  - Tested on live daemon over IPC: `[Cache] Cache hit -> reusing ...` log confirmed on real hardware.

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
- [x] **Native file picker & cache normalization (Real-hardware-tested)**:
  - Integrated `rfd::FileDialog` for native Wayland file picking (`*.mp4`, `*.webm`, `*.mkv`, `*.mov`).
  - Asynchronous background task imports and normalizes video via `CacheManager::import_video()` (verified: H.264/yuv420p/30fps transcode to `~/.cache/fluffy/objects/`).
  - Cache hit verified: Subsequent selection reuses cached normalization instantly (`Cache hit -> reusing ...`).
- [x] **IPC command triggers on real hardware (Real-hardware-tested)**:
  - `Apply Video Wallpaper` (Global): Both `DP-1` and `DP-2` simultaneously updated to `8a9a7c...mp4`.
  - `Apply Video Wallpaper` (Per-output): `DP-2` updated to `597799...mp4` while `DP-1` continued playing undisturbed (loop #10, #11, ...).
  - Quick action playback controls: `Pause` (both paused), `Resume` (both resumed), `Stop` (both stopped), then re-applied without flaw.
- [x] **Zero-flicker seamless transition triggered from GUI (Real-hardware-tested)**:
  - Verified `Preroll completed: res=Ok(Success), current=Paused, pending=VoidPending` and smooth transition to playing state on `DP-2`.
- [x] **Non-blocking lifecycle (Real-hardware-tested)**:
  - GUI is an ephemeral on-demand IPC client; opening/closing does not interrupt daemon playback.

### 2.2 Critical Unverified Items (DO NOT treat as complete)

To maintain strict alignment with the technical specification and test matrix, the following are **explicitly NOT yet verified on real hardware**:

1. **Systemd user service integration** (`systemd --user`) -> Phase 7.
2. **Systematic performance benchmarks** (CPU %, RSS MB, GPU 3D/Video decoder utilization across 1080p, 1440p, 4K) -> Phase 7.
3. **Failure recovery & crash resilience** -> Phase 7.
4. **Display hotplug & output removal/addition during runtime** (dynamic hotplug event handling) -> Phase 7 / hardening.
5. **Fractional scaling** under COSMIC.
6. **Dynamic surface scaling via `wp_viewporter`**.

------------------------------------------------------------------------

## 3. Most Important Next Task: Phase 7 (Hardening, systemd & Benchmarks)

### Goal:
Prepare Fluffy for production-ready desktop integration with a systemd user service unit, automated autostart, error recovery, structured logging, and thorough resource benchmarks.

### Phase 7 Action Items:
1. **systemd `--user` Service Unit**:
   - Provide `fluffy.service` under `~/.config/systemd/user/`.
   - Configure socket activation or dependency on `graphical-session.target`.
   - Ensure clean restart and termination behaviors.
2. **Logging & Journal Integration**:
   - Structured logging via `tracing` or `env_logger` routed cleanly to `journald` when running under systemd.
3. **Performance Benchmark Suite**:
   - Measure CPU usage %, Resident Set Size (RSS MB), and NVIDIA GPU Decoder utilization (`nvtop` / `nvidia-smi`) during multi-monitor 1440p/4K playback.
4. **Desktop Entry & Packaging**:
   - Create `com.github.fluffy.desktop` for `fluffy-settings` in the COSMIC Application Library.

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

### 4.5 Concurrent Multi-Output Independence
- Each physical monitor receives its own dedicated layer-shell surface and its own GStreamer playback pipeline (`1 output = 1 surface = 1 pipeline`).
- Operations on one output do not block, interrupt, or tear down playback on another output.

------------------------------------------------------------------------

## 5. Repository Layout (as of Phase 6 implementation)

``` text
Fluffy/
├── Cargo.toml                  (Features: default (daemon/cli), gui (libcosmic))
├── Cargo.lock
├── build.rs
├── docs/
│   ├── TECHNICAL_DESIGN.md
│   └── SESSION_HANDOVER.md
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
- [ ] Output removal / hotplug dynamic handling (Phase 7 / hardening)

### GStreamer Playback Core
- [x] H.264 playback via hardware decoder (`nvh264dec`) (Real-hardware-tested)
- [x] Seamless loop on EOS without recreation (Real-hardware-tested)
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
- [x] Timeout-safe client execution (Integration-tested & Real-hardware-tested)

### Cache & Import (Phase 4 COMPLETE)
- [x] `ffprobe` stream & codec inspection (Integration-tested & Real-hardware-tested)
- [x] 4K resolution limit rejection (`5120x1440` rejected immediately) (Unit-tested & Real-hardware-tested)
- [x] Odd-dimension normalization (`normalize_even_dimensions`) (Unit-tested)
- [x] `ffmpeg` normalization to H.264/yuv420p/30fps/even dimensions (Integration-tested & Real-hardware-tested)
- [x] Atomic file write via temporary file rename (Integration-tested & Real-hardware-tested)
- [x] Cache metadata tracking and SHA-256 reuse (Integration-tested & Real-hardware-tested)
- [x] Transcoding failure cleanup (Integration-tested)

### Settings GUI (Phase 6 COMPLETE)
- [x] Feature-gated `fluffy-settings` binary target (`--features gui`) (Compiled & Real-hardware-tested)
- [x] Resident daemon zero-overhead isolation (daemon binary unbloated) (Compiled & Real-hardware-tested)
- [x] COSMIC UI theme & layout (`libcosmic` Application) (Compiled & Real-hardware-tested)
- [x] Asynchronous daemon status subscription & query (Compiled & Real-hardware-tested)
- [x] Native Wayland file picker integration (`rfd`) (Compiled & Real-hardware-tested)
- [x] Display selector (`All Displays`, `DP-1`, `DP-2`) (Compiled & Real-hardware-tested)
- [x] IPC triggers (`set-video`, `pause`, `resume`, `stop`) (Compiled & Real-hardware-tested)
- [x] Real-hardware manual interaction test in COSMIC session (Real-hardware-tested)

### Viewporter / Scaling
- [ ] Dynamic destination scaling via `wp_viewporter`
- [ ] Ultrawide / mixed aspect ratio cropping policies

------------------------------------------------------------------------

## 7. Current Handoff Instructions

1. **Current State:**
   Phases 1 through 6 are **COMPLETE** (Real-hardware-tested).
2. **Next Developer Action:**
   Transition to **Phase 7: Hardening, systemd & Performance Benchmarks**.
   - Create systemd `--user` unit file (`fluffy.service`).
   - Create desktop entry (`com.github.fluffy.desktop`).
   - Run system resource and GPU video decode benchmarks across monitors.
3. **Guardrails:**
   - GUI must remain a lightweight IPC client; it must NEVER own background surfaces or GStreamer pipelines.
   - Preserving non-destructive overlay architecture (`Layer::Bottom`).
