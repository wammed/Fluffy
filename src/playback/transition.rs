use serde::{Deserialize, Serialize};

use crate::config::TransitionMode;

/// 診断用の環境パラメータ
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionEnvironment {
    /// Wayland コンポジターがアルファ制御プロトコルまたはサブサーフェスアルファをサポートしているか
    pub supports_alpha_modifier: bool,
    /// ハードウェアデコード支援（NVDEC または VA-API等）が利用可能か
    pub has_hw_accel: bool,
    /// 利用可能な論理CPUスレッド数
    pub cpu_threads: usize,
    /// 接続されているディスプレイの数
    pub output_count: usize,
    /// 全画面の総ピクセル数（例: 4K 1画面なら 3840*2160 = 8,294,400）
    pub total_pixels: u64,
}

impl Default for TransitionEnvironment {
    fn default() -> Self {
        Self {
            supports_alpha_modifier: true,
            has_hw_accel: true,
            cpu_threads: 8,
            output_count: 1,
            total_pixels: 1920 * 1080,
        }
    }
}

/// 環境診断結果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionCapability {
    /// クロスフェードの前提条件を満たしているか
    pub is_crossfade_ready: bool,
    /// 自動判別（Auto）時に選択される実効モード (Crossfade または HardCut)
    pub effective_mode_for_auto: TransitionMode,
    /// ユーザーフレンドリーな診断・理由メッセージ
    pub diagnostic_reason: String,
}

/// 閾値定数: ソフトウェアデコード時にクロスフェードを許容する最大総ピクセル (WQHD相当)
pub const MAX_PIXELS_FOR_SW_CROSSFADE: u64 = 2560 * 1440;

/// 閾値定数: 高性能CPUとみなす論理スレッド数の下限 (8スレッド以上)
pub const HIGH_PERF_CPU_MIN_THREADS: usize = 8;

/// 環境情報を受け取り、クロスフェードの可否および理由を評価する純粋関数
pub fn evaluate_transition_capability(env: &TransitionEnvironment) -> TransitionCapability {
    if !env.supports_alpha_modifier {
        return TransitionCapability {
            is_crossfade_ready: false,
            effective_mode_for_auto: TransitionMode::HardCut,
            diagnostic_reason:
                "COSMICコンポジタがアルファ透過度制御プロトコルに対応していません (ハードカット適用)"
                    .to_string(),
        };
    }

    if env.has_hw_accel {
        return TransitionCapability {
            is_crossfade_ready: true,
            effective_mode_for_auto: TransitionMode::Crossfade,
            diagnostic_reason: "COSMIC (cosmic-comp) のハードウェア透過支援 (wp_alpha_modifier_v1 / NVDEC) によるクロスフェード利用可能"
                .to_string(),
        };
    }

    // HWデコードが検出できない場合でも、8スレッド以上の高性能CPUであれば
    // 4Kやマルチ画面のソフトウェアデコード＋クロスフェードを十分な余力で処理可能
    if env.cpu_threads >= HIGH_PERF_CPU_MIN_THREADS {
        return TransitionCapability {
            is_crossfade_ready: true,
            effective_mode_for_auto: TransitionMode::Crossfade,
            diagnostic_reason: format!(
                "高性能マルチコアCPU ({}スレッド) のソフトウェア処理によるクロスフェード利用可能",
                env.cpu_threads
            ),
        };
    }

    // 省電力CPU環境で、かつ高負荷（4K以上またはマルチモニター）の場合は安全のためハードカットへフォールバック
    if env.output_count > 1 || env.total_pixels > MAX_PIXELS_FOR_SW_CROSSFADE {
        return TransitionCapability {
            is_crossfade_ready: false,
            effective_mode_for_auto: TransitionMode::HardCut,
            diagnostic_reason: format!(
                "HWデコード非対応かつ高負荷環境 ({}スレッド, {}画面 / 約{:.1}Mpx) のためリソース保護優先 (ハードカット適用)",
                env.cpu_threads,
                env.output_count,
                env.total_pixels as f64 / 1_000_000.0
            ),
        };
    }

    TransitionCapability {
        is_crossfade_ready: true,
        effective_mode_for_auto: TransitionMode::Crossfade,
        diagnostic_reason: "ソフトウェアデコードによるクロスフェード利用可能 (CPU負荷軽度)"
            .to_string(),
    }
}

/// 設定されたモードと環境診断結果から、実際に使用するモード（実効モード）を決定する
pub fn resolve_effective_transition(
    mode: TransitionMode,
    capability: &TransitionCapability,
) -> TransitionMode {
    match mode {
        TransitionMode::Auto => capability.effective_mode_for_auto,
        TransitionMode::Crossfade => TransitionMode::Crossfade,
        TransitionMode::HardCut => TransitionMode::HardCut,
    }
}

/// 実際のシステム環境をプローブ（検出）する
pub fn probe_system_transition_environment(
    supports_alpha: bool,
    output_count: usize,
    total_pixels: u64,
) -> TransitionEnvironment {
    let has_hw_accel = probe_gst_hw_accel();
    let cpu_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    TransitionEnvironment {
        supports_alpha_modifier: supports_alpha,
        has_hw_accel,
        cpu_threads,
        output_count,
        total_pixels,
    }
}

/// GStreamerのハードウェアデコード要素・プラグインが存在するかプローブ
pub fn probe_gst_hw_accel() -> bool {
    if gstreamer::init().is_err() {
        return false;
    }

    // 1. プラグイン自体の存在確認 (nvcodec, vaapi, etc.)
    let registry = gstreamer::Registry::get();
    let hw_plugins = ["nvcodec", "vaapi", "vulkan"];
    if hw_plugins
        .iter()
        .any(|&p| registry.find_plugin(p).is_some())
    {
        return true;
    }

    // 2. 代表的なハードウェア支援エレメントファクトリの存在確認
    let hw_factories = [
        // NVIDIA / CUDA
        "nvh264dec",
        "nvh265dec",
        "cudaconvert",
        "cudacompositor",
        "cudaupload",
        "cudadownload",
        "cudascale",
        // Vulkan Video
        "vulkanh264dec",
        "vulkanh265dec",
        "vulkanvp9dec",
        // Intel / AMD VA-API
        "vaapih264dec",
        "vaapih265dec",
        "vah264dec",
        "vah265dec",
        // V4L2 / MFX
        "v4l2h264dec",
        "mfxh264dec",
    ];
    hw_factories
        .iter()
        .any(|&name| gstreamer::ElementFactory::find(name).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_capability_full_hardware_ready() {
        let env = TransitionEnvironment {
            supports_alpha_modifier: true,
            has_hw_accel: true,
            cpu_threads: 8,
            output_count: 2,
            total_pixels: 3840 * 2160 * 2,
        };
        let cap = evaluate_transition_capability(&env);
        assert!(cap.is_crossfade_ready);
        assert_eq!(cap.effective_mode_for_auto, TransitionMode::Crossfade);
        assert!(cap.diagnostic_reason.contains("ハードウェア透過"));
    }

    #[test]
    fn test_evaluate_capability_compositor_unsupported_falls_back() {
        let env = TransitionEnvironment {
            supports_alpha_modifier: false,
            has_hw_accel: true,
            cpu_threads: 8,
            output_count: 1,
            total_pixels: 1920 * 1080,
        };
        let cap = evaluate_transition_capability(&env);
        assert!(!cap.is_crossfade_ready);
        assert_eq!(cap.effective_mode_for_auto, TransitionMode::HardCut);
        assert!(
            cap.diagnostic_reason
                .contains("アルファ透過度制御プロトコルに対応していません")
        );
    }

    #[test]
    fn test_evaluate_capability_sw_accel_low_res_allows_crossfade() {
        let env = TransitionEnvironment {
            supports_alpha_modifier: true,
            has_hw_accel: false,
            cpu_threads: 4,
            output_count: 1,
            total_pixels: 1920 * 1080,
        };
        let cap = evaluate_transition_capability(&env);
        assert!(cap.is_crossfade_ready);
        assert_eq!(cap.effective_mode_for_auto, TransitionMode::Crossfade);
        assert!(cap.diagnostic_reason.contains("ソフトウェアデコード"));
    }

    #[test]
    fn test_evaluate_capability_sw_accel_low_spec_falls_back_to_hardcut() {
        // Low-spec CPU (e.g. 4 threads) on 4K single display with software decode
        let env = TransitionEnvironment {
            supports_alpha_modifier: true,
            has_hw_accel: false,
            cpu_threads: 4,
            output_count: 1,
            total_pixels: 3840 * 2160,
        };
        let cap = evaluate_transition_capability(&env);
        assert!(!cap.is_crossfade_ready);
        assert_eq!(cap.effective_mode_for_auto, TransitionMode::HardCut);
        assert!(cap.diagnostic_reason.contains("高負荷環境"));

        // Multi-monitor with low-spec software decode
        let env_multi = TransitionEnvironment {
            supports_alpha_modifier: true,
            has_hw_accel: false,
            cpu_threads: 4,
            output_count: 2,
            total_pixels: 1920 * 1080 * 2,
        };
        let cap_multi = evaluate_transition_capability(&env_multi);
        assert!(!cap_multi.is_crossfade_ready);
        assert_eq!(cap_multi.effective_mode_for_auto, TransitionMode::HardCut);
    }

    #[test]
    fn test_evaluate_capability_sw_accel_high_perf_cpu_allows_crossfade() {
        // High-end multi-core CPU (e.g. 16 or 20 threads) can crossfade 4K/multi-monitor even with SW decode
        let env_high_perf = TransitionEnvironment {
            supports_alpha_modifier: true,
            has_hw_accel: false,
            cpu_threads: 20,
            output_count: 2,
            total_pixels: 3840 * 2160 * 2,
        };
        let cap = evaluate_transition_capability(&env_high_perf);
        assert!(cap.is_crossfade_ready);
        assert_eq!(cap.effective_mode_for_auto, TransitionMode::Crossfade);
        assert!(cap.diagnostic_reason.contains("高性能マルチコアCPU"));
    }

    #[test]
    fn test_resolve_effective_transition_respects_modes() {
        let cap_ready = TransitionCapability {
            is_crossfade_ready: true,
            effective_mode_for_auto: TransitionMode::Crossfade,
            diagnostic_reason: "Ready".to_string(),
        };

        let cap_unsupported = TransitionCapability {
            is_crossfade_ready: false,
            effective_mode_for_auto: TransitionMode::HardCut,
            diagnostic_reason: "Not ready".to_string(),
        };

        // Auto mode
        assert_eq!(
            resolve_effective_transition(TransitionMode::Auto, &cap_ready),
            TransitionMode::Crossfade
        );
        assert_eq!(
            resolve_effective_transition(TransitionMode::Auto, &cap_unsupported),
            TransitionMode::HardCut
        );

        // Explicit Crossfade mode (forced regardless of capability)
        assert_eq!(
            resolve_effective_transition(TransitionMode::Crossfade, &cap_ready),
            TransitionMode::Crossfade
        );
        assert_eq!(
            resolve_effective_transition(TransitionMode::Crossfade, &cap_unsupported),
            TransitionMode::Crossfade
        );

        // Explicit HardCut mode (forced regardless of capability)
        assert_eq!(
            resolve_effective_transition(TransitionMode::HardCut, &cap_ready),
            TransitionMode::HardCut
        );
        assert_eq!(
            resolve_effective_transition(TransitionMode::HardCut, &cap_unsupported),
            TransitionMode::HardCut
        );
    }

    #[test]
    fn test_probe_gst_hw_accel_callable() {
        // Verifies the function runs without panic
        let _ = probe_gst_hw_accel();
    }

    #[test]
    fn test_evaluate_capability_boundary_pixels() {
        // Exact threshold boundary: 2560 * 1440 for low-spec CPU
        let env_exact = TransitionEnvironment {
            supports_alpha_modifier: true,
            has_hw_accel: false,
            cpu_threads: 4,
            output_count: 1,
            total_pixels: MAX_PIXELS_FOR_SW_CROSSFADE,
        };
        let cap_exact = evaluate_transition_capability(&env_exact);
        assert!(cap_exact.is_crossfade_ready);
        assert_eq!(cap_exact.effective_mode_for_auto, TransitionMode::Crossfade);

        // Exceed threshold by 1 pixel: should fall back to HardCut for low-spec CPU
        let env_exceeded = TransitionEnvironment {
            supports_alpha_modifier: true,
            has_hw_accel: false,
            cpu_threads: 4,
            output_count: 1,
            total_pixels: MAX_PIXELS_FOR_SW_CROSSFADE + 1,
        };
        let cap_exceeded = evaluate_transition_capability(&env_exceeded);
        assert!(!cap_exceeded.is_crossfade_ready);
        assert_eq!(
            cap_exceeded.effective_mode_for_auto,
            TransitionMode::HardCut
        );
    }

    #[test]
    fn test_transition_environment_and_capability_serialization_roundtrip() {
        let env = TransitionEnvironment {
            supports_alpha_modifier: true,
            has_hw_accel: true,
            cpu_threads: 12,
            output_count: 3,
            total_pixels: 5760 * 1080,
        };
        let json = serde_json::to_string(&env).unwrap();
        let deserialized: TransitionEnvironment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, env);

        let cap = evaluate_transition_capability(&env);
        let cap_json = serde_json::to_string(&cap).unwrap();
        let cap_deserialized: TransitionCapability = serde_json::from_str(&cap_json).unwrap();
        assert_eq!(cap_deserialized, cap);
    }

    #[test]
    fn test_probe_system_transition_environment_construct() {
        let probed = probe_system_transition_environment(true, 2, 3840 * 2160);
        assert!(probed.supports_alpha_modifier);
        assert!(probed.cpu_threads >= 1);
        assert_eq!(probed.output_count, 2);
        assert_eq!(probed.total_pixels, 3840 * 2160);
    }
}
