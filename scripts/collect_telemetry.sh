#!/usr/bin/env bash
# Telemetry collector for Fluffy benchmark (100ms high-resolution sampling)
set -euo pipefail

PID="${1:-}"
OUT_DIR="${2:-}"

if [ -z "$PID" ] || [ -z "$OUT_DIR" ]; then
    echo "Usage: $0 <FLUFFY_PID> <OUTPUT_DIRECTORY>"
    exit 1
fi

mkdir -p "$OUT_DIR"

echo "=== Starting high-resolution telemetry collectors for PID $PID ==="
echo "Output directory: $OUT_DIR"

# 1. GPU Telemetry via nvidia-smi (100ms sampling)
if command -v nvidia-smi >/dev/null 2>&1; then
    nvidia-smi --query-gpu=timestamp,utilization.gpu,utilization.memory,utilization.decoder,memory.used,memory.free,temperature.gpu,power.draw \
        --format=csv -lms 100 > "$OUT_DIR/gpu_telemetry.csv" 2>/dev/null &
    NVIDIA_PID=$!
    echo "Started nvidia-smi collector (PID $NVIDIA_PID)"
else
    NVIDIA_PID=""
    echo "nvidia-smi not found; skipping GPU telemetry"
fi

# 2. Process CPU/Memory Telemetry via /proc (100ms sampling)
cleanup() {
    echo "Stopping telemetry collectors..."
    if [ -n "$NVIDIA_PID" ] && kill -0 "$NVIDIA_PID" 2>/dev/null; then
        kill "$NVIDIA_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

echo "timestamp,epoch_ms,utime_ticks,stime_ticks,rss_pages,threads" > "$OUT_DIR/proc_telemetry.csv"

# Record system page size and tick rate
CLK_TCK=$(getconf CLK_TCK 2>/dev/null || echo 100)
PAGE_SIZE=$(getconf PAGESIZE 2>/dev/null || echo 4096)
echo "CLK_TCK=$CLK_TCK,PAGE_SIZE=$PAGE_SIZE" > "$OUT_DIR/telemetry_meta.txt"

while kill -0 "$PID" 2>/dev/null; do
    NOW_EPOCH=$(date +%s%3N)
    ISO_TS=$(date --iso-8601=seconds 2>/dev/null || date +"%Y-%m-%dT%H:%M:%S%z")
    if [ -f "/proc/$PID/stat" ]; then
        STAT_CONTENT=$(cat "/proc/$PID/stat" 2>/dev/null || true)
        if [ -n "$STAT_CONTENT" ]; then
            # Extract fields: utime(14), stime(15), num_threads(20), rss(24)
            read -r _ _ _ _ _ _ _ _ _ _ _ _ _ utime stime _ _ _ _ threads _ _ _ rss _ <<< "$STAT_CONTENT"
            echo "$ISO_TS,$NOW_EPOCH,$utime,$stime,$rss,$threads" >> "$OUT_DIR/proc_telemetry.csv"
        fi
    fi
    sleep 0.1
done

echo "Target process $PID terminated. Telemetry collection complete."
