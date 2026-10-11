#!/usr/bin/env bash
set -euo pipefail

PID=$(pgrep -f "fluffy daemon" | head -n1)
if [ -z "$PID" ]; then
    echo "Fluffy daemon not running!"
    exit 1
fi

echo "Fluffy daemon PID: $PID"

# Function to sample metrics over N seconds
sample_metrics() {
    local duration=$1
    local label=$2
    local cpu_sum=0
    local rss_max=0
    local gpu_sum=0
    local dec_sum=0
    local dec_max=0
    local count=0

    # High resolution 200ms sampling
    local iterations=$((duration * 5))
    for ((i=1; i<=iterations; i++)); do
        sleep 0.2
        if ! kill -0 "$PID" 2>/dev/null; then
            break
        fi
        local ps_out=$(ps -p "$PID" -o %cpu,rss --no-headers 2>/dev/null || echo "0 0")
        local cpu=$(echo "$ps_out" | awk '{print $1}')
        local rss_kb=$(echo "$ps_out" | awk '{print $2}')
        local rss_mb=$(awk "BEGIN {printf \"%.1f\", ${rss_kb}/1024}")

        local nv_out=$(nvidia-smi --query-gpu=utilization.gpu,utilization.decoder --format=csv,noheader,nounits 2>/dev/null || echo "0, 0")
        local gpu=$(echo "$nv_out" | awk -F',' '{print $1}' | tr -d ' ')
        local dec=$(echo "$nv_out" | awk -F',' '{print $2}' | tr -d ' ')

        cpu_sum=$(awk "BEGIN {print ${cpu_sum} + ${cpu}}")
        gpu_sum=$((gpu_sum + gpu))
        dec_sum=$((dec_sum + dec))
        if (( $(awk "BEGIN {print (${rss_mb} > ${rss_max})}") )); then
            rss_max=$rss_mb
        fi
        if [ "$dec" -gt "$dec_max" ]; then
            dec_max=$dec
        fi
        count=$((count + 1))
    done

    if [ "$count" -gt 0 ]; then
        local avg_cpu=$(awk "BEGIN {printf \"%.1f\", ${cpu_sum} / ${count}}")
        local avg_gpu=$(awk "BEGIN {printf \"%.1f\", ${gpu_sum} / ${count}}")
        local avg_dec=$(awk "BEGIN {printf \"%.1f\", ${dec_sum} / ${count}}")
        echo "[$label] Samples: $count | Avg CPU: ${avg_cpu}% | Peak RSS: ${rss_max}MB | Avg GPU: ${avg_gpu}% | NVDEC Avg: ${avg_dec}% (Peak: ${dec_max}%)"
    fi
}

echo "=== 1. Measuring Steady-state (1080p dual) ==="
fluffy set-video /home/susie/Videos/Fluffy-Loops/cyberpunk-night-city.1920x1080.mp4
sleep 3
sample_metrics 4 "1080p Steady-state"

echo "=== 2. Measuring Transition (1080p -> 1080p crossfade 3.0s) ==="
( sample_metrics 4 "1080p Transition" ) &
M_PID=$!
sleep 0.5
fluffy set-video /home/susie/Videos/Fluffy-Loops/14203802_1920_1080_30fps.mp4
wait $M_PID

sleep 2
sample_metrics 4 "1080p Post-transition (Recovered)"

echo "=== 3. Measuring Steady-state (4K dual) ==="
fluffy set-video /home/susie/Videos/Fluffy-Loops/neon-reflections.3840x2160.mp4
sleep 3
sample_metrics 4 "4K Steady-state"

echo "=== 4. Measuring Transition (4K -> 4K crossfade 3.0s) ==="
( sample_metrics 4 "4K Transition" ) &
M_PID=$!
sleep 0.5
fluffy set-video /home/susie/Videos/Fluffy-Loops/ocean-with-neon-glitch.3840x2160.mp4
wait $M_PID

sleep 2
sample_metrics 4 "4K Post-transition (Recovered)"
