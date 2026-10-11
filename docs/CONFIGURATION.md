# ⚙️ Fluffy Configuration & State Specifications (XDG Compliant)

<p align="center">
  <a href="CONFIGURATION.md">English</a> | <a href="CONFIGURATION.ja.md">日本語</a> | <a href="PORTAL.md">📚 Documentation Portal</a>
</p>

Fluffy strictly adheres to the XDG Base Directory Specification for configuration and runtime state persistence.

---

## Configuration File (`config.json`)

- **Location**: `$XDG_CONFIG_HOME/fluffy/config.json` (default: `~/.config/fluffy/config.json`)

### Keys and Types

```json
{
  "startup_and_wallpaper": {
    "restore_on_startup": false,
    "autostart_daemon": false,
    "pause_on_fullscreen": false,
    "loop_crossfade_ms": 0,
    "transition_mode": "auto",
    "switch_crossfade_ms": 1000
  }
}
```

| Config Key | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `restore_on_startup` | boolean | `false` | When true, the daemon restores the last applied video wallpaper on each display upon launch or user login (default is stateless startup). |
| `autostart_daemon` | boolean | `false` | Tracks whether the systemd `--user` service (`fluffy.service`) is enabled to start automatically on login. |
| `pause_on_fullscreen` | boolean | `false` | When true, playback automatically pauses when a fullscreen window is detected to conserve CPU/GPU resources. |
| `loop_crossfade_ms` | u32 | `0` | Crossfade duration in milliseconds between video head and tail for seamless looping (`0` disables crossfading). When set, FFmpeg `xfade` filter blends loop boundaries into a seamless file. |
| `transition_mode` | string | `"auto"` | Video switch transition visual mode (`"auto"`, `"crossfade"`, `"hard_cut"`). |
| `switch_crossfade_ms` | u64 | `1000` | Crossfade duration in milliseconds during video switching (default: 1.0 second = 1000ms). |

### Transition Modes and Hardware Requirement Matrix

Crossfading between videos (simultaneous dual-pipeline playback with alpha fading) enhances UX and is automatically selected based on system capability:

| Effective Mode | Automatic Selection Conditions (Minimum Requirements) | Runtime Characteristics & Load |
| :--- | :--- | :--- |
| **Hardware Accelerated Crossfade**<br>(HW Crossfade) | **GPU / Hardware Decoder Available**: <br>• GStreamer `nvcodec` (NVIDIA CUDA/NVDEC), `vaapi` (Intel/AMD), or `vulkan` plugins detected, or individual hardware decoder elements (`nvh264dec`, `cudaconvert`, `vulkanvp9dec`, `vaapih264dec`, etc.).<br>• No display count or resolution ceiling (supports 4K and multi-monitor). | Offloads video decoding to dedicated ASIC (NVDEC / VA-API) with GStreamer alpha element.<br>CPU usage: ~1–3% negligible overhead.<br>Silky-smooth 60fps fade. |
| **High-Spec Software Crossfade**<br>(High-Spec SW Crossfade) | **No HW Decoder + High-Performance Multi-Core CPU**: <br>• Logical CPU thread count is **8 threads or higher** (e.g., Core i7/i9, Ryzen 7/9 multi-core desktop CPUs).<br>• No display count or resolution ceiling (supports 4K and multi-monitor). | Ample CPU parallelism allows decoding two concurrent streams via libav `avdec_h264` without frame drops or desktop stutter.<br>CPU usage: Temporarily 5–15% during transition only. |
| **Low-Load Software Crossfade**<br>(Low-Load SW Crossfade) | **No HW Decoder + Low-Power CPU + Single Low-Res Display**: <br>• Logical CPU thread count is **under 8 threads** (e.g., 4-core mobile/low-power laptop).<br>• Display count: **Single display only** (`output_count == 1`).<br>• Total resolution: **WQHD or lower** (total pixels ≤ 2,560 × 1,440 ≈ 3.68 Mpx, including standard FHD 1920x1080). | Low-power mobile CPUs can safely handle 60fps software decode crossfading for a single screen at 1080p/1440p.<br>CPU usage: Temporarily 10–25% during transition. |
| **HardCut Fallback (Resource Guard)**<br>(HardCut Fallback) | **Triggers on any of the following**: <br>1. **Unsupported Compositor**: Wayland compositor does not support alpha modifier/subsurface alpha control protocol.<br>2. **Low-Power CPU + Heavy Environment**: No hardware decoder AND CPU has **under 8 threads** on **multi-monitor (≥2 screens)** or **high resolution (> WQHD / 4K)** setups.<br>*(Also active when explicitly set to `"hard_cut"`)* | Prevents desktop micro-stutters and frame drops on constrained devices from dual 4K CPU decoding.<br>Immediately tears down the old pipeline for instant flicker-free switch. |

> **💡 Manual Override**: Selecting "Crossfade" in `fluffy-settings` or executing `fluffy config --transition-mode crossfade` bypasses the automated resource guard and forces crossfading unconditionally.

### CLI Management Examples
```bash
fluffy config --restore-on-startup true
fluffy config --autostart true
fluffy config --pause-fullscreen true
fluffy config --loop-crossfade-ms 1000
fluffy config --transition-mode auto       # Automatic detection (default)
fluffy config --transition-mode crossfade  # Forced crossfade
fluffy config --transition-mode hard_cut   # Forced hard cut
fluffy config --switch-crossfade-ms 1500   # Set switch transition duration to 1.5s
```

---

## State File (`state.json`)

- **Location**: `$XDG_STATE_HOME/fluffy/state.json` (default: `~/.local/state/fluffy/state.json`)

Records the path of the last applied normalized video for each connected display output. This enables instant wallpaper restoration during daemon restart or monitor hotplugging without re-transcoding.

```json
{
  "outputs": {
    "DP-1": "/home/user/.local/share/fluffy/storage/videos/a1b2c3d4....mp4",
    "DP-2": "/home/user/.local/share/fluffy/storage/videos/e5f6g7h8....mp4"
  }
}
```
