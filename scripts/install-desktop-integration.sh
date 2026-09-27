#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
DESKTOP_DIR="${HOME}/.local/share/applications"

echo "=== Fluffy Desktop & systemd Integration Installer ==="

# 1. Build release binaries
echo "[1/4] Building release binaries (daemon + GUI)..."
cargo build --release --features gui --manifest-path "${REPO_DIR}/Cargo.toml"

# 2. Install binaries to ~/.cargo/bin (or ~/.local/bin)
BIN_DEST="${HOME}/.cargo/bin"
if [[ ! -d "${BIN_DEST}" ]]; then
    BIN_DEST="${HOME}/.local/bin"
    mkdir -p "${BIN_DEST}"
fi
echo "[2/4] Installing binaries to ${BIN_DEST}..."
cp "${REPO_DIR}/target/release/fluffy" "${BIN_DEST}/"
cp "${REPO_DIR}/target/release/fluffy-settings" "${BIN_DEST}/"

# 3. Install systemd user service
echo "[3/4] Installing systemd --user service..."
mkdir -p "${SYSTEMD_USER_DIR}"
cp "${REPO_DIR}/data/systemd/fluffy.service" "${SYSTEMD_USER_DIR}/"
systemctl --user daemon-reload

# 4. Install Desktop Entry
echo "[4/4] Installing Desktop Entry..."
mkdir -p "${DESKTOP_DIR}"
cp "${REPO_DIR}/data/desktop/com.github.fluffy.Fluffy.desktop" "${DESKTOP_DIR}/"

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
