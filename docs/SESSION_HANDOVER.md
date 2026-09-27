# COSMIC Video Wallpaper Manager --- Session Handover

**Status:** Architecture/design phase --- implementation has not
started.\
**Last updated:** 2026-09-27\
**Next owner:** Implementation agent / developer

------------------------------------------------------------------------

## 1. Project Summary

Build a very lightweight loop-video wallpaper manager for COSMIC Desktop
/ Wayland.

The intended user experience is:

``` text
User / autostart
      |
      v
wallpaper daemon
      |
      +-- Wayland layer-shell BACKGROUND
      |
      +-- GStreamer video playback
      |
      +-- Unix socket IPC
```

Settings are handled by a separate `libcosmic` GUI.

The daemon must remain alive after the GUI exits.

------------------------------------------------------------------------

## 2. Current State

### Completed

-   High-level architecture defined.
-   Daemon / GUI separation chosen.
-   Unix-domain-socket IPC chosen.
-   GStreamer chosen for playback.
-   `waylandsink` chosen as the initial Wayland sink candidate.
-   `wlr-layer-shell` BACKGROUND chosen for desktop placement.
-   `wp_viewporter` chosen for compositor-side surface scaling.
-   ffprobe/ffmpeg chosen for media validation and normalization.
-   H.264/yuv420p/MP4/no-audio/30fps defined as the default playback
    profile.
-   3840x2160 maximum input dimensions defined.
-   One output = one layer surface = one playback pipeline defined for
    v1.
-   `systemd --user` identified as the preferred daemon lifecycle
    manager.
-   Performance numbers were deliberately downgraded from hard
    guarantees to measurable targets.

### Not completed

There is currently no confirmed implementation.

The following have not yet been proven on real hardware:

-   GStreamer `waylandsink` rendering into the intended layer-shell
    surface.
-   DMABUF path under COSMIC/cosmic-comp.
-   hardware decoder selection and fallback behavior.
-   viewporter behavior with the chosen rendering path.
-   multi-output playback.
-   output hotplug/reconfiguration.
-   daemon IPC.
-   cache conversion pipeline.
-   GUI.
-   performance measurements.

------------------------------------------------------------------------

## 3. Most Important Next Task

### DO NOT START WITH THE GUI.

The first implementation task is a minimal technical PoC.

Required PoC:

``` text
Rust
 |
 +-- Wayland connection
 |
 +-- wl_surface
 |
 +-- zwlr_layer_surface_v1
 |      layer = BACKGROUND
 |
 +-- GStreamer
        |
        +-- test H.264 MP4
        |
        +-- waylandsink
```

Target:

``` text
COSMIC Desktop / cosmic-comp
one output
```

Success means:

1.  daemon starts;
2.  creates a BACKGROUND layer surface;
3.  GStreamer renders a test video to that surface;
4.  video loops;
5.  no separate visible GStreamer toplevel window appears;
6.  surface covers the intended output;
7.  the compositor remains usable.

This is the primary architecture gate.

------------------------------------------------------------------------

## 4. Critical Technical Finding

GStreamer `waylandsink` creates its own window by default but implements
`GstVideoOverlay`.

Therefore the implementation must investigate the
existing-window/surface integration rather than assuming:

``` text
layer-shell surface + waylandsink
```

will automatically connect.

GStreamer documents `GstVideoOverlay` as the mechanism for directing a
video sink to an application-provided native rendering target.

The exact Wayland handle semantics for the installed GStreamer version
must be verified experimentally.

If the direct `waylandsink` approach cannot be made reliable, stop and
reassess the sink architecture before implementing IPC/GUI/cache layers.

------------------------------------------------------------------------

## 5. Target Media Profile

Default:

``` text
Container: MP4
Codec: H.264/AVC
Pixel format: yuv420p
FPS: 30
Audio: none
Subtitles: none
Progressive: preferred/required
Max width: 3840
Max height: 2160
```

Optional later:

``` text
24 fps
```

Reject before conversion:

``` text
width > 3840
OR
height > 2160
```

Examples:

``` text
3840x2160  OK
3840x1600  OK
2560x1440  OK
5120x1440  REJECT
7680x2160  REJECT
```

------------------------------------------------------------------------

## 6. Important Corrections to Earlier Assumptions

Do not reintroduce these claims:

### Do not claim universal HW decode

Correct:

> Prefer available hardware decoding and fall back when necessary.

Incorrect:

> H.264 guarantees hardware decoding on Intel/AMD/NVIDIA.

### Do not claim guaranteed Zero Copy

Correct:

> Prefer DMABUF-compatible paths when negotiated.

Incorrect:

> DMABUF/waylandsink always gives zero-copy.

### Do not claim Direct Scanout

Correct:

> The compositor may optimize presentation/scanout depending on its own
> conditions.

Incorrect:

> BACKGROUND layer guarantees Direct Scanout.

### Do not claim fixed CPU/RAM numbers

Correct:

> Performance targets must be measured on actual hardware.

------------------------------------------------------------------------

## 7. Proposed Repository Layout

Start simple.

``` text
wallpaper-project/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── docs/
│   ├── TECHNICAL_DESIGN.md
│   └── SESSION_HANDOVER.md
├── src/
│   ├── main.rs
│   ├── daemon/
│   ├── ipc/
│   ├── wayland/
│   ├── playback/
│   ├── cache/
│   ├── config/
│   └── error.rs
└── tests/
```

Do not over-engineer the workspace before the Wayland/GStreamer PoC
works.

------------------------------------------------------------------------

## 8. Suggested Implementation Sequence

### Step 1 --- Environment audit

Record:

``` bash
rustc --version
cargo --version
gst-launch-1.0 --version
gst-inspect-1.0 waylandsink
gst-inspect-1.0 h264parse
gst-inspect-1.0 avdec_h264
```

Also inspect available hardware decoder plugins.

Record compositor/environment details.

------------------------------------------------------------------------

### Step 2 --- Minimal layer-shell surface

Implement:

-   Wayland connection
-   registry discovery
-   `wl_compositor`
-   `wl_output`
-   layer-shell
-   BACKGROUND surface
-   configure/ack handling
-   fullscreen anchoring

No GStreamer yet.

Success criterion:

``` text
A solid-color BACKGROUND surface is visible on the target output.
```

------------------------------------------------------------------------

### Step 3 --- Attach GStreamer

Create the smallest possible pipeline:

``` text
filesrc
 ! qtdemux
 ! h264parse
 ! decoder
 ! waylandsink
```

or use an equivalent controlled GStreamer construction.

Do not optimize pipeline selection yet.

Success criterion:

``` text
test.mp4 -> existing layer-shell surface
```

------------------------------------------------------------------------

### Step 4 --- Verify looping

Verify EOS handling and restart without destroying the layer-shell
surface.

------------------------------------------------------------------------

### Step 5 --- Measure

Measure baseline:

-   RSS
-   CPU
-   dropped frames
-   decoder utilization
-   first-frame latency

Record the exact machine/GPU/driver/GStreamer versions.

------------------------------------------------------------------------

### Step 6 --- Playback abstraction

Only after the PoC works:

``` rust
trait VideoPlayer {
    fn play(&mut self, video: &Path) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}
```

The exact trait can be adapted to the final GStreamer design.

------------------------------------------------------------------------

### Step 7 --- IPC

Implement:

``` text
status
set-video
pause
resume
stop
reload
```

Use:

``` text
request_id
generation
```

in the protocol.

Test stale requests.

------------------------------------------------------------------------

### Step 8 --- Cache/import

Implement:

``` text
ffprobe
 -> validate
 -> ffmpeg
 -> temp file
 -> atomic rename
 -> metadata
```

No shell invocation.

Use process argument arrays.

------------------------------------------------------------------------

### Step 9 --- Multi-output

Implement:

``` text
OutputManager
    |
    +-- OutputPlayer(DP-1)
    +-- OutputPlayer(DP-2)
```

Do not attempt shared decoding yet.

------------------------------------------------------------------------

### Step 10 --- GUI

Only now implement `libcosmic` settings.

Responsibilities:

-   choose video
-   show validation
-   start conversion
-   show conversion result
-   assign output
-   send daemon commands

The GUI must never be the owner of playback state.

------------------------------------------------------------------------

## 9. IPC Initial Protocol

Example:

``` json
{
  "request_id": 1,
  "generation": 7,
  "command": "set_video",
  "output": "DP-1",
  "path": "/home/user/.cache/my-wallpaper/objects/example.mp4"
}
```

Response:

``` json
{
  "request_id": 1,
  "generation": 7,
  "ok": true
}
```

Error:

``` json
{
  "request_id": 1,
  "generation": 7,
  "ok": false,
  "error": {
    "code": "VIDEO_NOT_FOUND",
    "message": "Cached video was not found"
  }
}
```

Protocol details can be changed before implementation, but the
request/generation concept should remain.

------------------------------------------------------------------------

## 10. First Test Matrix

### Layer Shell

-   [ ] BACKGROUND surface appears
-   [ ] surface covers output
-   [ ] surface disappears cleanly
-   [ ] output removal does not crash daemon

### GStreamer

-   [ ] H.264 playback
-   [ ] 30fps
-   [ ] loop
-   [ ] pause
-   [ ] resume
-   [ ] EOS recovery
-   [ ] invalid file error
-   [ ] decoder fallback

### Viewporter

-   [ ] source rectangle
-   [ ] destination size
-   [ ] 16:9 on 16:9
-   [ ] 16:9 on ultrawide
-   [ ] output resize

### IPC

-   [ ] connect
-   [ ] status
-   [ ] set-video
-   [ ] pause
-   [ ] resume
-   [ ] malformed message
-   [ ] stale generation
-   [ ] client disconnect

### Cache

-   [ ] valid cache reuse
-   [ ] invalid cache rejection
-   [ ] conversion failure cleanup
-   [ ] concurrent writes
-   [ ] atomic rename
-   [ ] cache eviction later

------------------------------------------------------------------------

## 11. Real-Hardware Validation

Initial target should be the user's known COSMIC test environment:

``` text
COSMIC Desktop / cosmic-comp
DP-1: 2560x1440
DP-2: 2560x1440
```

After the first successful implementation:

1.  DP-1 only
2.  DP-2 only
3.  DP-1 + DP-2
4.  switch active output
5.  remove/reconnect output
6.  mixed resolutions
7.  scaling
8.  fractional scaling if available

Do not mark mixed-resolution/DPI support as verified until it has been
tested.

------------------------------------------------------------------------

## 12. Performance Benchmark Plan

At minimum:

  Scenario     Measure
  ------------ -------------
  1080p30 x1   CPU/RSS/GPU
  1440p30 x1   CPU/RSS/GPU
  4K30 x1      CPU/RSS/GPU
  1440p30 x2   CPU/RSS/GPU
  4K30 x2      CPU/RSS/GPU

Record:

``` text
CPU %
RSS
GPU 3D utilization
video decoder utilization
dropped frames
first frame latency
switch latency
```

Do not compare measurements unless the GPU, driver, compositor,
GStreamer version, and test media are recorded.

------------------------------------------------------------------------

## 13. Known Risks / Stop Conditions

### Stop condition A

If `waylandsink` cannot reliably target the layer-shell surface:

**Stop feature development.**

Do not build the GUI around an unproven playback backend.

### Stop condition B

If the target GStreamer stack cannot provide the desired hardware path:

Do not promise hardware decode.

Document the actual fallback.

### Stop condition C

If viewporter behavior is inconsistent:

First establish a correct fixed-resolution path, then revisit scaling.

------------------------------------------------------------------------

## 14. Definition of "Ready for GUI"

The project is not ready for GUI work until all are true:

-   [ ] Layer-shell PoC works.
-   [ ] GStreamer video is visible on the layer surface.
-   [ ] Loop works.
-   [ ] Surface remains stable during playback.
-   [ ] At least one output is reliable.
-   [ ] Basic output reconfiguration does not crash.
-   [ ] CPU/RAM baseline recorded.
-   [ ] Playback API boundary is stable enough for IPC.

------------------------------------------------------------------------

## 15. What the Next Developer Should Do First

### Immediate task

Create the repository skeleton and implement **only the Phase 1 PoC**.

Do not implement:

-   GUI
-   ffmpeg conversion
-   cache
-   systemd integration
-   multi-output optimization
-   playlists

until the following question is answered experimentally:

> **Can the selected GStreamer Wayland sink render H.264 frames reliably
> into a Rust-created `wlr-layer-shell` BACKGROUND surface under
> COSMIC/cosmic-comp?**

That answer determines whether the rest of the architecture can proceed
unchanged.

------------------------------------------------------------------------

## 16. Documentation Rules

Every implementation session should update:

``` text
docs/TECHNICAL_DESIGN.md
docs/SESSION_HANDOVER.md
```

Record:

-   current phase
-   completed items
-   failed experiments
-   exact dependency versions when relevant
-   hardware/compositor used for validation
-   test commands
-   test results
-   known limitations
-   next concrete task

Do not mark an item "verified" merely because it compiles.

For Wayland/GStreamer behavior, distinguish:

``` text
Implemented
Compiled
Unit-tested
Integration-tested
Real-hardware-tested
```

------------------------------------------------------------------------

## 17. Final Architecture Goal

``` text
                 +----------------------+
                 |  libcosmic GUI       |
                 |  (short-lived)       |
                 +----------+-----------+
                            |
                       Unix socket
                            |
                            v
                 +----------------------+
                 | Wallpaper daemon     |
                 |                      |
                 | IPC                  |
                 | Output manager       |
                 | Playback manager     |
                 | Cache manager        |
                 +----+------------+----+
                      |            |
                   Output 1     Output 2
                      |            |
                LayerShell    LayerShell
                      |            |
                GStreamer     GStreamer
                      |            |
                    Wayland compositor
                            |
                         Display
```

The daemon is the product core.

The GUI is a client.

The converter/cache is an import subsystem.

Wayland/GStreamer integration is the primary technical risk.

------------------------------------------------------------------------

## 18. Current Handoff Summary

**Current phase:** Architecture / design.

**Primary next action:** Build the minimal Rust + layer-shell +
GStreamer PoC.

**Primary risk:** Reliable rendering from GStreamer `waylandsink` into
an application-owned layer-shell surface under COSMIC.

**Do not proceed to GUI until that risk is resolved.**

**Success milestone:** One real COSMIC output displays a looping
H.264/MP4 video through the daemon with a stable BACKGROUND layer
surface and documented CPU/RAM behavior.
