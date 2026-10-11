# ⚙️ Fluffy 設定・状態保存仕様 (XDG 準拠)

<p align="center">
  <a href="CONFIGURATION.md">English</a> | <a href="CONFIGURATION.ja.md">日本語</a> | <a href="PORTAL.ja.md">📚 ドキュメントポータル</a>
</p>

Fluffy は XDG Base Directory Specification に厳格に準拠して設定ファイルおよび再生状態を管理します。

---

## 設定ファイル (`config.json`)

- **配置場所**: `$XDG_CONFIG_HOME/fluffy/config.json`（既定: `~/.config/fluffy/config.json`）

### 項目一覧

```json
{
  "startup_and_wallpaper": {
    "restore_on_startup": false,
    "autostart_daemon": false,
    "pause_on_fullscreen": false,
    "loop_crossfade_ms": 0,
    "transition_mode": "auto",
    "switch_crossfade_ms": 1000
  }
}
```

| 設定キー | 型 | 既定値 | 説明 |
| :--- | :--- | :--- | :--- |
| `restore_on_startup` | boolean | `false` | デーモン起動時やユーザーログイン時に、前回各モニターに適用した動画壁紙を自動復元するかどうか（既定はステートレス起動）。 |
| `autostart_daemon` | boolean | `false` | systemd `--user` サービス (`fluffy.service`) によるログイン時自動起動の有効状態。 |
| `pause_on_fullscreen` | boolean | `false` | ゲームや動画などの全画面ウィンドウ表示を検知した際に動画再生を一時停止して GPU/CPU 負荷を削減するかどうか。 |
| `loop_crossfade_ms` | u32 | `0` | ループ時の末尾・先頭クロスフェード時間（ミリ秒、`0` で無効）。有効時は FFmpeg `xfade` フィルタにより末尾と先頭をブレンドした完全シームレス動画を生成。 |
| `transition_mode` | string | `"auto"` | 動画切り替え時の転換演出モード (`"auto"`, `"crossfade"`, `"hard_cut"`)。 |
| `switch_crossfade_ms` | u64 | `1000` | 動画切り替え時のクロスフェード時間（ミリ秒、既定は 1.0秒 = 1000ms）。 |

### 動画切り替えモードとハードウェア要件マトリクス

動画切り替え時の UX を高めるクロスフェード（新旧パイプラインの同時再生とアルファフェードアウト/フェードイン）は、環境の処理能力に応じて以下のルールで自動選択されます：

| 実行モード | 自動判別の条件（最低ハードウェア要件） | 動作挙動・負荷特性 |
| :--- | :--- | :--- |
| **ハードウェア支援クロスフェード**<br>(HW Crossfade) | **GPU / HWデコーダー支援あり**: <br>• GStreamer の `nvcodec`（NVIDIA CUDA/NVDEC）、`vaapi`（Intel/AMD）、`vulkan` プラグイン、または各種 HW デコーダーエレメント（`nvh264dec`, `cudaconvert`, `vulkanvp9dec`, `vaapih264dec` 等）が検出された環境。<br>• 画面数・解像度制限なし（4K、マルチモニター対応）。 | GPU の専用動画デコーダー ASIC (NVDEC / VA-API) と GStreamer alpha フィルタを活用。<br>CPU 使用率: 1〜3% 程度の超低負荷。<br>60fps の滑らかなフェード。 |
| **ソフトウェア処理クロスフェード (高性能CPU)**<br>(High-Spec SW Crossfade) | **HWデコーダーなし ＋ 高性能マルチコアCPU**: <br>• 論理CPUスレッド数が **8 スレッド以上**（例: Core i7/i9, Ryzen 7/9 などのマルチコアデスクトップCPU）。<br>• 画面数・解像度制限なし（4K、マルチモニター対応）。 | CPU コア数に十分な余裕があるため、ソフトウェアデコード（libav `avdec_h264`）で 2 本の動画を同時再生してもフレーム落ちやプチフリが発生しないと判定。<br>CPU 使用率: 切り替え時のみ一時的に 5〜15% 程度。 |
| **ソフトウェア処理クロスフェード (軽量画面)**<br>(Low-Load SW Crossfade) | **HWデコーダーなし ＋ 省電力CPU ＋ 単一軽量画面**: <br>• 論理CPUスレッド数が **8 スレッド未満**（4コア4スレッド等の省電力ノートPC/タブレット）。<br>• ディスプレイ数: **1 画面のみ**（`output_count == 1`）。<br>• 総解像度: **WQHD以下**（総ピクセル数 ≤ 2,560 × 1,440 = 約 3.68 Mpx、FHD 1920x1080 を含む）。 | 省電力 CPU でも単一画面かつ WQHD 以下であれば CPU ソフトウェアデコードで安全に 60fps クロスフェードが可能。<br>CPU 使用率: 切り替え時のみ一時的に 10〜25% 程度。 |
| **ハードカット（強制リソース保護）**<br>(HardCut Fallback) | **以下のいずれかに該当する場合**: <br>1. **コンポジター非対応**: Wayland コンポジターがアルファ透過度制御プロトコルに対応していない。<br>2. **省電力CPU ＋ 高負荷環境**: HWデコーダー支援がなく、かつ CPU が **8 スレッド未満** で、**マルチモニター（2画面以上）** または **4K以上（総解像度 > WQHD）** の環境。<br>*(※手動で「ハードカット」を設定した場合も含む)* | 性能不足の端末で 4K 動画を 2 本同時に CPU デコードすることによるコマ落ちやデスクトップ UI のもたつきを防止。<br>旧パイプラインを即座に破棄し、黒画面なしの通常切替を行う。 |

> **💡 手動設定での上書き**: 設定アプリ（`fluffy-settings`）または CLI で `fluffy config --transition-mode crossfade` を指定することで、上記のリソース保護ガードをバイパスして強制的にクロスフェードを有効化できます。

### CLI からの変更例
```bash
fluffy config --restore-on-startup true
fluffy config --autostart true
fluffy config --pause-fullscreen true
fluffy config --loop-crossfade-ms 1000
fluffy config --transition-mode auto       # 自動判別（既定）
fluffy config --transition-mode crossfade  # 強制クロスフェード
fluffy config --transition-mode hard_cut   # ハードカット（即時切替）
fluffy config --switch-crossfade-ms 1500   # 切り替え時間を1.5秒に設定
```

---

## 状態ファイル (`state.json`)

- **配置場所**: `$XDG_STATE_HOME/fluffy/state.json`（既定: `~/.local/state/fluffy/state.json`）

各ディスプレイ出力ごとに最後に適用された正規化済み動画のパスを記録します。これにより、デーモン起動時やモニターのホットプラグ時に再エンコードなしで瞬時に前回の壁紙を復元できます。

```json
{
  "outputs": {
    "DP-1": "/home/user/.local/share/fluffy/storage/videos/a1b2c3d4....mp4",
    "DP-2": "/home/user/.local/share/fluffy/storage/videos/e5f6g7h8....mp4"
  }
}
```
