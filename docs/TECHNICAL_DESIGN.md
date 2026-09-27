# COSMIC Video Wallpaper Manager --- Technical Design Specification

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

The playback system should prefer an available hardware decoder.

Examples of possible platform paths include:

-   VA-API on supported Intel/AMD environments
-   NVIDIA hardware decode where the installed GStreamer stack exposes
    it
-   software decoding as fallback

The project must not claim that hardware decode is guaranteed on all
Intel/AMD/NVIDIA systems.

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

### 3.2 Viewporter

`wp_viewporter` / `wp_viewport` is used to express source cropping and
destination scaling at the Wayland surface level.

The video is not resized in the Rust render loop merely to match the
monitor geometry.

The compositor remains responsible for the final surface scaling.

The implementation must still verify the actual behavior on
COSMIC/cosmic-comp.

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

This intentionally does not attempt shared decoding in v1.

If the same video is used on multiple outputs, v1 may decode it
independently.

Shared decode / tee-based optimization is a later optimization because
DMABUF sharing, sink negotiation, output formats, and lifetime
management add complexity.

------------------------------------------------------------------------

## 8. Output Lifecycle

Required events:

-   output discovered
-   output metadata updated
-   output removed
-   output geometry changed
-   output scale changed
-   output reconfigured

The daemon must not assume that the first `wl_output` event contains
final logical geometry.

Surface configuration should wait for the relevant compositor configure
sequence before treating the output size as authoritative.

Output identification should use stable Wayland output metadata where
available.

------------------------------------------------------------------------

## 9. IPC Protocol

Transport:

``` text
Unix domain socket
```

Recommended location:

``` text
$XDG_RUNTIME_DIR/my-wallpaper.sock
```

The socket must not be placed in a world-writable persistent directory.

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

Possible future commands:

``` text
set-output-video
clear-output
list-outputs
get-config
```

### 9.2 Request envelope

Logical protocol:

``` json
{
  "request_id": 123,
  "generation": 42,
  "command": "set_video",
  "output": "DP-1",
  "path": "/home/user/.cache/my-wallpaper/objects/abc.mp4"
}
```

### 9.3 Generation semantics

If request 42 is still processing and request 43 supersedes it:

``` text
generation 42 -> stale -> discard
generation 43 -> current -> apply
```

This is mandatory for asynchronous video switching.

### 9.4 IPC validation

The daemon must validate:

-   command type
-   required fields
-   path type
-   target output
-   generation
-   request size
-   malformed input

The daemon must never execute arbitrary shell commands received over
IPC.

------------------------------------------------------------------------

## 10. Video Import Pipeline

``` text
User selects input
       |
       v
     ffprobe
       |
       +-- invalid / oversized --> reject
       |
       v
   normalize metadata
       |
       v
     ffmpeg
       |
       v
 temporary cache file
       |
       v
 atomic rename
       |
       v
   cache metadata
       |
       v
 IPC set-video
```

### 10.1 ffprobe validation

Check at minimum:

-   stream exists
-   video stream exists
-   width
-   height
-   frame rate
-   codec/container information
-   interlacing/progressive status where available
-   duration
-   audio presence

The importer should reject or normalize unsupported inputs
deterministically.

### 10.2 ffmpeg output

The default output should be equivalent in intent to:

``` text
MP4
H.264
yuv420p
30 fps
no audio
no subtitles
even dimensions
```

The exact CRF/preset values should be configurable constants in code and
documented.

Odd dimensions may be normalized to even dimensions before H.264
encoding.

### 10.3 Conversion failure

A failed conversion must:

-   remove the incomplete temporary output
-   preserve the previous valid cache
-   return a user-readable error
-   retain detailed diagnostic logs separately

------------------------------------------------------------------------

## 11. Cache Design

Recommended structure:

``` text
~/.cache/my-wallpaper/
├── objects/
│   ├── <content-or-source-hash>.mp4
│   └── ...
└── metadata/
    ├── <id>.json
    └── ...
```

Metadata should include:

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
  "output": "<cache-id>.mp4"
}
```

Temporary files must use unique names.

Do not use a single fixed `.tmp` filename because concurrent imports can
collide.

The final cache file should be installed atomically using rename
semantics.

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

## 16. systemd --user

Preferred unit concept:

``` text
my-wallpaper.service
```

Properties to consider:

``` text
ExecStart=/path/to/wallpaper-daemon run
Restart=on-failure
```

The exact unit should be generated/documented only after the daemon CLI
is stable.

The GUI may detect a missing daemon and request/start the user service,
then retry IPC.

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

### Phase 3 --- IPC

Add:

-   Unix socket
-   protocol
-   generation
-   status
-   set-video

### Phase 4 --- Cache/import

Add:

-   ffprobe
-   validation
-   ffmpeg
-   atomic cache
-   metadata

### Phase 5 --- Multi-output

Add:

-   output manager
-   per-output pipeline
-   hotplug/reconfiguration

### Phase 6 --- GUI

Add:

-   libcosmic settings
-   video picker
-   conversion progress
-   output assignment
-   daemon control

### Phase 7 --- hardening

Add:

-   systemd --user integration
-   logging
-   stress tests
-   failure recovery
-   performance benchmark suite
-   documentation

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
