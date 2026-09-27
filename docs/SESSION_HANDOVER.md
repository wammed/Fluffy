# COSMIC Video Wallpaper Manager --- Session Handover

**Status:** Phase 3 (IPC & Daemon) COMPLETED (Real-hardware-tested) --- Moving to Phase 4 (Cache / Import / Normalize).\
**Last updated:** 2026-09-27\
**Current phase summary:**
- **Phase 1: Wayland / GStreamer PoC** --- **COMPLETE** (Real-hardware-tested)
- **Phase 2: Playback Core** --- **COMPLETE** (Real-hardware-tested)
- **Phase 3: IPC & Daemon** --- **COMPLETE** (Real-hardware-tested)
- **Phase 4: Cache / Import / Normalize** --- **NEXT**
- Phase 5: Multi-output Management --- PENDING
- Phase 6: Settings GUI (libcosmic) --- PENDING
- Phase 7: Hardening & systemd --- PENDING

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
- Stack: Rust 1.98.1 (Edition 2024), GStreamer 1.28.7, `waylandsink`, `nvh264dec`

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

### 2.2 Critical Unverified Items (DO NOT treat as complete)

To maintain strict alignment with the technical specification and test matrix, the following are **explicitly NOT yet verified on real hardware**:

1. **Multi-output simultaneous playback** (`DP-1 + DP-2` dual concurrent playback) -> Phase 5.
2. **Mixed resolution handling** -> Phase 5.
3. **Display hotplug & output removal/addition** -> Phase 5.
4. **Output reconfiguration & scale changes** -> Phase 5.
5. **Fractional scaling** under COSMIC.
6. **Dynamic surface scaling via `wp_viewporter`**.
7. **Cache management & normalization pipeline** (`ffprobe` validation / `ffmpeg` normalization) -> Phase 4.
8. **Systemd user service integration** (`systemd --user`) -> Phase 7.
9. **Settings GUI** (`libcosmic` / Iced) -> Phase 6.
10. **Systematic performance benchmarks** (CPU %, RSS MB, GPU 3D/Video decoder utilization across 1080p, 1440p, 4K) -> Phase 7.

------------------------------------------------------------------------

## 3. Most Important Next Task: Phase 4 (Cache / Import / Normalize)

### Goal:
Build the media normalization and atomic caching subsystem so that arbitrary user videos are validated, safely normalized, and stored before the daemon attempts playback.

### Phase 4 Scope:
1. **`ffprobe` Inspection & Validation**:
   - Validate container, streams, and codecs.
   - Enforce 4K dimension policy: `width <= 3840 && height <= 2160`.
   - Reject oversized videos **before** executing expensive transcoding.
   - Check frame rates, duration, audio/subtitle presence.
2. **`ffmpeg` Normalization**:
   - Transcode to playback profile: MP4 container, H.264 codec, `yuv420p` pixel format, 30 fps, no audio, no subtitles.
   - Ensure even dimensions (width/height divisible by 2).
   - Use direct argument arrays (never `sh -c` or shell strings).
3. **Atomic Cache Storage**:
   - Location: `~/.cache/fluffy/objects/<sha256>.mp4` and `~/.cache/fluffy/metadata/<sha256>.json`.
   - Unique temporary file naming (`.tmp.<pid>.<uuid>`) to prevent collisions during concurrent imports.
   - Atomic rename semantics into final destination.
   - Reuse existing valid cache on identical input.
   - Clean up temporary files on conversion failure.

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

------------------------------------------------------------------------

## 5. Repository Layout (as of Phase 3 completion)

``` text
Fluffy/
├── Cargo.toml
├── Cargo.lock
├── build.rs
├── docs/
│   ├── TECHNICAL_DESIGN.md
│   └── SESSION_HANDOVER.md
├── src/
│   ├── main.rs                 (CLI subcommands & daemon entry)
│   ├── error.rs                (Consolidated FluffyError)
│   ├── daemon/                 (Daemon controller & output manager)
│   │   ├── mod.rs
│   │   └── controller.rs
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
- [ ] Output removal / hotplug handling (Phase 5)

### GStreamer Playback Core
- [x] H.264 playback via hardware decoder (`nvh264dec`) (Real-hardware-tested)
- [x] Seamless loop on EOS without recreation (Real-hardware-tested)
- [x] Pause and resume state transitions (Real-hardware-tested)
- [x] Dual-pipeline flicker-free switching (Real-hardware-tested)
- [x] Stop & unmap (Real-hardware-tested)
- [ ] Software decoder fallback (`avdec_h264`) explicit test

### IPC & Daemon
- [x] Unix socket bind & non-blocking poll (Real-hardware-tested)
- [x] Stale socket cleanup on crash/restart (Real-hardware-tested)
- [x] JSON-RPC serialization & deserialization (Unit-tested & Real-hardware-tested)
- [x] Malformed JSON & oversized request rejection (Unit-tested)
- [x] `status` command (Real-hardware-tested)
- [x] `set-video` command (Real-hardware-tested)
- [x] `pause` / `resume` / `stop` / `reload` commands (Real-hardware-tested)
- [x] Generation semantics & stale request rejection (Unit-tested & Real-hardware-tested)
- [x] Timeout-safe client execution (Integration-tested & Real-hardware-tested)

### Viewporter / Scaling
- [ ] Dynamic destination scaling via `wp_viewporter`
- [ ] Ultrawide / mixed aspect ratio cropping policies

### Cache & Import (Phase 4 Target)
- [ ] `ffprobe` stream & codec inspection
- [ ] 4K resolution limit rejection
- [ ] `ffmpeg` normalization to H.264/yuv420p/30fps/even dimensions
- [ ] Atomic file write via temporary file rename
- [ ] Cache metadata validation and reuse
- [ ] Cleanup on transcoding failure

------------------------------------------------------------------------

## 7. Current Handoff Instructions

1. **Current State:**
   Phase 1, 2, and 3 are **COMPLETE** and verified on real hardware under COSMIC Desktop.
2. **Next Developer Action:**
   Proceed to **Phase 4: Cache / Import / Normalize** (`src/cache/`).
   - Implement `ffprobe` video validator.
   - Implement `ffmpeg` normalizer.
   - Implement atomic cache manager (`~/.cache/fluffy/`).
   - Add unit tests for 4K boundary rejection, odd-dimension normalization, and cache keys.
3. **Guardrails:**
   - Do NOT start GUI development yet (`libcosmic`).
   - Do NOT claim universal hardware decoding or Direct Scanout.
   - Preserve non-destructive overlay architecture (`Layer::Bottom`).
