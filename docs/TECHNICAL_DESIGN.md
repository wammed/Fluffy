# COSMIC Video Wallpaper Manager --- Technical Design Specification

<p align="center">
  <a href="TECHNICAL_DESIGN.md">English</a> | <a href="TECHNICAL_DESIGN.ja.md">日本語</a> | <a href="PORTAL.md">📚 Portal</a>
</p>

**Document:** Technical Design Specification\
**Status:** Draft / implementation-ready\
**Target:** COSMIC Desktop / Wayland\
**Language:** Rust\
**Primary goal:** A permanently running loop-video wallpaper daemon with
minimal CPU/RAM overhead.

------------------------------------------------------------------------

## 1. Purpose

This project provides a lightweight video-wallpaper manager for COSMIC
Desktop on Wayland.

The system is intentionally split into:

1.  **Wallpaper daemon**
    -   Permanently running.
    -   Owns Wayland layer-shell surfaces.
    -   Owns GStreamer playback.
    -   Exposes a small Unix-domain-socket IPC interface.
    -   Contains no GUI toolkit.
2.  **Settings GUI**
    -   Implemented with `libcosmic` / Iced.
    -   Started only when configuration is needed.
    -   Validates and imports videos.
    -   Runs `ffprobe` / `ffmpeg` as external processes.
    -   Sends commands to the daemon through IPC.
    -   May start the daemon when no daemon is available, but the
        preferred lifecycle owner is `systemd --user`.
3.  **CLI / automation**
    -   Talks to the same daemon socket.
    -   Supports scripting and startup automation.

The daemon must continue wallpaper playback if the GUI crashes or exits.

------------------------------------------------------------------------

## 2. Design Principles

### 2.1 Keep the resident process small

The daemon must not load a GUI toolkit.

Target qualities:

-   no GUI toolkit in the daemon
-   no audio pipeline
-   no subtitle pipeline
-   no unnecessary media parsing
-   no video conversion during normal playback
-   one playback pipeline per output in v1

The exact RSS/CPU figures are **measurement targets, not hard
guarantees**.

### 2.2 Normalize media before playback

Imported media is converted to a known playback profile.

Default playback profile:

-   Container: MP4
-   Video codec: H.264/AVC
-   Pixel format: `yuv420p`
-   Progressive video
-   Audio: none
-   Subtitles: none
-   Default frame rate: 30 fps
-   Optional supported frame rate: 24 fps
-   Maximum width: 3840
-   Maximum height: 2160

The 4K policy is a dimension policy:

`width <= 3840 && height <= 2160`

Examples:

-   3840x2160: accepted
-   3840x1600: accepted
-   2560x1440: accepted
-   5120x1440: rejected
-   7680x2160: rejected

The importer must reject oversized input **before invoking an expensive
downscale encode**.

### 2.3 Prefer hardware decode, never promise it

Hardware decode is preferred; the actual decoder depends on the GStreamer environment and installed system plugins.

Examples of platform paths include:

-   NVIDIA hardware decode via NVDEC (`nvh264dec`) where the driver and GStreamer plugins expose it (verified on this test environment: NVIDIA GeForce RTX 3080, Driver 615.71.09, GStreamer 1.28.7)
-   VA-API on supported Intel/AMD environments (`vaapih264dec` or `vah264dec`)
-   Software decoding fallback (`avdec_h264`) when hardware decoders are absent or fail negotiation

The project does not claim that hardware decode is guaranteed across all environments, but dynamically selects the highest-ranked available decoder via GStreamer `playbin`.

### 2.4 Prefer DMABUF paths when negotiated

The architecture should permit a GStreamer → Wayland DMABUF path.

However:

-   DMABUF availability is environment-dependent.
-   Format/modifier negotiation can fail.
-   Zero-copy must not be treated as an unconditional guarantee.

The daemon should favor the efficient path and retain a functional
fallback.

### 2.5 Do not guarantee Direct Scanout

`wlr-layer-shell` BACKGROUND placement does not mean that the compositor
will always use direct scanout.

Therefore the project does not promise:

-   GPU 3D utilization of 0%
-   direct scanout
-   a specific compositor scheduling strategy

The measurable goals are instead:

-   avoid CPU-side frame conversion where possible
-   prefer hardware decoding
-   prefer DMABUF-compatible paths
-   avoid unnecessary frame copies
-   keep frame rate bounded
-   keep resolution bounded

------------------------------------------------------------------------

## 3. Wayland Architecture

### 3.1 Layer Shell

Each output gets a dedicated `wl_surface` with a `zwlr_layer_surface_v1`
role on the `bottom` or `background` layer.

Conceptual model:

``` text
Output
  |
  +-- wl_surface
        |
        +-- zwlr_layer_surface_v1 (Layer::Bottom / background)
              |
              +-- base surface (mapped)
                    |
                    +-- wl_subsurface (GStreamer video frames)
```

The output object must be explicitly supplied to `get_layer_surface()`
when the application wants deterministic output binding.

#### 3.1.1 Coexistence with COSMIC Wallpaper Daemon (`cosmic-bg`) & Intentional Overlay Design

> **Architecture & Design Note:**
> This project intentionally implements a **non-destructive overlay model** rather than terminating or replacing the desktop environment's native wallpaper daemon (`cosmic-bg`).

1. **Why `cosmic-bg` is NOT killed or disabled:**
   - `cosmic-bg` is an integral part of the COSMIC Desktop session (`cosmic-session`). Terminating or masking it introduces system instability, requires elevated permissions or session modifications, and risks breakage across OS upgrades.
   - Once `cosmic-bg` renders its initial static image, it remains idle waiting for events. It consumes **0% CPU and 0% GPU**, incurring negligible background resource overhead (idle memory only).

2. **The Overlay Architecture (`Layer::Bottom`):**
   - By rendering on `zwlr_layer_surface_v1` with `Layer::Bottom`, Fluffy places its video surface directly **above** the `Layer::Background` (where `cosmic-bg` resides), but **below** desktop icons, panels, docks, and normal application windows.
   - Visual Z-Order:
     ``` text
     [Top / Overlay] Windows, Panels, Docks, Desktop Icons
            ^
            |
     [Layer::Bottom] ★ Fluffy Video Wallpaper Surface (covers 100% of output)
            ^
            |
     [Layer::Background] cosmic-bg static wallpaper (idle in background)
     ```

3. **Graceful Fail-Safe & Clean Teardown:**
   - If Fluffy exits, pauses, stops, or even crashes unexpectedly, its Wayland surface is automatically unmapped by the compositor.
   - The user's original desktop wallpaper managed by `cosmic-bg` immediately becomes visible without leaving the screen black and without requiring user intervention or wallpaper reconfiguration.
   - This design guarantee ensures zero destructive impact on user desktop settings.

### 3.2 Surface Scaling & Protocol Status (Implemented vs. Unimplemented)

To maintain precise architectural clarity, Wayland features are distinguished as follows:

1. **Implemented & Verified:**
   - **`zwlr_layer_shell_v1`**: Full layer-shell integration on `Layer::Bottom` covering target outputs without stealing focus.
   - **Output Hotplug**: Real-time detection of display connection (`new_output`) and disconnection (`output_destroyed`).
   - **Output Geometry & Scale Updates**: Dynamic resolution updates, monitor scale factor tracking, and logical positioning via `wl_output` events without daemon restart.
   - **Surface Scaling**: Wallpaper surfaces anchor to all four borders (`TOP | BOTTOM | LEFT | RIGHT`) while video frame rendering is scaled to surface bounds via GStreamer `overlay.set_render_rectangle(0, 0, width, height)`.
   - **Multi-Output Management**: Independent layer surfaces and playback pipelines per physical monitor (`1 output = 1 surface = 1 pipeline`).

2. **Unimplemented / Future Protocol Enhancement:**
   - **`wp_viewporter` / `wp_viewport` Protocol Extension**: Direct Wayland protocol binding of `wp_viewporter` in the Rust client code is **not implemented** in v1. Video frame scaling and viewport adjustments are currently handled internally by `waylandsink` and the Wayland compositor (`cosmic-comp`). Direct client-side `wp_viewport` integration is tracked as a future enhancement for fractional pixel cropping.

### 3.3 GStreamer integration risk

`waylandsink` creates a Wayland window by default and implements
`GstVideoOverlay`.

The project therefore must not assume that simply creating `waylandsink`
will automatically render into the application's layer-shell surface.

The first technical proof-of-concept must verify:

``` text
Rust Wayland connection
    |
    +-- wl_surface
    +-- layer-shell BACKGROUND
    |
    +-- GStreamer waylandsink
           |
           +-- existing Wayland target integration
```

GStreamer provides a `GstVideoOverlay` mechanism for directing a sink to
an existing native window/surface handle. The exact Wayland handle
integration must be verified experimentally against the installed
GStreamer version and the COSMIC environment.

**This is the highest-risk integration point and must be validated
before the full application is built.**

------------------------------------------------------------------------

## 4. Process Architecture

``` text
                         +----------------------+
                         |  Settings GUI        |
                         |  libcosmic / Iced    |
                         +----------+-----------+
                                    |
                              Unix socket
                                    |
                                    v
+-------------+          +----------------------+
| CLI/scripts |--------->| Wallpaper daemon     |
+-------------+          |                      |
                         | IPC controller       |
                         | Output manager       |
                         | Playback manager      |
                         | Cache manager        |
                         +----------+-----------+
                                    |
                    +---------------+---------------+
                    |                               |
                    v                               v
             +-------------+                 +-------------+
             | Output DP-1 |                 | Output DP-2 |
             | LayerShell  |                 | LayerShell  |
             | GStreamer   |                 | GStreamer   |
             +-------------+                 +-------------+
```

Preferred lifecycle:

``` text
systemd --user
      |
      +-- wallpaper-daemon
```

The GUI should not own the daemon lifetime permanently.

For manual usage:

``` text
wallpaper-daemon run
```

must remain possible.

------------------------------------------------------------------------

## 5. Rust Workspace / Module Structure

A single Cargo package is acceptable for the first implementation.

Recommended logical modules:

``` text
src/
├── main.rs
├── daemon/
│   ├── mod.rs
│   └── controller.rs
├── ipc/
│   ├── mod.rs
│   ├── protocol.rs
│   └── server.rs
├── wayland/
│   ├── mod.rs
│   ├── connection.rs
│   ├── output.rs
│   ├── layer_surface.rs
│   └── viewport.rs
├── playback/
│   ├── mod.rs
│   ├── pipeline.rs
│   ├── player.rs
│   └── state.rs
├── cache/
│   ├── mod.rs
│   ├── index.rs
│   └── objects.rs
├── config/
│   └── mod.rs
└── error.rs
```

GUI can initially be a separate package/binary:

``` text
src/bin/wallpaper-settings.rs
```

or later become a workspace member:

``` text
crates/
├── daemon/
├── settings/
└── protocol/
```

A shared protocol crate is preferred once the IPC protocol stabilizes.

------------------------------------------------------------------------

## 6. Core Data Model

### 6.1 Daemon

``` rust
struct WallpaperDaemon {
    outputs: HashMap<OutputId, OutputState>,
    cache: CacheManager,
    config: Config,
    ipc: IpcServer,
}
```

### 6.2 Output state

``` rust
struct OutputState {
    output: WaylandOutput,
    surface: LayerSurface,
    viewport: Option<Viewport>,
    playback: PlaybackState,
}
```

### 6.3 Playback state

``` rust
struct PlaybackState {
    current: Option<VideoId>,
    requested: Option<VideoId>,
    generation: u64,
}
```

`generation` prevents stale asynchronous requests from replacing a newer
requested video.

------------------------------------------------------------------------

------------------------------------------------------------------------

## 7. Multi-monitor Model

The v1 model is:

> **one output = one layer surface = one playback pipeline**

Example:

``` text
DP-1
  -> LayerSurface
  -> GStreamer Pipeline A

DP-2
  -> LayerSurface
  -> GStreamer Pipeline B
```

This intentionally does not attempt shared decoding across monitors in v1.

If the same video is used on multiple outputs, v1 decodes it independently on each output.

### 7.1 Best-Effort All-Output Semantics

When `set-video` is invoked without specifying an `--output` (global broadcast to all displays), the daemon applies the video on a **best-effort** basis per output:
- Each output pipeline is updated independently.
- If Output A succeeds and Output B fails (e.g., subsurface allocation error or pipeline failure on Output B), Output A transitions to the new video while Output B retains its existing playback state.
- The IPC response returns a structured `SetVideoResult` containing a list of `OutputApplyResult` items (specifying each `output`, `success: bool`, and optional `error: String`).
- A full transactional rollback across all outputs is intentionally avoided, as rolling back GStreamer Wayland pipelines after partial commitment introduces state corruption and visual tearing risks.

------------------------------------------------------------------------

## 8. Output Lifecycle & Dynamic Geometry

Required events handled by `OutputManager`:

-   `new_output`: Output discovered; creates `ManagedOutput`, binds layer surface on `Layer::Bottom`, and initializes playback pipeline.
-   `output_destroyed`: Output disconnected; cleanly tears down GStreamer pipeline, unmaps layer surface, and removes output state.
-   `update_output`: Output geometry or scale changed; dynamically updates `OutputGeometry` (logical width, height, scale factor, x, y coordinates).

### 8.1 Authoritative Geometry Updates

The daemon does not treat the first `wl_output.geometry` event as final. Instead:
- Sizing configuration waits for `wl_output.done` and layer surface configure events.
- When resolution or scale changes (e.g. 2560x1440 -> 3840x2160, or scale 1.0 -> 1.5):
  - Surface dimensions and layer surface anchors are reconfigured.
  - Viewport render rectangle (`overlay.set_render_rectangle(0, 0, width, height)`) is updated.
  - If geometry changed while playing, the player pipeline is cleanly reconfigured for the new resolution.
- Output identification uses stable Wayland connector names (`DP-1`, `DP-2`, `HDMI-A-1`) matching COSMIC display configuration.

------------------------------------------------------------------------

## 9. IPC Protocol

Transport:

``` text
Unix domain socket
```

Location:

``` text
$XDG_RUNTIME_DIR/fluffy.sock (fallback: /run/user/<UID>/fluffy.sock)
```

The socket has strict `0600` file permissions (owner only) and automatically clears stale dead sockets upon daemon startup.

### 9.1 Commands

Minimum v1 command set:

``` text
status
set-video
pause
resume
stop
reload
```

### 9.2 Request envelope

Logical protocol:

``` json
{
  "request_id": 123,
  "generation": 42,
  "command": "set_video",
  "output": "DP-1",
  "path": "/home/user/video.mp4"
}
```

### 9.3 Generation Semantics (Monotonic Reservation at Request Arrival)

To prevent race conditions during asynchronous video transcoding and switching:
1. **Reservation at Request Arrival**: The daemon controller increments its monotonic generation counter (`self.generation += 1`) immediately upon receiving the `set-video` request, *not* when transcoding completes.
2. **Stale Job Detection**: The assigned generation is passed into the background `TranscodeJob`. When the job completes, the daemon controller checks `job.generation == self.generation`.
3. If a newer request (e.g. Generation 11) arrived while Generation 10 was transcoding:
   ``` text
   Request A arrives -> Generation 10 assigned, transcode begins
   Request B arrives -> Generation 11 assigned, transcode begins
   Request B finishes -> 11 == current (11) -> applied to outputs
   Request A finishes -> 10 < current (11) -> detected as STALE -> discarded cleanly
   ```
   Older asynchronous tasks never overwrite newer user requests.

### 9.4 JobManager & In-Flight Deduplication

Asynchronous video normalization is managed by `JobManager` with explicit lifecycle tracking:
- **8-State Model**: Each job progresses through defined states:
  `Queued` -> `Probing` -> `Transcoding` -> `Installing` -> `Completed` (or `Failed`, `Cancelled`, `Stale`).
- **In-Flight Deduplication**: When a `set-video` request is received, the content SHA-256 hash is checked. If an active job for the same content hash is already running (`Queued`, `Probing`, `Transcoding`, or `Installing`), the new request attaches to or reuses the existing job instead of spawning duplicate `ffprobe` / `ffmpeg` processes.
- **Multiple Concurrent Jobs**: The daemon accurately tracks multiple active conversion jobs via `HashMap<JobId, TranscodeJob>`, avoiding single-job state clobbering.

### 9.5 Asynchronous Settings GUI IPC

The `fluffy-settings` GUI executes all IPC interactions (queries, status checks, video commands) asynchronously via `iced::Task` / dedicated background worker threads. The GUI interface remains completely responsive (0 frame drops, no input lag) regardless of daemon transcode load or socket latency.

### 9.6 IPC validation

The daemon strictly validates:
-   command type
-   required fields
-   path type (canonicalized, rejecting path traversal)
-   target output
-   generation
-   request size (max 64 KB)
-   malformed input

The daemon never executes arbitrary shell commands received over IPC.

------------------------------------------------------------------------

## 10. Video Import Pipeline & Container Validation

``` text
User selects input
       |
       v
     ffprobe (container + video stream + 4K check)
       |
       +-- invalid / oversized (>3840x2160) --> reject
       |
       +-- MP4 container && H.264 && yuv420p && progressive && <=30fps && even dims
       |         |
       |         v
       |   direct bypass (instant copy, no transcoding CPU cost)
       |
       v
  non-MP4 container (MKV/WebM) or non-compliant codec (HEVC/AV1/60fps)
       |
       v
     ffmpeg (H.264 / yuv420p / 30fps / no-audio / even dims)
       |
       v
  temporary storage file (.tmp.<pid>.<time>.<hash>.mp4)
       |
       v
  atomic rename + metadata install
       |
       v
  IPC set-video
```

### 10.1 Container & Stream Validation (`ffprobe`)

Validation checks:
-   container format (`format_name` must contain `mp4` or `mov` for direct bypass)
-   video stream presence
-   width & height (enforcing 4K boundary: `width <= 3840 && height <= 2160`)
-   frame rate (<= 30 fps)
-   codec (`h264`, `avc1`)
-   pixel format (`yuv420p`)
-   interlacing/progressive status

**Strict Container Rule**: Even if a file's video stream is H.264 / `yuv420p`, if its container is MKV, WebM, AVI, etc., it does **not** bypass transcoding. It is transcoded into a genuine MP4 container to avoid storing mislabeled container payloads as `.mp4`.

### 10.2 Normalization (`ffmpeg`)

Normalized profile:
``` text
Container: MP4
Codec: H.264 (libx264)
Pixel format: yuv420p
Framerate: 30 fps
Audio: stripped (-an)
Subtitles: stripped (-sn)
Dimensions: normalized to even numbers
```

Executed directly with argument arrays (no `sh -c` shell interpolation).

### 10.3 Failure Recovery & Dual-Store Integrity

- Incomplete temporary files (`.tmp.*`) are removed immediately on failure.
- When installing into persistent storage, the normalized video file and metadata JSON are installed atomically. If metadata writing fails, the video file is cleanly rolled back and deleted to ensure no orphaned video files without metadata exist.

------------------------------------------------------------------------

## 11. Persistent Storage Design

Structure (`~/.local/share/fluffy/storage/`):

``` text
~/.local/share/fluffy/storage/
├── videos/
│   ├── <content-sha256-hash>.mp4
│   └── ...
└── metadata/
    ├── <content-sha256-hash>.json
    └── ...
```

Metadata schema:

``` json
{
  "source": "/path/to/input",
  "source_size": 123456,
  "source_mtime": 1234567890,
  "profile": {
    "codec": "h264",
    "pixel_format": "yuv420p",
    "fps": 30,
    "audio": false,
    "max_width": 3840,
    "max_height": 2160
  },
  "output": "<content-sha256-hash>.mp4"
}
```

Temporary files use collision-free unique names (`.tmp.<pid>.<timestamp>.<hash>.mp4`).
The final cache file is installed atomically using rename semantics.

------------------------------------------------------------------------

## 12. Playback Switching

Avoid:

``` text
stop old
destroy surface
create new
start new
```

because this can produce visible blank frames.

Preferred sequence:

``` text
current pipeline
      |
      | continue playing
      |
new pipeline
      |
      +-- PAUSED
      +-- preroll
      +-- first frame available
      |
      v
switch
      |
      v
new pipeline PLAYING
      |
      v
old pipeline teardown
```

The exact switching primitive depends on the chosen GStreamer
architecture and must be validated in the PoC.

### Verified Architecture & PoC Findings (Phase 2):
1. **Root Cause of Single-Pipeline Discontinuity:**
   In `gstwaylandsink.c`, the state transition `PAUSED -> READY` executes:
   `gst_wl_window_render(self->window, NULL, NULL); /* remove buffer from surface, show nothing */`.
   This unmaps the buffer from the subsurface, causing a visible flicker/black flash if the same pipeline is stopped to change URI.
2. **Dual Pipeline (Preroll-Before-Switch) Verification:**
   - Wayland's `wl_subsurface` allows multiple child subsurfaces to attach to the same `WallpaperSurface`.
   - New subsurfaces are stacked **above** existing subsurfaces by default in Wayland compositors (cosmic-comp).
   - Video B's pipeline is constructed, attached to the same `WallpaperSurface`, and transitioned to `PAUSED` (preroll).
   - Once B's preroll finishes (first frame committed to compositor), B transitions to `PLAYING`.
   - Video A's pipeline is then immediately transitioned to `NULL` and dropped.
   - Result: Zero visible blank frame, zero flicker, stable layer-shell surface.

The layer-shell surface itself should remain stable during a normal
video change.

------------------------------------------------------------------------

## 13. Looping

Loop playback must happen without recreating the Wayland surface.

The preferred behavior is:

``` text
video EOS
  |
  v
seek to 0
  |
  v
PLAYING
```

rather than:

``` text
EOS
 -> destroy pipeline
 -> create pipeline
 -> recreate surface
```

This reduces latency and avoids visual flicker.

------------------------------------------------------------------------

## 14. Performance Targets

These are **targets to measure**, not contractual guarantees.

### Resident daemon

Measure:

-   RSS
-   CPU %
-   GPU utilization
-   video decoder utilization
-   frame drops
-   power consumption where measurable

Test at:

-   1080p30
-   1440p30
-   4K30
-   one output
-   two outputs

Hardware environments should be recorded.

### Binary size

Record actual release binary size after stripping where appropriate.

Do not promise a fixed size before implementation.

------------------------------------------------------------------------

## 15. GUI Responsibilities

The GUI owns:

-   video selection
-   validation status
-   conversion progress
-   cache management UI
-   output selection
-   per-output video assignment
-   pause/resume controls
-   error display

The GUI does not own:

-   permanent playback state
-   Wayland background surfaces
-   long-lived GStreamer pipelines
-   daemon lifecycle state

GUI crash must not stop playback.

------------------------------------------------------------------------

## 16. systemd --user Integration

Unit configuration:

``` ini
[Unit]
Description=Fluffy Video Wallpaper Manager Daemon
Documentation=https://github.com/wammed/Fluffy
PartOf=graphical-session.target
After=graphical-session.target
Requisite=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/fluffy daemon
Restart=on-failure
RestartSec=3
TimeoutStopSec=5
PassEnvironment=WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_RUNTIME_DIR XDG_SESSION_TYPE
Environment=RUST_LOG=fluffy=info

[Install]
WantedBy=graphical-session.target
```

Using `%h/.local/bin/fluffy daemon` ensures the service starts reliably regardless of `systemd --user` environment `PATH` variations.

------------------------------------------------------------------------

## 17. Error Handling

Error categories:

``` text
WaylandError
GstreamerError
IpcError
CacheError
ProbeError
ConversionError
ConfigError
```

Errors exposed to users should be concise.

Internal logs should contain:

-   subsystem
-   output
-   request_id
-   generation
-   source path where safe
-   GStreamer state
-   error details

Do not log secrets or arbitrary IPC payloads unnecessarily.

------------------------------------------------------------------------

## 18. Security / Trust Boundaries

The daemon is a local IPC service.

Assumptions:

-   Unix socket permissions restrict access to the user.
-   IPC messages are untrusted input.
-   Video paths are untrusted input.
-   ffprobe/ffmpeg are external programs and their exit status must be
    checked.
-   No shell interpolation should be used for paths.
-   External commands should be spawned with argument arrays, not
    `sh -c`.
-   The daemon must not accept arbitrary executable commands through
    IPC.

Path handling must avoid accidental traversal when paths are converted
into cache identifiers.

------------------------------------------------------------------------

## 19. Testing Strategy

### Unit tests

Required:

-   4K boundary validation

-   4K rejection

-   odd-dimension normalization

-   unsupported media rejection

-   cache key generation

-   metadata validation

-   generation/stale request rejection

-   IPC serialization/deserialization

-   malformed IPC rejection

### Integration tests

Required:

-   ffprobe success/failure
-   ffmpeg success/failure
-   cache atomic write
-   cache reuse
-   daemon socket startup
-   set-video command
-   pause/resume
-   looping

### Wayland hardware tests

First target:

``` text
COSMIC / cosmic-comp
DP-1: 2560x1440
DP-2: 2560x1440
```

Then:

-   mixed resolutions
-   output add/remove
-   output reconfiguration
-   mixed scale
-   fractional scaling if supported by the target compositor

### Playback tests

Measure:

-   first frame latency
-   loop transition
-   video switch latency
-   dropped frames
-   CPU
-   RAM
-   GPU decode utilization

------------------------------------------------------------------------

## 20. Definition of Done for MVP

MVP is complete only when all are true:

1.  A Rust daemon creates a COSMIC-compatible BACKGROUND layer surface.
2.  GStreamer can render the test H.264 video into the intended Wayland
    target.
3.  One output plays a looping video.
4.  Output geometry changes are handled without corrupting the surface.
5.  Unix-socket `status` and `set-video` work.
6.  4K+ input is rejected before expensive conversion.
7.  Valid input is normalized to the playback profile.
8.  Cache writes are atomic and safe under concurrent imports.
9.  GUI can select/import a video and tell the daemon to switch.
10. GUI exit/crash does not stop playback.
11. Hardware decoding is preferred where available, with a documented
    fallback.
12. Performance is measured on real hardware.
13. No documentation claims Direct Scanout or universal hardware decode
    as guaranteed.

------------------------------------------------------------------------

## 21. Development Order

### Phase 0 --- Protocol / environment inspection [DONE - Real-hardware-tested]

-   [x] confirm COSMIC compositor protocol availability (cosmic-comp, wayland-1)
-   [x] confirm installed GStreamer version/plugins (GStreamer 1.28.7, waylandsink, nvh264dec, avdec_h264)
-   [x] confirm layer-shell support (zwlr_layer_shell_v1 functional under cosmic-comp)
-   [x] inspect Rust bindings/crates (smithay-client-toolkit 0.21, gstreamer 0.25, libgstwayland-1.0)

### Phase 1 --- Critical PoC [DONE - Real-hardware-tested]

Implemented and verified:

``` text
Wayland connection
+
layer-shell BACKGROUND
+
GStreamer playbin / waylandsink (nvh264dec HW accelerated)
+
H.264 MP4 loop playback
```

**Gate PASSED:** Proven and visually validated on real hardware (NVIDIA RTX 3080, COSMIC Desktop, DP-1 & DP-2 2560x1440).
- Key technical requirement identified: Passing `GstWaylandDisplayHandleContext` via `gst_wl_display_handle_context_new` is mandatory for surface sharing.
- Key technical requirement identified: Explicit `overlay.set_render_rectangle(0, 0, width, height)` is required by `waylandsink` when targeting an external surface.
- Key technical requirement identified: Parent layer surface must map an initial base buffer for the subsurface video to render.
- Key layer selection: `Layer::Bottom` displays directly above `cosmic-bg` wallpaper and under desktop icons.
- Seamless loop playback without recreating surface verified via EOS `seek_simple(ZERO)`.
- Clean exit without crashing or hanging confirmed.

### Phase 2 --- Playback core [DONE - Real-hardware-tested]

Implemented and verified:
- [x] Codebase modularization: `src/error.rs`, `src/wayland/`, `src/playback/`.
- [x] `VideoPlayer` abstraction trait & `GstVideoPlayer` implementation.
- [x] State management (`Playing`, `Paused`, `Stopped`).
- [x] Pause / Resume lifecycle verified without frame drops or tearing.
- [x] Dynamic video switching without destroying layer-shell surface verified.
- [x] Automatic seamless loop recovery via EOS `seek_simple(ZERO)`.
- [x] Clean exit signal handling (SIGINT/Ctrl+C) and surface teardown.

### Phase 3 --- IPC [DONE - Real-hardware-tested]

Implemented and verified:
- [x] Unix domain socket IPC architecture (`src/ipc/`): non-blocking `IpcServer` & timeout-safe `IpcClient`.
- [x] Safe socket lifecycle: automated stale socket detection/cleanup, restricted file permissions (0600).
- [x] JSON-RPC/JSONL protocol with Request/Response envelopes and maximum request size enforcement (64KB).
- [x] Full command suite implemented:
  - `status`: real-time output state, current playing video, generation, loop count.
  - `set-video`: dynamic video switching via IPC.
  - `pause` / `resume`: pause and resume playback via IPC.
  - `stop`: stop playback and unmap frames.
  - `reload`: reload current video.
- [x] Generation semantics (Section 9.3): stale request detection and rejection verified (`gen < current`).
- [x] CLI subcommand suite integrated in `src/main.rs` (`daemon`, `status`, `set-video`, `pause`, `resume`, `stop`, `reload`).
- [x] Comprehensive unit & integration tests for serialization, socket communication, stale recovery, and validation.
- [x] Verified on real COSMIC / Wayland hardware with live daemon and IPC clients.

### Phase 4 --- Cache/import [DONE - Real-hardware-tested]

Implemented and verified:
- [x] `ffprobe` video probe and metadata validation (`src/cache/probe.rs`).
- [x] 4K boundary validation policy (`width <= 3840 && height <= 2160`); rejects oversized inputs (e.g. 5120x1440 5K ultrawide) before expensive transcoding.
- [x] `ffmpeg` transcoding normalization (`src/cache/normalize.rs`):
  - Normalizes to H.264 / `yuv420p` / 30fps / no audio (`-an`) / no subtitles (`-sn`) / even dimensions.
  - Safe external process execution via argument arrays without shell interpolation.
  - Immediate cleanup of temporary files on conversion failure.
- [x] Persistent storage & compatibility bypass (`src/cache/manager.rs` & `src/cache/probe.rs`):
  - Moved from ephemeral cache to persistent storage (`~/.local/share/fluffy/storage/videos/<sha256>.mp4`).
  - Compatible video profile bypass: videos matching H.264/`yuv420p`/<=30fps/even dims/<=4K bypass ffmpeg entirely and copy directly (instantaneous, no transcoding CPU cost).
  - Non-compliant videos (HEVC, AV1, 60fps+, odd dimensions) undergo automatic normalization in background worker threads.
  - Transparent initial layer surface base buffer (`0x00000000`), preserving the desktop wallpaper during initial conversion without black screen interruptions.
  - Asynchronous normalization via dedicated worker threads keeps Wayland event dispatch and ongoing video loops running smoothly.
  - Visual animated conversion indicators in `fluffy-settings` and status reporting in `fluffy status`.
- [x] Atomic storage writing (`src/cache/manager.rs`):
  - Content-based SHA-256 hash naming (`~/.local/share/fluffy/storage/videos/<sha256>.mp4`).
  - Metadata tracking (`~/.local/share/fluffy/storage/metadata/<sha256>.json`).
  - Collision-free unique temporary files (`.tmp.<pid>.<timestamp>.<hash>.mp4`) with atomic rename.
  - Automatic storage hit detection and zero-overhead reuse.
- [x] Integration with WallpaperDaemon controller and CLI (`fluffy import <path>` and automatic normalization on `set-video`).
- [x] Unit & integration tests for 4K validation, odd dimensions, hash generation, atomic write, bypass check, and reuse.

### Phase 5 --- Multi-output [DONE - Real-hardware-tested]

Implemented and verified:
- [x] `OutputManager` architecture (`src/daemon/output_manager.rs`) managing arbitrary number of concurrent outputs.
- [x] Concurrent multi-monitor playback (`DP-1` + `DP-2` 2560x1440 simultaneous playback verified).
- [x] Independent pipeline per output (`1 output = 1 layer surface = 1 GStreamer pipeline`).
- [x] Dynamic geometry detection per output (retrieving resolution from `WlOutput` logical size / current mode instead of fixed constants).
- [x] Global vs. Per-output IPC control:
  - `set-video <path>`: applies to all active outputs simultaneously.
  - `set-video <path> --output <NAME>`: applies to specified output only, leaving other outputs playing undisturbed.
  - `pause` / `resume` / `stop`: independent per-output control confirmed on real hardware.
- [x] Clean multi-surface teardown and desktop wallpaper restoration.

### Phase 6 --- GUI [COMPLETE - Real-hardware-tested]

Add:

- [x] Dedicated `fluffy-settings` binary using `libcosmic` / Iced (isolated via `gui` cargo feature to keep resident daemon minimal).
- [x] Real-time daemon connection & status display (`IpcClient` queries daemon state, current output count, generations, and loop counts).
- [x] Display management: Lists active displays (`DP-1`, `DP-2`), allows target selection (`All Displays` or specific monitor).
- [x] Video file picker via native file dialog (`rfd`) with automatic `CacheManager` normalization.
- [x] Output assignment: Sends `set-video` IPC command to targeted display.
- [x] Playback controls: `Pause`, `Resume`, `Stop`, `Refresh Status` directly from GUI.
- [x] Clean lifecycle: GUI is strictly an on-demand IPC client; closing it leaves daemon playback running undisturbed.

### Phase 7 --- hardening [COMPLETE]

Implemented and verified on real hardware:

- [x] systemd --user service unit (`data/systemd/fluffy.service`) bound to `graphical-session.target`.
- [x] Desktop Entry for COSMIC Application Library (`data/desktop/com.github.fluffy.Fluffy.desktop`).
- [x] Structured logging via `tracing` & `tracing-subscriber` routed cleanly to `journald` and console.
- [x] Failure recovery & dynamic display hotplug (attach/detach event handling via SCTK `OutputHandler`).
- [x] Flexible positional and flag CLI parsing across all subcommands.
- [x] Systematic performance benchmark suite (`scripts/benchmark.sh`) measuring CPU%, RSS MB, GPU 3D%, GPU NVDEC% across 1080p, 1440p, 4K on dual displays.
- [x] Benchmarking documented in [`docs/BENCHMARK_REPORT.md`](BENCHMARK_REPORT.md).
- [x] Minimal release binary footprints: resident daemon **4.7 MB** (**3.3 MB** stripped), GUI **30 MB** (**21 MB** stripped).

------------------------------------------------------------------------

## 22. Known Risks

### P0 --- GStreamer/Wayland target integration

The exact mechanism for directing `waylandsink` to the existing
layer-shell surface must be proven first.

### P1 --- Hardware decoder availability

Hardware decode differs by GPU, driver, GStreamer plugin set, and
distribution packaging.

### P1 --- DMABUF negotiation

Zero-copy may fail because of format/modifier compatibility.

### P1 --- compositor behavior

COSMIC/cosmic-comp may differ from wlroots compositors in details
relevant to layer-shell, scaling, presentation, or scanout.

### P2 --- multi-output synchronization

One pipeline per output is intentionally simple but may duplicate
decoding work.

### P2 --- cache storage growth

A cache eviction policy is required before long-term use.

------------------------------------------------------------------------

## 23. Explicit Non-Goals for v1

Do not implement initially:

-   audio playback
-   subtitles
-   playlists
-   network streaming
-   8K input
-   60fps default playback
-   shared decoder across outputs
-   complex video effects
-   GUI preview rendering through a second full playback stack
-   guaranteed Direct Scanout
-   universal hardware-decoding guarantee

------------------------------------------------------------------------

## 24. Legal / Licensing Position

The project invokes `ffmpeg` / `ffprobe` as external processes rather
than linking the FFmpeg libraries into the daemon.

Documentation must describe this as an architectural fact, not as a
blanket legal conclusion.

Do not claim:

-   that H.264 patent obligations never apply
-   that private use automatically eliminates all patent issues
-   that external-process execution makes all licensing questions
    irrelevant

Users/distributors must assess the legal and licensing conditions
applicable to their jurisdiction and distribution model.

The project may independently choose a permissive license for its own
source code, subject to the licenses of its direct dependencies.

------------------------------------------------------------------------

## 25. Source / Technical References

Primary references used during design validation:

-   GStreamer `GstVideoOverlay`
-   GStreamer `waylandsink`
-   GStreamer Rust bindings for `GstVideoOverlay`
-   Wayland `wlr-layer-shell`
-   Wayland `wp_viewporter`
-   FFmpeg legal/licensing documentation

These should be rechecked against the exact dependency versions used by
the implementation.

------------------------------------------------------------------------

## 26. Logging and Diagnostics Policy

To maintain high observability and stability across long-running daemon sessions, the following logging and error handling standards must be followed for all existing and newly added code:

### 26.1 Structured Logging via `tracing`
* **Daemon & Library Modules**: All resident daemon, playback, cache, Wayland, and IPC server modules **must strictly use `tracing::*`** (`error!`, `warn!`, `info!`, `debug!`, `trace!`).
* **Zero `println!` / `eprintln!` in Resident Code**: Direct standard output writes are forbidden in daemon code. Only CLI subcommands (`fluffy status`, `fluffy import`, etc.) may print formatted terminal text for human CLI users.
* **Standard Event Levels**:
  * `info!`: Lifecycle milestones (daemon startup/shutdown, video import, cache hit/install, playback state changes, output add/remove).
  * `warn!`: Recoverable issues (stale generation discards, socket cleanup warnings, compositor layer close).
  * `error!`: Actionable failures (ffprobe failure, ffmpeg transcode failure, pipeline bus error).
  * `debug!` / `trace!`: Verbose protocol events (IPC request arrival, preroll timings, benign cleanups).

### 26.2 Diagnostic Context
Include structured fields in tracing macros where available:
* `operation`: e.g. `"pipeline_switch"`, `"ffmpeg_transcode"`, `"cache_lookup"`, `"job_complete"`.
* Identifiers: `output = %name`, `generation`, `job_id`, `client_id = client_id.0`.
* Avoid redundant spamming of oversized raw paths in log bodies; use structured keys.

### 26.3 Error Propagation & Ignored Operations
* Never use silent `let _ = ...` on filesystem mutations, pipeline state transitions, or IPC responses without justification.
* Benign cleanups (e.g., temporary file deletion or disconnected client writes) must be logged at `debug!` or `trace!` level with context, or accompanied by an explicit code comment documenting why it is safe to ignore.

### 26.4 GUI Resilience
* The GUI client (`fluffy-settings`) must never panic (`.unwrap()`) on daemon connection or IPC failures.
* Failures must be classified into structured categories (`connection unavailable`, `timeout`, `daemon stopped`, `invalid response`) and gracefully presented to the user.

------------------------------------------------------------------------

## 27. Architecture Optimizations & Fullscreen Detection (Phase 8 Hardening)

### 27.1 Auto-Pause on Fullscreen Windows (Dual Wayland Protocol)
To preserve GPU and CPU resources during gaming or media consumption, Fluffy monitors Wayland toplevel states and automatically pauses/resumes video playback pipelines.
* **COSMIC Desktop Native**: Binds `zcosmic_toplevel_info_v1` (`cosmic-protocols`), tracking toplevel `state` events (`Fullscreen = 3`).
* **wlroots / Sway / Hyprland**: Falls back to `zwlr_foreign_toplevel_manager_v1` (`wayland-protocols-wlr`) to track equivalent fullscreen window states.
* **Startup Capability Detection**: Probes compositor advertised globals during Wayland initialization to establish `supports_fullscreen_detection`, reporting this via IPC `DaemonStatus`.
* **GUI Integration**: `fluffy-settings` displays a real-time status label (`🟢 コンポジター対応` / `⚠️ コンポジター非対応`) and disables the toggle switch if the compositor lacks support.

### 27.2 Dynamic Adaptive Loop Sleep
Replaces a static 5ms busy-wait sleep with an adaptive polling interval matching the daemon's operational phase:
* Background transcoding active: **5 ms** (responsive process polling).
* Normal video playback: **16 ms** (~60fps event loop rate).
* Complete idle: **50 ms** (reduces CPU usage below 0.1%).

### 27.3 Atomic Cancellation of Superseded Transcoding Jobs
When rapid consecutive wallpaper changes are issued, prior in-flight ffmpeg transcoding processes for the same output are superseded:
* Managed by `JobManager::cancel_superseded_jobs`.
* Uses `Arc<AtomicBool>` cancellation tokens with immediate `child.kill()` execution, terminating orphaned ffmpeg subprocesses and cleaning temporary files immediately.

### 27.4 Hardlink Fast-Path for Compliant Videos
When importing videos that already comply with standard specifications (H.264 / <=30fps / yuv420p / even dimensions), Fluffy attempts `fs::hard_link` within the same filesystem. This eliminates disk I/O and duplicate storage space, with automatic fallback to `copy` across filesystems.

### 27.5 Memory Safety and Hardening
* **NULL Verification**: Added explicit NULL checks for `gst_wl_display_handle_context_new` to prevent segmentation faults during initialization failures.
* **Dynamic Buffer Allocation**: In `SlotPool`, dynamic buffer capacity calculation now scales with geometry changes, completely avoiding buffer overflow crashes on display hotplug or resolution adjustments.
* **Safe POSIX FFI**: Replaced unsafe `libc::getuid` calls with safe `rustix::process::getuid()`.
* **Declarative CLI**: Migrated manual string-splitting argument parsing to type-safe declarative `clap` derive structures.
