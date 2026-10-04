# 💻 Fluffy CLI Command Reference

<p align="center">
  <a href="CLI.md">English</a> | <a href="CLI.ja.md">日本語</a> | <a href="PORTAL.md">📚 Documentation Portal</a>
</p>

Comprehensive reference and usage examples for the Fluffy command-line interface (`fluffy`).

---

## Syntax

```text
fluffy [COMMAND] [OPTIONS]
```

---

## Commands

| Command | Alias | Description |
| :--- | :--- | :--- |
| `daemon` | `run` | Start the resident background wallpaper daemon in foreground |
| `status` | - | Query daemon status and connected output states via IPC |
| `set-video <PATH>` | `set_video` | Change video wallpaper (validates, normalizes, and stores to cache) |
| `import <PATH>` | - | Validate and import video into storage without playing |
| `pause` | - | Pause video playback on specified (or all) outputs |
| `resume` | - | Resume paused video playback |
| `stop` | - | Stop playback and destroy overlay surfaces (restoring original wallpaper) |
| `reload` | - | Reload and re-apply current video wallpaper |
| `config` | - | View or modify startup and playback settings |
| `mark <LABEL>` | - | Record a benchmark/profiling marker event |
| `help` | `--help` | Show help message |

---

## Global Options

| Option | Argument | Description | Default |
| :--- | :--- | :--- | :--- |
| `--socket` | `<PATH>` | Unix domain socket path of the target daemon | `$XDG_RUNTIME_DIR/fluffy.sock` |

---

## Command Options

### `set-video` / `pause` / `resume` / `stop` / `reload`
- `--output <NAME>`: Target display name (e.g. `DP-1`, `DP-2`). Omit to target all active displays.
- `--generation <NUM>`: (set-video only) Monotonic generation number to prevent race conditions.
- `--timeout <SECS>`: (set-video only) IPC response timeout in seconds (default: `60`).

### `daemon`
- `--output <NAME>`: Bind exclusively to a specific display output.
- `--video <PATH>`: Start playback immediately upon launch with specified video path.

### `import`
- `--crossfade-ms <MS>`: Duration in milliseconds for head/tail seamless crossfading (uses config default if omitted).

### `config`
- `--restore-on-startup <BOOL>`: Automatically restore last wallpaper on daemon launch / login (`true` / `false`).
- `--autostart <BOOL>`: Enable/disable systemd `--user` daemon autostart on user login (`true` / `false`).
- `--pause-fullscreen <BOOL>`: Enable/disable auto-pause when fullscreen windows are active (`true` / `false`).
- `--loop-crossfade-ms <MS>`: Crossfade duration in milliseconds for seamless looping (`0` to disable).

---

## Practical Examples

```bash
# Set wallpaper on all connected monitors
fluffy set-video ~/Videos/ambient_city.mp4

# Change wallpaper on secondary monitor (DP-2) only
fluffy set-video ~/Videos/nature.mp4 --output DP-2

# Pause playback on primary monitor (DP-1)
fluffy pause --output DP-1

# Resume playback on primary monitor (DP-1)
fluffy resume --output DP-1

# Check current playback and transcoding status
fluffy status

# View current settings and saved wallpaper state
fluffy config

# Enable automatic wallpaper restoration on startup
fluffy config --restore-on-startup true

# Enable autostart via systemd user service
fluffy config --autostart true

# Enable automatic pause when a window is fullscreen
fluffy config --pause-fullscreen true

# Set loop crossfade duration to 1.0 second (1000 ms)
fluffy config --loop-crossfade-ms 1000

# Pre-import a video with 1.0 second seamless loop crossfade (without playing immediately)
fluffy import ~/Videos/loop_bg.mp4 --crossfade-ms 1000
```
