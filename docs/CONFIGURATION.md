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
    "loop_crossfade_ms": 0
  }
}
```

| Config Key | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `restore_on_startup` | boolean | `false` | When true, the daemon restores the last applied video wallpaper on each display upon launch or user login (default is stateless startup). |
| `autostart_daemon` | boolean | `false` | Tracks whether the systemd `--user` service (`fluffy.service`) is enabled to start automatically on login. |
| `pause_on_fullscreen` | boolean | `false` | When true, playback automatically pauses when a fullscreen window is detected to conserve CPU/GPU resources. |
| `loop_crossfade_ms` | u32 | `0` | Crossfade duration in milliseconds between video head and tail for seamless looping (`0` disables crossfading). When set, FFmpeg `xfade` filter blends loop boundaries into a seamless file. |

### CLI Management Examples
```bash
fluffy config --restore-on-startup true
fluffy config --autostart true
fluffy config --pause-fullscreen true
fluffy config --loop-crossfade-ms 1000
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
