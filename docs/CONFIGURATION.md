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
| `transition_mode` | string | `"auto"` | Video switch transition visual mode (`"auto"`, `"crossfade"`, `"hard_cut"`). Note: `"crossfade"` is exclusive to COSMIC desktop environment (`cosmic-comp`). |
| `switch_crossfade_ms` | u64 | `1000` | Crossfade duration in milliseconds during video switching (default: 1.0 second = 1000ms). |

### Transition Modes and Hardware Requirement Matrix

Crossfading between videos (simultaneous dual-pipeline playback with hardware compositor alpha blending) utilizes the `wp_alpha_modifier_v1` protocol provided by the **COSMIC desktop environment (`cosmic-comp`)** to blend layers at the GPU compositor level.
Unlike legacy CPU software alpha filtering, full zero-copy GPU video decoding (NVDEC / VA-API) is preserved, achieving silky-smooth 120Hz/60fps crossfading without CPU overhead or memory copies.

| Effective Mode | Automatic Selection Conditions (Minimum Requirements) | Runtime Characteristics & Load |
| :--- | :--- | :--- |
| **Hardware Alpha Crossfade (COSMIC Exclusive)**<br>(HW Alpha Crossfade) | **COSMIC Desktop + `wp_alpha_modifier_v1` Available**: <br>• COSMIC compositor (`cosmic-comp`) exports the Wayland alpha modifier extension.<br>• GPU / Hardware video decoder available (NVDEC / VA-API) with zero-copy scanout.<br>• No display count or resolution ceiling (supports 4K and 120Hz multi-monitor). | Surface transparency is managed entirely by `cosmic-comp` GPU compositor pipeline.<br>Zero CPU color space conversion or buffer copies in GStreamer.<br>CPU usage: <1% negligible overhead. Silky-smooth 120Hz/60fps transitions. |
| **High-Spec Software Crossfade**<br>(High-Spec SW Crossfade) | **No HW Decoder + High-Performance Multi-Core CPU (COSMIC)**: <br>• Logical CPU thread count is **8 threads or higher** (e.g., Core i7/i9, Ryzen 7/9).<br>• Alpha blending still performed by compositor GPU via `wp_alpha_modifier_v1`. | Software decoding CPU overhead only during switch; compositor handles blending.<br>CPU usage: Temporarily 5–15% during transition only. |
| **Low-Load Software Crossfade**<br>(Low-Load SW Crossfade) | **No HW Decoder + Low-Power CPU + Single Low-Res Display (COSMIC)**: <br>• Logical CPU thread count is **under 8 threads**.<br>• Display count: **Single display only** (`output_count == 1`).<br>• Total resolution: **WQHD or lower** (total pixels ≤ 2,560 × 1,440). | Safe 60fps software decode crossfading for single screen handled smoothly by GPU compositor.<br>CPU usage: Temporarily 10–20% during transition. |
| **HardCut Fallback (Non-COSMIC / Resource Guard)**<br>(HardCut Fallback) | **Triggers on any of the following**: <br>1. **Non-COSMIC Environment**: Wayland compositor does not support `wp_alpha_modifier_v1`.<br>2. **Low-Power CPU + Heavy Environment**: No hardware decoder AND CPU has **under 8 threads** on multi-monitor or 4K setups.<br>*(Also active when explicitly set to `"hard_cut"`)* | Prevents frame drops and stuttering on unsupported or constrained platforms.<br>Immediately tears down the old pipeline for instant flicker-free switch. |

> **💡 Manual Override**: Selecting "Crossfade (COSMIC only)" in `fluffy-settings` or executing `fluffy config --transition-mode crossfade` forces crossfading (requires COSMIC desktop environment with `wp_alpha_modifier_v1`).

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
