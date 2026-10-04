# 🖥️ Fluffy Desktop & systemd User Service Setup Guide

<p align="center">
  <a href="SYSTEMD.md">English</a> | <a href="SYSTEMD.ja.md">日本語</a> | <a href="PORTAL.md">📚 Documentation Portal</a>
</p>

Instructions for integrating Fluffy into your desktop environment with automatic login startup and crash recovery.

---

## Automated Desktop Installer

Run the included desktop integration script:

```bash
./scripts/install-desktop-integration.sh
```

This script:
1. Builds optimized release binaries (`fluffy` and `fluffy-settings`).
2. Installs binaries into `~/.local/bin/`.
3. Installs `fluffy.service` into `~/.config/systemd/user/` and reloads systemd.
4. Installs scalable application icons into `~/.local/share/icons/hicolor/scalable/apps/`.
5. Installs the desktop launcher into `~/.local/share/applications/com.github.fluffy.Fluffy.desktop`.

---

## Managing with systemd `--user`

```bash
# Enable autostart on user login and start immediately
systemctl --user enable --now fluffy.service

# Check daemon status
systemctl --user status fluffy.service

# Follow real-time log output
journalctl --user -u fluffy.service -f

# Stop daemon
systemctl --user stop fluffy.service

# Restart daemon
systemctl --user restart fluffy.service

# Disable autostart
systemctl --user disable fluffy.service
```

---

## Service Definition Key Properties

- `After=graphical-session.target`: Ensures Fluffy starts only after the graphical session and Wayland compositor are ready.
- `Restart=on-failure`: Recovers automatically if the process exits unexpectedly.
- `ExecStart=%h/.local/bin/fluffy daemon`: Uses absolute path relative to user home directory.
