#!/usr/bin/env bash
# Integrated Benchmark & Telemetry Runner for Fluffy Video Wallpaper Switch Latency
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

SESSION_TAG=$(date +"%Y%m%d_%H%M%S")
BENCH_DIR="${REPO_DIR}/target/benchmarks/${SESSION_TAG}"
mkdir -p "${BENCH_DIR}"

echo "========================================================"
echo " Fluffy Integrated Switch Latency Benchmark Suite"
echo " Session Directory: ${BENCH_DIR}"
echo "========================================================"

# 1. Environment & Dependency Check
echo "[1/6] Checking Fluffy daemon and environment..."

SWITCH_COUNT="${1:-8}"
VIDEO_A="${2:-${REPO_DIR}/test.mp4}"
VIDEO_B="${3:-${REPO_DIR}/test2.mp4}"

# Prevent output directory collisions
if [[ -d "${BENCH_DIR}" ]]; then
    BENCH_DIR="${BENCH_DIR}_${RANDOM}"
    mkdir -p "${BENCH_DIR}"
fi

# Prefer release binary for accurate performance benchmarking
if [[ -n "${FLUFFY_BIN_OVERRIDE:-}" ]] && [[ -x "${FLUFFY_BIN_OVERRIDE}" ]]; then
    FLUFFY_BIN="${FLUFFY_BIN_OVERRIDE}"
elif [[ -x "${REPO_DIR}/target/release/fluffy" ]]; then
    FLUFFY_BIN="${REPO_DIR}/target/release/fluffy"
elif [[ -x "${REPO_DIR}/target/debug/fluffy" ]]; then
    FLUFFY_BIN="${REPO_DIR}/target/debug/fluffy"
else
    echo "  Building Fluffy release binary..."
    cargo build --release --manifest-path "${REPO_DIR}/Cargo.toml"
    FLUFFY_BIN="${REPO_DIR}/target/release/fluffy"
fi

echo "  Target Binary: ${FLUFFY_BIN}"
echo "  Switch Count:  ${SWITCH_COUNT}"
echo "  Video A:       ${VIDEO_A}"
echo "  Video B:       ${VIDEO_B}"

if [[ ! -f "${VIDEO_A}" ]] || [[ ! -f "${VIDEO_B}" ]]; then
    echo "ERROR: Test videos not found at ${VIDEO_A} or ${VIDEO_B}."
    echo "Please specify valid video files as arguments: $0 [SWITCH_COUNT] [VIDEO_A] [VIDEO_B]"
    exit 1
fi

# Detect running daemon PID or start temporary daemon
FLUFFY_PID=$(pgrep -f "fluffy daemon" | head -n1 || true)
DAEMON_MANAGED=false

if [[ -z "${FLUFFY_PID}" ]]; then
    echo "  Starting temporary Fluffy daemon with test video..."
    RUST_LOG=info "${FLUFFY_BIN}" daemon --video "${VIDEO_A}" > "${BENCH_DIR}/fluffy_daemon.log" 2>&1 &
    FLUFFY_PID=$!
    DAEMON_MANAGED=true
    sleep 2
    if ! kill -0 "${FLUFFY_PID}" 2>/dev/null; then
        echo "ERROR: Fluffy daemon failed to start. Check ${BENCH_DIR}/fluffy_daemon.log"
        exit 1
    fi
    echo "  Started daemon (PID ${FLUFFY_PID})"
else
    echo "  Connected to running daemon (PID ${FLUFFY_PID})"
fi

# 2. Record Environment Metadata
echo "[2/6] Recording environment metadata..."
cat <<EOF > "${BENCH_DIR}/env_metadata.txt"
SESSION_TAG=${SESSION_TAG}
KERNEL=$(uname -r)
ARCH=$(uname -m)
GPU=$(nvidia-smi --query-gpu=name,driver_version --format=csv,noheader 2>/dev/null || echo "No NVIDIA GPU")
WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-unknown}
FLUFFY_BIN=${FLUFFY_BIN}
FLUFFY_BIN_SHA256=$(sha256sum "${FLUFFY_BIN}" | awk '{print $1}')
FLUFFY_PID=${FLUFFY_PID}
DAEMON_MANAGED=${DAEMON_MANAGED}
SWITCH_COUNT=${SWITCH_COUNT}
VIDEO_A=${VIDEO_A}
VIDEO_B=${VIDEO_B}
START_EPOCH_MS=$(date +%s%3N)
EOF

# 3. Start High-Resolution Telemetry (100ms)
echo "[3/6] Starting high-resolution CPU/GPU telemetry collectors..."
"${SCRIPT_DIR}/collect_telemetry.sh" "${FLUFFY_PID}" "${BENCH_DIR}/telemetry" &
TELEMETRY_PID=$!

# Ensure cleanup on interrupt
cleanup() {
    echo "Cleaning up benchmark session..."
    if kill -0 "${TELEMETRY_PID}" 2>/dev/null; then
        kill "${TELEMETRY_PID}" 2>/dev/null || true
    fi
    if [[ "${DAEMON_MANAGED}" == "true" ]] && kill -0 "${FLUFFY_PID}" 2>/dev/null; then
        kill "${FLUFFY_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# 4. Execute Standardized Switch Sequences
echo "[4/6] Executing video switch test sequence (${SWITCH_COUNT} switches)..."
TEST_VIDEOS=("${VIDEO_A}" "${VIDEO_B}")
FAILED_SWITCHES=0

for i in $(seq 1 "${SWITCH_COUNT}"); do
    target_idx=$(( (i - 1) % 2 ))
    target_video="${TEST_VIDEOS[$target_idx]}"
    echo "  [Switch $i/${SWITCH_COUNT}] Switching to $(basename "$target_video")..."
    if ! "${FLUFFY_BIN}" set-video "$target_video" >/dev/null 2>"${BENCH_DIR}/set_video_last.err"; then
        echo "    WARNING: set-video command returned non-zero exit status on switch $i"
        FAILED_SWITCHES=$((FAILED_SWITCHES + 1))
    fi
    sleep 1.0 # Allow post-switch stabilization
done

# 5. Stop Telemetry
echo "[5/6] Finalizing telemetry and logs..."
sleep 0.5
if kill -0 "${TELEMETRY_PID}" 2>/dev/null; then
    kill "${TELEMETRY_PID}" 2>/dev/null || true
fi
wait "${TELEMETRY_PID}" 2>/dev/null || true

# If daemon was started by us, capture and stop it
if [[ "${DAEMON_MANAGED}" == "true" ]]; then
    kill "${FLUFFY_PID}" 2>/dev/null || true
    wait "${FLUFFY_PID}" 2>/dev/null || true
    LOG_SOURCE="${BENCH_DIR}/fluffy_daemon.log"
else
    # Copy systemd journal if running as user service
    LOG_SOURCE="${BENCH_DIR}/journal.log"
    journalctl --user -u fluffy.service --since "-2m" -o cat > "${LOG_SOURCE}" 2>/dev/null || true
fi

# 6. Aggregate and Analyze
echo "[6/6] Analyzing switch latency stages..."
SUMMARY_CSV="${BENCH_DIR}/switch_summary.csv"
if [[ -f "${LOG_SOURCE}" ]] && [[ -s "${LOG_SOURCE}" ]]; then
    if python3 "${SCRIPT_DIR}/analyze_switch_bench.py" "${LOG_SOURCE}" "${SUMMARY_CSV}"; then
        echo "Analysis completed successfully."
    else
        echo "WARNING: analyze_switch_bench.py encountered warnings or errors."
    fi
else
    echo "ERROR: Log source is missing or empty (${LOG_SOURCE}). Cannot analyze switches."
    exit 1
fi

echo "========================================================"
echo " Benchmark Complete!"
echo " Results Directory: ${BENCH_DIR}"
echo " Summary CSV:       ${SUMMARY_CSV}"
echo " Telemetry:         ${BENCH_DIR}/telemetry/"
echo " Failed Switches:   ${FAILED_SWITCHES}/${SWITCH_COUNT}"
echo "========================================================"
