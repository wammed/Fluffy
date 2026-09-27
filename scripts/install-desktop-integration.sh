#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
DESKTOP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"

echo "=== Fluffy Desktop & systemd Integration Installer ==="

# 1. Build release binaries
echo "[1/5] Building release binaries (daemon + GUI)..."
cargo build --release --features gui --manifest-path "${REPO_DIR}/Cargo.toml"

# 2. Install binaries to ~/.cargo/bin (or ~/.local/bin)
BIN_DEST="${HOME}/.cargo/bin"
if [[ ! -d "${BIN_DEST}" ]]; then
    BIN_DEST="${HOME}/.local/bin"
    mkdir -p "${BIN_DEST}"
fi
echo "[2/5] Installing binaries to ${BIN_DEST}..."
cp "${REPO_DIR}/target/release/fluffy" "${BIN_DEST}/"
cp "${REPO_DIR}/target/release/fluffy-settings" "${BIN_DEST}/"

# 3. Install systemd user service
echo "[3/5] Installing systemd --user service..."
mkdir -p "${SYSTEMD_USER_DIR}"
cp "${REPO_DIR}/data/systemd/fluffy.service" "${SYSTEMD_USER_DIR}/"
systemctl --user daemon-reload

# 4. Install Application Icon
echo "[4/5] Installing Application Icons..."
mkdir -p "${ICON_DIR}"
cp "${REPO_DIR}/images/fluffy-icon.svg" "${ICON_DIR}/fluffy-icon.svg"
cp "${REPO_DIR}/images/fluffy-icon.svg" "${ICON_DIR}/com.github.fluffy.Fluffy.svg"
cp "${REPO_DIR}/images/fluffy-icon.svg" "${ICON_DIR}/com.github.wammed.fluffy.settings.svg"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true
fi

# 5. Install Desktop Entry
echo "[5/5] Installing Desktop Entry..."
mkdir -p "${DESKTOP_DIR}"
cp "${REPO_DIR}/data/desktop/com.github.fluffy.Fluffy.desktop" "${DESKTOP_DIR}/"
ln -sf "${DESKTOP_DIR}/com.github.fluffy.Fluffy.desktop" "${DESKTOP_DIR}/com.github.wammed.fluffy.settings.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${DESKTOP_DIR}" 2>/dev/null || true
fi

echo "=== Installation Complete ==="
echo ""
echo "To enable and start the daemon now with systemd:"
echo "  systemctl --user enable --now fluffy.service"
echo ""
echo "To check daemon status and journal logs:"
echo "  systemctl --user status fluffy.service"
echo "  journalctl --user -u fluffy.service -f"
echo ""
echo "To launch Settings GUI:"
echo "  fluffy-settings"
