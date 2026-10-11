# Fluffy Performance Benchmark Report (Phase 7 & Phase 8)

<p align="center">
  <a href="BENCHMARK_REPORT.md">English</a> | <a href="BENCHMARK_REPORT.ja.md">日本語</a> | <a href="PORTAL.md">📚 Portal</a>
</p>

**Date:** 2026-09-27 (Phase 7 Steady-State) / 2026-10-11 (Phase 8 Transition Performance)  
**Environment:**
- **OS:** Linux (CachyOS / Arch Linux)
- **Compositor:** COSMIC Desktop (`cosmic-comp` on Wayland)
- **Displays:** Dual Monitors (`DP-1` 2560x1440, `DP-2` 2560x1440)
- **GPU:** NVIDIA GeForce RTX 3080 (Driver: 615.71.09)
- **Binary Target:** `fluffy` release (4.7M / 3.3M stripped)

---

## 1. Steady-State Loop Playback Summary (Phase 7)

Resource consumption characteristics during continuous long-term single video loop playback.

| Test Scenario | CPU Usage (%) | RSS Memory (MB) | GPU 3D Util (%) | GPU Video Decoder (NVDEC) (%) |
| :--- | :--- | :--- | :--- | :--- |
| **Daemon Idle** (0 videos, standby) | **0.2%** | **40.5 MB** | 33.2% | **0.0%** |
| **1080p30** (Single Output: DP-1) | **10.2%** | **312.7 MB** | 31.4% | **9.9%** |
| **1080p30** (Dual Output: DP-1 + DP-2) | **18.0%** | **514.3 MB** | 25.2% | **10.5%** |
| **1440p30** (Dual Output: DP-1 + DP-2) | **27.4%** | **615.5 MB** | 33.0% | **26.6%** |
| **4K30** (Dual Output: DP-1 + DP-2) | **43.9%** | **854.1 MB** | 32.1% | **50.6%** |

---

## 2. Transition Transient Load Verification (Phase 8)

Empirical telemetry during wallpaper video transitions (A ➔ B) over the **3.0-second hardware GPU crossfade transient state (where both outgoing and incoming video pipelines decode and display concurrently)**.  
Demonstrates the efficiency of the zero-copy Wayland `wp_alpha_modifier_v1` compositor blending architecture.

### Benchmark Results Table (Steady vs 3.0s Crossfade Transient vs Recovered)

| Resolution & Scenario | Execution Phase | CPU Usage (%) | Peak RSS Memory (MB) | GPU 3D Util (%) | Characteristics & Notes |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **1080p30 (Dual Screen)** | **Steady Playback** | 29.8% | 892.1 MB | 24.6% | 1 video looping on dual outputs |
| | **Crossfade Transient (3.0s)** | **29.9% (+0.1%)** | **927.7 MB (+35.6 MB)** | **41.9%** | **2 concurrent video decodes (Zero CPU spike)** |
| | **Post-Transition** (Recovered) | 30.2% | 918.8 MB | 26.4% | Outgoing pipeline destroyed, steady state restored |
| **4K30 (Dual Screen)** | **Steady Playback** | 31.5% | 2,111.9 MB | 26.9% | Dual 4K video looping |
| | **Crossfade Transient (3.0s)** | **33.7% (+2.2%)** | **3,011.5 MB (+899.6 MB)** | **55.5%** | **Dual 4K concurrent decodes (+2.2% CPU delta only)** |
| | **Post-Transition** (Recovered) | 35.5% | **2,048.4 MB (-963.1 MB)** | 32.3% | **Full buffer deallocation (Zero leak verified)** |

---

## 3. Observations & Architecture Verification

1. **Zero CPU Spike During Transitions (+0.1% to +2.2% CPU delta only)**:
   - Typical video wallpaper implementations experience massive CPU spikes (approaching 100%) during transitions due to CPU software color blending, ARGB conversions, or memory copies, causing stutter across the desktop environment.
   - Fluffy leverages the Wayland protocol extension **`wp_alpha_modifier_v1`**. Decoded hardware DMABuf video buffers are delivered directly to the compositor without CPU memory copying. Even during simultaneous dual 4K crossfading, **CPU usage increases by merely +2.2% (and only +0.1% for 1080p)**.
2. **Complete Offloading to Compositor GPU (3D) Shader**:
   - Alpha transparency composition is performed directly by `cosmic-comp`'s GPU render pass. The temporary rise in GPU 3D engine utilization during the transient window (24.6% ➔ 41.9% for 1080p, 26.9% ➔ 55.5% for 4K) provides direct empirical proof of full GPU shader offloading.
3. **Immediate and Full Transient Memory Recovery (Zero Memory Leak)**:
   - During dual 4K switching, running two concurrent decoding pipelines temporarily elevates resident memory by ~900 MB (2,111 MB ➔ 3,011 MB). Upon the 3.0-second fade completion, the outgoing pipeline is transitioned to `Null` and torn down, resulting in **immediate 100% reclamation down to 2,048 MB (lower than pre-transition baseline)**.
4. **Complete Elimination of Black Screens & Hard Cuts (Dual-Subsurface Synchronization)**:
   - GStreamer's `waylandsink` dynamically allocates both an outer `area_surface` (handling background margins and clear color) and an inner `video_surface`. By binding alpha modifiers to both subsurfaces simultaneously with an initial alpha of 0.0, the default black buffer fill committed by GStreamer is rendered invisible, achieving a seamless crossfade dissolve.
5. **120Hz/60fps Smoothstep (Cubic Easing) Pacing**:
   - Driven at an 8ms cadence (120Hz) with cubic Hermite smoothstep easing ($3x^2 - 2x^3$), brightness and perceived contrast transition smoothly without harsh perceptual jumps or stepping artifacts.
6. **Binary Footprint and Resident Daemon Architecture**:
   - The resident background daemon binary is only **4.7 MB** (**3.3 MB** stripped), strictly preserving a lightweight daemon architecture completely decoupled from the `libcosmic` GUI frontend.
7. **Initial Normalization vs Subsequent Playback**:
   - **Compliant Videos (MP4 Container + H.264/yuv420p/30fps)**: Transcoding is completely bypassed; files are registered to persistent storage and start playing in sub-0.1s with zero CPU spikes.
   - **Non-Compliant Videos (HEVC/60fps/MKV)**: Initial playback triggers asynchronous background ffmpeg normalization on a worker thread. Playback of the existing wallpaper continues uninterrupted with no black screen.
   - **Subsequent Playback**: Normalized videos in persistent storage (`~/.local/share/fluffy/storage`) are replayed instantly with zero transcoding delay or CPU spikes.
