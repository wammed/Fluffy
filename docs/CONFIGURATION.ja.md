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
| `transition_mode` | string | `"auto"` | 動画切り替え時の転換演出モード (`"auto"`, `"crossfade"`, `"hard_cut"`)。※`"crossfade"` は COSMIC デスクトップ環境 (`cosmic-comp`) 専用です。 |
| `switch_crossfade_ms` | u64 | `1000` | 動画切り替え時のクロスフェード時間（ミリ秒、既定は 1.0秒 = 1000ms）。 |

### 動画切り替えモードとハードウェア要件マトリクス

動画切り替え時の UX を高めるクロスフェード（新旧パイプラインの同時再生と Wayland ハードウェア透過ブレンド）は、**COSMIC デスクトップ環境 (`cosmic-comp`)** が提供する `wp_alpha_modifier_v1` プロトコルを活用して GPU レベルで合成されます。
従来の CPU ソフトウェア透過（alpha フィルタ）と異なり、NVDEC / VA-API による GPU ゼロコピー・ハードウェアデコードを維持したまま、120Hz/60fps の滑らかなディゾルブを実現します。

| 実行モード | 自動判別の条件（最低ハードウェア要件） | 動作挙動・負荷特性 |
| :--- | :--- | :--- |
| **ハードウェア透過クロスフェード (COSMIC専用)**<br>(HW Alpha Crossfade) | **COSMIC環境 ＋ `wp_alpha_modifier_v1` 利用可能**: <br>• COSMIC デスクトップ (`cosmic-comp`) が Wayland アルファモディファイア拡張をエクスポートしている環境。<br>• GPU / HWデコーダー支援（NVDEC / VA-API）によるゼロコピー描画。<br>• 画面数・解像度制限なし（マルチモニター・4K / 120Hz 対応）。 | コンポジタ（`cosmic-comp`）の GPU パイプラインでサーフェス透過度を制御。<br>GStreamer 側での CPU 色空間変換やフレームコピーは完全ゼロ。<br>CPU 使用率: 1% 未満。120Hz/60fps の極めて滑らかなフェード。 |
| **ソフトウェア処理クロスフェード (高性能CPU)**<br>(High-Spec SW Crossfade) | **HWデコーダーなし ＋ 高性能マルチコアCPU（COSMIC環境）**: <br>• 論理CPUスレッド数が **8 スレッド以上**（Core i7/i9, Ryzen 7/9 等）。<br>• `wp_alpha_modifier_v1` によるコンポジタ透過合成。 | ソフトウェアデコードでも描画合成はコンポジタ GPU で行われるため、CPU 負荷はデコード処理のみ。<br>CPU 使用率: 切り替え時のみ一時的に 5〜15% 程度。 |
| **ソフトウェア処理クロスフェード (軽量画面)**<br>(Low-Load SW Crossfade) | **HWデコーダーなし ＋ 省電力CPU ＋ 単一軽量画面（COSMIC環境）**: <br>• 論理CPUスレッド数が **8 スレッド未満**。<br>• ディスプレイ数: **1 画面のみ**（`output_count == 1`）。<br>• 総解像度: **WQHD以下**（総ピクセル数 ≤ 2,560 × 1,440）。 | 省電力 CPU でもコンポジタ GPU 合成により、安全に 60fps クロスフェードが可能。<br>CPU 使用率: 切り替え時のみ一時的に 10〜20% 程度。 |
| **ハードカット（非COSMIC環境 / リソース保護）**<br>(HardCut Fallback) | **以下のいずれかに該当する場合**: <br>1. **非COSMIC環境**: Wayland コンポジタが `wp_alpha_modifier_v1` に非対応。<br>2. **省電力CPU ＋ 高負荷環境**: HWデコーダー支援がなく、かつ CPU が **8 スレッド未満** でマルチモニターまたは 4K の環境。<br>*(※手動で「ハードカット」を設定した場合も含む)* | 非対応環境やリソース逼迫環境でのコマ落ちや負荷増大を防止。<br>旧パイプラインを即座に破棄し、黒画面なしの瞬時切り替えを行う。 |

> **💡 手動設定での上書き**: 設定アプリ（`fluffy-settings`）または CLI で `fluffy config --transition-mode crossfade` を指定することで強制有効化できます（※クロスフェードの動作には COSMIC デスクトップ環境の `wp_alpha_modifier_v1` が必要です）。

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
