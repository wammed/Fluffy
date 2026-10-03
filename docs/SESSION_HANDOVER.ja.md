# COSMIC 動画壁紙マネージャー --- セッション引継ぎ書 (Session Handover)

<p align="center">
  <a href="SESSION_HANDOVER.md">English</a> | <a href="SESSION_HANDOVER.ja.md">日本語</a> | <a href="PORTAL.ja.md">📚 ポータル</a>
</p>

**ステータス:** Phase 8 (リファクタリング、最適化、全画面自動一時停止) 完了 (実機検証済み) --- 全計画フェーズ完了。\
**最終更新日:** 2026-10-04\
**フェーズ進捗サマリー:**
- **Phase 1: Wayland / GStreamer PoC** --- **完了** (実機検証済み)
- **Phase 2: 再生コア (Playback Core)** --- **完了** (実機検証済み)
- **Phase 3: IPC & デーモン基盤** --- **完了** (実機検証済み)
- **Phase 4: キャッシュ / インポート / 正規化** --- **完了** (実機検証済み)
- **Phase 5: マルチモニター管理** --- **完了** (実機検証済み)
- **Phase 6: 設定 GUI (libcosmic)** --- **完了** (実機検証済み)
- **Phase 7: 堅牢化、systemd、性能測定** --- **完了** (実機検証済み)
- **Phase 8: 最適化、安全性向上、全画面検知** --- **完了** (実機検証済み)

------------------------------------------------------------------------

## 1. プロジェクト概要

COSMIC Desktop / Wayland 向けの軽量ループ動画壁紙マネージャー。

システムは意図的に以下の構成に分離されています：

``` text
ユーザー / 自動起動 / CLI / 設定 GUI
      │
      ▼
壁紙デーモン (常駐プロセス, 最小限の CPU/メモリ オーバーヘッド)
      │
      ├─ Wayland layer-shell (Layer::Bottom 非破壊オーバーレイ)
      │
      ├─ GStreamer 再生コア (デュアルパイプライン・黒画面ゼロ切替, EOS シークループ)
      │
      ├─ Unix ドメインソケット IPC ($XDG_RUNTIME_DIR/fluffy.sock)
      │
      ├─ 永続ストレージ・インポートサブシステム (~/.local/share/fluffy/storage/)
      │     ├─ ffprobe 4K 境界検証
      │     ├─ ffmpeg H.264/yuv420p/30fps 正規化
      │     └─ SHA-256 原子的ストレージ & 即時再利用
      │
      └─ マルチ出力マネージャー
            ├── ManagedOutput [DP-1] (LayerSurface + GstVideoPlayer + 世代番号)
            └── ManagedOutput [DP-2] (LayerSurface + GstVideoPlayer + 世代番号)
```

設定操作は、IPC 経由で接続する別プロセスの `libcosmic` GUI が担当し、設定完了後は即座に終了します。デーモンは独立して再生を継続します。

------------------------------------------------------------------------

## 2. 現在の状態と検証レベル (Verification Level)

プロジェクト方針に基づき、各項目は 5 段階の検証レベルで管理されています：
`実装済み (Implemented)` ➔ `コンパイル完了 (Compiled)` ➔ `単体テスト完了 (Unit-tested)` ➔ `結合テスト完了 (Integration-tested)` ➔ `実機検証完了 (Real-hardware-tested)`.

### 2.1 実機検証済み項目 (`Real-hardware-tested`)

**テスト環境:**
- OS: CachyOS / Arch Linux
- コンポジター: COSMIC Desktop (`cosmic-comp`, `wayland-1`, `XDG_CURRENT_DESKTOP=COSMIC`)
- GPU: NVIDIA GeForce RTX 3080 (ドライバー: 615.71.09)
- 検出ディスプレイ: `DP-1` (2560x1440), `DP-2` (2560x1440)
- スタック: Rust 1.98.1 (Edition 2024), GStreamer 1.28.7, `waylandsink`, `nvh264dec`, `ffmpeg` n9.0.2, `ffprobe`

#### Phase 1: Wayland / GStreamer PoC [完了]
- [x] **Wayland ディスプレイハンドルの受け渡し**: GStreamer の sink と Wayland 接続を `GstWaylandDisplayHandleContext` (`gst_wl_display_handle_context_new`) により安全にバインド。
- [x] **レイヤーサーフェスのマッピング**: 対象出力を覆う `Layer::Bottom` 上の `zwlr_layer_surface_v1` を検証。
- [x] **サブサーフェスの可視性確保**: 親レイヤーサーフェスに初期ベースバッファ (`Argb8888`) をアタッチし、GStreamer のサブサーフェスをコンポジターへ可視化。
- [x] **明示的レンダリング矩形設定**: 外部サーフェスバインドに必要な `overlay.set_render_rectangle(0, 0, w, h)` を設定。
- [x] **ハードウェアデコード**: ハードウェアデコードは優先利用方針であり、利用可否は GStreamer 環境に依存します。本検証環境（NVIDIA RTX 3080 / CachyOS / Driver 615.71 / GStreamer 1.28.7）において `nvh264dec` による自動ハードウェアアクセラレーション動作を実機確認済みです。非対応環境ではソフトウェアデコード（`avdec_h264`）がフォールバックとして機能します。

#### Phase 2: 再生コア (Playback Core) [完了]
- [x] **コードベースのモジュール化**: `src/error.rs`, `src/wayland/`, `src/playback/` にクリーンに分離。
- [x] **VideoPlayer トレイト & GstVideoPlayer**:
  - `play(video)`: 即時再生開始。
  - `pause()`: フレームが滑らかにフリーズ。
  - `resume()`: コマ落ちなくスムーズに再開。
  - `stop()`: パイプラインが `NULL` へクリーンに遷移。
- [x] **黒画面フラッシュゼロのシームレス切替**: デュアルパイプライン・プリロール構造を実証。動画 A から動画 B への切替時に、黒画面やちらつきが一切発生しないことを確認。
- [x] **シームレスなループ再生**: EOS バスイベント受信時にパイプラインやサーフェスを再生成せず `seek_simple(ZERO)` を発行（実機にて 17 サイクル以上の連続ループを確認）。

#### Phase 3: IPC & デーモン基盤 [完了]
- [x] **Unix ドメインソケット IPC**:
  - `$XDG_RUNTIME_DIR/fluffy.sock` (または `/run/user/1000/fluffy.sock`) にバインド。
  - 厳格な `0600` ファイル権限（所有者のみアクセス可能）。
  - デッドソケットの自動検出・クリーンアップと再バインド。
  - ノンブロッキングディスパッチ: Wayland/GStreamer イベントループをブロックしない統合処理。
- [x] **プロトコル実装**:
  - JSON Lines による Request/Response エンベローププロトコル。
  - リクエスト最大サイズ制限 (64 KB) を強制。
  - サブコマンド: `status`, `set-video`, `pause`, `resume`, `stop`, `reload`。
- [x] **完全な CLI スイート (`fluffy`)**:
  - `fluffy daemon`: 常駐バックグラウンドデーモン。
  - `fluffy status`: 各モニターの再生状態、動画パス、世代番号、ループ回数を照会。
  - `fluffy set-video <path>`: IPC 経由での壁紙切替。
  - `fluffy pause` / `resume` / `stop`: 再生制御。
- [x] **世代セマンティクス (Generation Semantics)**:
  - 古い世代番号のリクエスト拒否を検証（高速切替時のレースコンディション防止）。
- [x] **クリーンなシャットダウンとデスクトップ壁紙の復帰**:
  - SIGINT / SIGTERM 受信時にプレイヤーを停止、サーフェスを破棄、ソケットを削除し、`cosmic-bg` の静止画壁紙を即座に復旧。

#### Phase 4: キャッシュ / インポート / 正規化 [完了]
- [x] **`ffprobe` 動画メタデータ検証 (`src/cache/probe.rs`)**:
  - 解像度、fps、コーデック、再生時間、音声有無を取得。
  - コンテナ形式（`format_name`）を厳格に検査し、真の MP4 のみトランスコードをバイパス。
  - 4K 解像度制限ポリシー (`width <= 3840 && height <= 2160`) を強制。過大な 5K 入力を即座に拒否。
- [x] **`ffmpeg` 正規化 (`src/cache/normalize.rs`)**:
  - 標準プロファイルへのトランスコード: H.264 (`libx264`), `yuv420p`, 30 fps, 音声なし (`-an`), 字幕なし (`-sn`), 偶数寸法補正 (`normalize_even_dimensions`)。
  - シェル展開を排除した直接引数配列での安全なプロセス実行。
- [x] **原子的ストレージ管理 (`src/cache/manager.rs`)**:
  - 保存先: `~/.local/share/fluffy/storage/videos/<sha256>.mp4` および `metadata/<sha256>.json`。
  - 一時ファイルからの一意なアトミック `fs::rename` による不完全書き込み防止。
  - メタデータ失敗時の動画ロールバック（不完全キャッシュの残留防止）。
  - SHA-256 コンテンツハッシュによる自動重複排除と即時再利用 (10ms 未満)。
- [x] **デーモン & CLI 統合**:
  - `fluffy import <path>` による事前キャッシュ登録。
  - `set-video` 実行時の自動検証・正規化・キャッシュ適用。

#### Phase 5: マルチモニター管理 [完了]
- [x] **`OutputManager` アーキテクチャ (`src/daemon/output_manager.rs`)**:
  - 複数物理ディスプレイの同時管理 (`1 出力 = 1 レイヤーサーフェス = 1 GStreamer パイプライン`)。
  - 各ディスプレイの解像度・スケール (`WlOutput` の論理サイズ / モード) を個別取得し、動的解像度変更にも追従。
- [x] **実機でのデュアル同時再生 (`DP-1` + `DP-2`)**:
  - `DP-1` (2560x1440) と `DP-2` (2560x1440) で独立した `Layer::Bottom` サーフェスとデコーダパイプラインを並行駆動。
- [x] **全体制御と画面個別制御の両立**:
  - 全体: `fluffy set-video test.mp4` で両画面を同時にベストエフォート適用。
  - 個別: `fluffy set-video test2.mp4 --output DP-2` で DP-2 のみを切り替え、DP-1 はそのまま再生継続。
  - 個別一時停止/再開/停止の独立性を確認。
- [x] **マルチサーフェスのクリーンな破棄**:
  - 終了時に全サーフェスを破棄し、両モニターの静止画壁紙を即座に復帰。

#### Phase 6: 設定 GUI (libcosmic) [完了]
- [x] **独立クライアント構成 (`fluffy-settings`)**:
  - `--features gui` による専用バイナリターゲット (`src/bin/fluffy-settings.rs`)。
  - 常駐デーモン (`fluffy`) を GUI 依存関係から完全に隔離し軽量性を維持。
- [x] **動的デーモンステータス & ディスプレイ一覧取得**:
  - 起動時に IPC で稼働中のディスプレイ、動画 URI、世代、ループ数を取得。
  - 完全非同期 IPC（`iced::Task` / ワーカースレッド）により、GUI 操作時のフリーズを完全防止。
- [x] **対象ディスプレイ選択 UI**:
  - 「すべてのディスプレイ (全体)」または検出された各モニター個別の選択。
- [x] **ネイティブファイルピッカー (`rfd`) & キャッシュ連携**:
  - Wayland ネイティブのダイアログから動画を選択し、非同期バックグラウンドでキャッシュ検証・正規化。
- [x] **IPC 連携による即座の反映**:
  - 全画面適用、画面個別適用、再生/一時停止/停止ボタンの完全動作。

#### Phase 7: 堅牢化、systemd、性能測定 [完了]
- [x] **systemd `--user` サービスユニット (`data/systemd/fluffy.service`)**:
  - `ExecStart=%h/.local/bin/fluffy daemon` による確実なバイナリ起動。
  - `graphical-session.target` にバインド (`PartOf`, `After`, `Requisite`)。
  - 厳格なライフサイクル管理: `Restart=on-failure`, `RestartSec=3`, `TimeoutStopSec=5`。
  - 必要な Wayland / デスクトップ環境変数 (`WAYLAND_DISPLAY`, `XDG_CURRENT_DESKTOP` 等) の受け渡し設定。
- [x] **構造化ロギング & journald 統合 (`tracing` + `tracing-subscriber`)**:
  - `main`, `daemon`, `playback`, `cache`, `wayland` 各層のログを統一。
  - `RUST_LOG` による動的レベル制御、journald およびコンソールへのクリーンな出力。
- [x] **デスクトップエントリ (`data/desktop/com.github.fluffy.Fluffy.desktop`)**:
  - COSMIC アプリケーションライブラリに正式登録。
  - ワンクリック導入スクリプト (`scripts/install-desktop-integration.sh`) を完備。
- [x] **動的ディスプレイ・ホットプラグ (Hotplug & Fault Recovery)**:
  - SCTK `OutputHandler` のイベント (`new_output`, `update_output`, `output_destroyed`) を検知。
  - 解像度・スケール変更時の動的サーフェス/レンダー矩形更新。
  - モニター追加時に自動でサーフェスを生成し再生中の壁紙を適用。
  - モニター切断時に安全に停止・クリーンアップし、デーモンの稼働を維持。
- [x] **実機性能ベンチマークの実施 (Real-hardware-tested)**:
  - ベンチマークスイート (`scripts/benchmark.sh`) を実機実行：
    - **Daemon Idle**: 0.2% CPU, 40.5 MB RSS, 0.0% GPU Decoder.
    - **1080p30 (DP-1)**: 10.2% CPU, 312.7 MB RSS, 9.9% GPU Decoder.
    - **1080p30 (DP-1 + DP-2)**: 18.0% CPU, 514.3 MB RSS, 10.5% GPU Decoder.
    - **1440p30 (DP-1 + DP-2)**: 27.4% CPU, 615.5 MB RSS, 26.6% GPU Decoder.
    - **4K30 (DP-1 + DP-2)**: 43.9% CPU, 854.1 MB RSS, 50.6% GPU Decoder.
  - 詳細レポート: [`docs/BENCHMARK_REPORT.ja.md`](BENCHMARK_REPORT.ja.md)
- [x] **バイナリフットプリントの極小化**:
  - 常駐デーモンリリースバイナリ: **3.2 MB** (GUI 依存ゼロ)。
  - 設定 GUI リリースバイナリ: **30 MB**。

### 2.2 将来の拡張課題 (バックログ)
1. **クライアント側での `wp_viewporter` プロトコル直接バインド**: 現在は `waylandsink` および 4方向アンカーによるスケーリングで動作しており、クライアント直接バインドは将来のプロトコル拡張として管理。
2. **フラクショナルスケーリング**: COSMIC コンポジター側でのバッファ拡大縮小処理の微調整。
3. **ウルトラワイド / 混合アスペクト比クロッピングポリシー**。

------------------------------------------------------------------------

## 3. 外部レビューに伴う堅牢化成果物 (P1, P2, P3 完了)

### 3.1 P1 堅牢化: 信頼性向上とレースコンディション防止
- **コンテナ形式検証**: `ffprobe` で `format_name` を検査。真の MP4 コンテナ (`mp4`/`mov`) のみトランスコードをバイパスし、MKV や WebM 等は映像コーデックが H.264 であっても MP4 へ正規化。拡張子偽装ファイルを防止。
- **リクエスト受付時での世代番号モノトニック確保**: 世代番号（`generation`）を「動画変換完了時」ではなく「**set-video 要求受付時**」に即座に採番 (`self.generation += 1`)。変換中に新しい要求が到着した場合、古い変換結果は Stale として安全に破棄され、古い要求が後から勝ってしまう競合を完全排除。
- **systemd 絶対パス**: `fluffy.service` の ExecStart を `%h/.local/bin/fluffy daemon` に統一し、ユーザーのセッション環境 PATH に依存しない確実な起動を担保。

### 3.2 P2 堅牢化: 複数ジョブ管理と動的 OutputGeometry 追従
- **JobManager による 8 状態ライフサイクル管理**: 単一状態から `HashMap<JobId, TranscodeJob>` に刷新し、`Queued`, `Probing`, `Transcoding`, `Installing`, `Completed`, `Failed`, `Cancelled`, `Stale` の 8 状態を追跡。複数変換が同時に走ってもデーモン状態が狂わない設計を実現。
- **動的 OutputGeometry 追従**: `OutputManager` が `wl_output` の解像度・スケール・座標変更を検知し、サーフェスサイズおよび `waylandsink` のレンダー矩形を動的に再構成。

### 3.3 P3 堅牢化: 並行最適化とベストエフォートセマンティクス
- **In-Flight 重複排除**: 同一の SHA-256 コンテンツハッシュに対する同時・重複 `set-video` 要求を検出し、実行中ジョブを共有・再利用。冗長な `ffmpeg`/`ffprobe` プロセスの重複起動を防止。
- **全画面適用のベストエフォート (Best-Effort) 仕様**: 全画面壁紙適用時に一部のモニターでエラーが発生しても、成功したモニターは新動画へ遷移し、失敗したモニターは旧動画を維持。各画面の個別結果を `SetVideoResult` (`outputs: Vec<OutputApplyResult>`) で返却。
- **設定 GUI の完全非同期 IPC**: `fluffy-settings` の IPC 通信を `iced::Task` による完全非同期処理とし、デーモン高負荷時や変換中も GUI が絶対にフリーズしない応答性を実現。

### 3.4 ブランド意匠・法務ライセンス & IP監査適合
- **アプリアイコン意匠刷新 (`images/fluffy-icon.svg`)**: 公式 COSMIC ロゴ類似要素を排し、独自の流線型 "F" フレームおよびディスプレイ＋再生シンボルをマット質感で表現した新意匠へ刷新。
- **全システムアセット同期**: `data/icons/hicolor/scalable/apps/` (`fluffy-icon.svg`, `com.github.fluffy.Fluffy.svg`, `com.github.wammed.fluffy.settings.svg`)、設定 GUI 内蔵バイナリ (`include_bytes!`)、およびユーザー環境キャッシュ (`~/.local/share/icons/hicolor/`) を完全同期。
- **法務・ライセンス・知的財産 (IP) ドキュメント体系の確立 (`legal/`)**: アイコン意匠のプロヴェナンス記録（`legal/IP_COMPLIANCE.md` / `legal/IP_COMPLIANCE.ja.md`）、依存クレートおよび外部ランタイム（GStreamer / FFmpeg）のライセンス監査書（`legal/THIRD_PARTY_LICENSES.md` / `legal/THIRD_PARTY_LICENSES.ja.md`）、および 4 アプリ横断アイコン意匠履歴（`legal/ICON_DESIGN_HISTORY.md` / `legal/ICON_DESIGN_HISTORY.ja.md`）を `legal/` 配下に統合配備。また、`cargo-deny` 用設定ファイル `deny.toml` を導入。

------------------------------------------------------------------------

## 4. 確立された主要アーキテクチャ方針

### 4.1 非破壊オーバーレイモデル (`cosmic-bg` との共存)
- ネイティブの `cosmic-bg` は**決して停止・変更しません**。`Layer::Background` 上で待機（CPU 0%、GPU 0%）しています。
- Fluffy は `Layer::Bottom` に描画し、`cosmic-bg` を覆い隠しつつ、デスクトップアイコンやドック、ウィンドウの下層に位置します。
- プロセスのクラッシュ、終了、停止時には Fluffy のサーフェスが瞬時に unmap され、元の壁紙が即座に表示されます。

### 4.2 デュアルパイプライン・プリロール切替（実機検証済み）
- 単一パイプラインで URI を切り替えると、`PAUSED -> READY` への遷移時に `gstwaylandsink` が NULL バッファを描画し、黒画面がフラッシュします。
- Fluffy は同一の親サーフェスに対してセカンダリパイプラインを生成し、プリロール完了まで一時停止待機させた上で `PLAYING` に切り替え、旧パイプラインを破棄します。これにより 100% ちらつきのない切り替えを実現しています。

### 4.3 古いリクエストの拒否 (世代セマンティクス)
- リクエスト受付時点でモノトニックな世代番号を割り当て。
- 現在よりも古い世代番号を持つリクエストや非同期変換結果は破棄され、連続切り替え時の競合を完全に防ぎます。

### 4.4 決定論的メディア正規化と原子的ストレージ
- `ffprobe` によるコンテナおよび 4K 境界検証 (`width <= 3840 && height <= 2160`)。
- H.264 / `yuv420p` / 30fps / 音声なし / 偶数寸法への標準化。
- SHA-256 コンテンツアドレッシングにより、同一動画の冗長な変換をゼロに抑制。動画とメタデータのアトミック配置。

### 4.5 独立したマルチモニター並行管理とベストエフォート適用
- 物理モニターごとに個別のレイヤーサーフェスと GStreamer 再生パイプラインを割り当て (`1 出力 = 1 サーフェス = 1 パイプライン`)。
- 1 つのモニターに対する操作が、他のモニターの再生を阻害したり停止させたりすることはありません。全画面適用時はベストエフォート方式で各画面に適用されます。

------------------------------------------------------------------------

## 5. リポジトリ構成

``` text
Fluffy/
├── Cargo.toml                  (Features: default (daemon/cli), gui (libcosmic))
├── Cargo.lock
├── deny.toml                   (cargo-deny 依存ライセンス & セキュリティ監査設定)
├── build.rs
├── README.md                   (英語ルートドキュメント)
├── README.ja.md                (日本語ルートドキュメント)
├── legal/
│   ├── IP_COMPLIANCE.md        (アイコン意匠設計・独自性検証・IPデューデリジェンス記録: 英語)
│   ├── IP_COMPLIANCE.ja.md     (アイコン意匠設計・独自性検証・IPデューデリジェンス記録: 日本語)
│   ├── THIRD_PARTY_LICENSES.md (依存関係監査 & ランタイムライセンス記録: 英語)
│   ├── THIRD_PARTY_LICENSES.ja.md (依存関係監査 & ランタイムライセンス記録: 日本語)
│   ├── ICON_DESIGN_HISTORY.md  (4アプリ横断アイコン意匠履歴: 英語)
│   └── ICON_DESIGN_HISTORY.ja.md (4アプリ横断アイコン意匠履歴: 日本語)
├── images/
│   ├── fluffy-icon.svg         (アプリアイコン マスター SVG)
│   └── fluffy-banner.png       (プロジェクトヘッダーバナー PNG)
├── data/
│   ├── systemd/
│   │   └── fluffy.service      (systemd --user サービスユニット)
│   ├── desktop/
│   │   └── com.github.fluffy.Fluffy.desktop (COSMIC アプリケーションエントリ)
│   └── icons/
│       └── hicolor/scalable/apps/
│           ├── fluffy-icon.svg
│           ├── com.github.fluffy.Fluffy.svg
│           └── com.github.wammed.fluffy.settings.svg
├── scripts/
│   ├── install-desktop-integration.sh (自動インストーラー)
│   └── benchmark.sh            (実機性能ベンチマークスイート)
├── docs/
│   ├── PORTAL.md / PORTAL.ja.md
│   ├── BENCHMARK_REPORT.md / BENCHMARK_REPORT.ja.md
│   ├── TECHNICAL_DESIGN.md / TECHNICAL_DESIGN.ja.md
│   └── SESSION_HANDOVER.md / SESSION_HANDOVER.ja.md
├── src/
│   ├── lib.rs                  (ライブラリ基盤)
│   ├── main.rs                 (CLI コマンド & デーモンエントリー)
│   ├── bin/
│   │   └── fluffy-settings.rs  (libcosmic 設定 GUI バイナリ)
│   ├── error.rs                (統合エラー型 FluffyError)
│   ├── cache/                  (キャッシュ & 正規化サブシステム)
│   │   ├── mod.rs
│   │   ├── probe.rs            (ffprobe ラッパー & 4K バリデータ)
│   │   ├── normalize.rs        (ffmpeg トランスコーダ & 偶数寸法補正)
│   │   └── manager.rs          (CacheManager, 原子的ファイル書き込み)
│   ├── daemon/                 (デーモンコントローラー & マルチ出力管理)
│   │   ├── mod.rs
│   │   ├── controller.rs       (メインループ, ホットプラグ & IPC ディスパッチ)
│   │   └── output_manager.rs   (OutputManager & ManagedOutput コレクション)
│   ├── ipc/                    (Unix ドメインソケット IPC サブシステム)
│   │   ├── mod.rs
│   │   ├── protocol.rs         (JSON Lines エンベロープ & 検証)
│   │   ├── server.rs           (ノンブロッキング IpcServer & デッドソケット掃除)
│   │   └── client.rs           (タイムアウト保護付き IpcClient)
│   ├── playback/               (GStreamer 再生コア)
│   │   ├── mod.rs
│   │   ├── pipeline.rs         (PipelineHandle & Wayland コンテキストバインド)
│   │   ├── player.rs           (VideoPlayer トレイト & GstVideoPlayer)
│   │   └── state.rs            (PlaybackState)
│   └── wayland/                (Wayland クライアントコア)
│       ├── mod.rs
│       ├── connection.rs       (WaylandContext, 出力ホットプラグイベント)
│       └── layer_surface.rs    (Layer::Bottom 壁紙サーフェス)
├── test.mp4
└── test2.mp4
```

------------------------------------------------------------------------

## 5. テストマトリクス

### Layer Shell
- [x] `Layer::Bottom` サーフェスの出現 (実機検証済み)
- [x] 出力ジオメトリ（解像度）の完全被覆 (実機検証済み)
- [x] サーフェスのクリーンな消去と壁紙復元 (実機検証済み)
- [x] マルチモニター同時サーフェス (`DP-1` + `DP-2`) (実機検証済み)
- [x] 出力の切断・動的ホットプラグ処理 (実機検証済み)
- [x] 解像度・スケール変更時の動的 OutputGeometry 追従とサーフェス更新 (単体テスト & 実機検証済み)

### GStreamer 再生コア
- [x] ハードウェアデコーダ (`nvh264dec`) による H.264 再生 (実機検証済み)
- [x] パイプライン再生成を伴わない EOS でのシームレスループ (実機検証済み)
- [x] 一時停止 (Pause) および再開 (Resume) 状態遷移 (実機検証済み)
- [x] デュアルパイプラインによる黒画面ゼロ切替 (実機検証済み)
- [x] 停止 (Stop) & サーフェス破棄 (実機検証済み)
- [x] 独立デュアル出力パイプライン (`DP-1` + `DP-2`) (実機検証済み)

### IPC & デーモン
- [x] Unix ソケットのバインド & ノンブロッキングポーリング (実機検証済み)
- [x] 異常終了時のデッドソケット自動検出・削除 (実機検証済み)
- [x] JSON-RPC シリアライズ / デシリアライズ (単体テスト & 実機検証済み)
- [x] 不正 JSON および過大リクエストの拒否 (単体テスト済み)
- [x] `status` コマンド (実機検証済み)
- [x] `set-video` コマンド (全体 & 画面個別) (実機検証済み)
- [x] `pause` / `resume` / `stop` / `reload` コマンド (全体 & 画面個別) (実機検証済み)
- [x] 世代セマンティクスと古いリクエストの拒否 (単体テスト & 実機検証済み)
- [x] リクエスト受付時点での世代番号モノトニック確保 (単体テスト済み)
- [x] JobManager による 8 状態ライフサイクル管理 (単体テスト済み)
- [x] コンテンツハッシュによる In-Flight トランスコード重複排除 (単体テスト済み)
- [x] In-Flight 重複排除時の複数画面・Subscriber 個別適用保証 (`JobSubscriber`) (単体テスト済み)
- [x] 全量 SHA-256 計算のワーカー完全委託によるメインループ・Wayland イベントループの非ブロッキング保証 (単体テスト & 実機検証済み)
- [x] 全画面適用のベストエフォート結果報告 (`SetVideoResult`) (単体テスト済み)
- [x] タイムアウト保護付きクライアント通信 (結合テスト & 実機検証済み)

### キャッシュ & インポート
- [x] `ffprobe` ストリームおよびコーデック検査 (結合テスト & 実機検証済み)
- [x] コンテナ形式検証（MP4 バイパス vs MKV/WebM 正規化）(単体テスト済み)
- [x] 4K 超過解像度の即時拒否 (`5120x1440` 拒否) (単体テスト & 実機検証済み)
- [x] 奇数解像度の偶数寸法補正 (`normalize_even_dimensions`) (単体テスト済み)
- [x] `ffmpeg` による H.264/yuv420p/30fps/偶数寸法正規化 (結合テスト & 実機検証済み)
- [x] 一時ファイル rename による原子的書き込み (結合テスト & 実機検証済み)
- [x] キャッシュメタデータ追跡と SHA-256 再利用 (結合テスト & 実機検証済み)
- [x] メタデータ書き込み失敗時の動画ロールバック原子的保証 (単体テスト済み)
- [x] 変換失敗時の一時ファイルクリーンアップ (結合テスト済み)

### 設定 GUI (libcosmic)
- [x] フィーチャー分離された `fluffy-settings` バイナリ (`--features gui`) (実機検証済み)
- [x] 常駐デーモンのゼロオーバーヘッド分離 (バイナリ肥大化なし) (実機検証済み)
- [x] COSMIC UI テーマ & レイアウト (`libcosmic` Application) (実機検証済み)
- [x] 非同期デーモンステータス購読・照会 (実機検証済み)
- [x] `iced::Task` による完全非同期 IPC コマンド発行 (実機検証済み)
- [x] ネイティブ Wayland ファイルピッカー連携 (`rfd`) (実機検証済み)
- [x] ディスプレイ選択 (`すべてのディスプレイ`, `DP-1`, `DP-2`) (実機検証済み)
- [x] IPC トリガー (`set-video`, `pause`, `resume`, `stop`) (実機検証済み)
- [x] COSMIC セッション内での手動操作実機テスト (実機検証済み)

### 堅牢化 & デスクトップ統合 (Phase 7)
- [x] systemd `--user` サービスユニット (`data/systemd/fluffy.service`) (テスト済み)
- [x] `fluffy.service` 内の絶対パス `%h/.local/bin/fluffy` (テスト済み)
- [x] `tracing` + `tracing-subscriber` 構造化ロギング（デーモン、IPC、再生、キャッシュ、Wayland で journald 連携） (実機検証済み)
- [x] 統一された診断・エラーハンドリング（常駐デーモン内 `println!` 全廃、エラー伝播、GUI での障害分類表示） (単体テスト & 実機検証済み)
- [x] 動的ディスプレイホットプラグ (接続・切断イベント処理) (実機検証済み)
- [x] 引数順序に依存しない堅牢な CLI パーサー (単体テスト & 実機検証済み)
- [x] 実機性能ベンチマークスイート (`scripts/benchmark.sh`) (実機検証済み)
- [x] 自動インストーラー (`scripts/install-desktop-integration.sh`) (テスト済み)

### 最適化、リファクタリング & 全画面検知 (Phase 8)
- [x] 全画面表示ウィンドウ検知による動画自動一時停止・再開 (`pause_on_fullscreen`) (実機検証済み)
- [x] COSMIC Desktop ネイティブプロトコル (`zcosmic_toplevel_info_v1`) 自動検知・バインド (実機検証済み)
- [x] wlroots 共通プロトコル (`zwlr_foreign_toplevel_manager_v1`) 自動検知・フォールバック (テスト済み)
- [x] 設定 GUI (`fluffy-settings`) 上でのコンポジター対応状況のリアルタイム表示 (`🟢 コンポジター対応` / `⚠️ コンポジター非対応`) (実機検証済み)
- [x] 非対応コンポジター接続時のトグルスイッチ無効化 (実機検証済み)
- [x] 稼働状態適応型イベントループスリープ（変換中 5ms / 再生中 16ms / アイドル 50ms） (単体・実機検証済み)
- [x] 先行トランスコードジョブの即時アトミックキャンセル (`cancel_token` + `child.kill()`) (単体テスト & 実機検証済み)
- [x] 適合動画インポート時の hardlink 高速化 (単体テスト済み)
- [x] `SlotPool` 解像度追従型動的バッファ容量計算によるオーバーフロー防止 (実機検証済み)
- [x] GStreamer Wayland ディスプレイハンドルの NULL ポインタ安全性検証 (単体テスト済み)
- [x] 不安全な `libc` FFI から safe Rust `rustix` への置換 (単体テスト済み)
- [x] 宣言型 `clap` derive による CLI パーサーの刷新 (単体テスト & 実機検証済み)

### サーフェススケーリング & Wayland プロトコル (実装状態)
- [x] レイヤーサーフェス 4 方向アンカーおよび waylandsink レンダー矩形によるスケーリング (実機検証済み)
- [ ] クライアント側での `wp_viewporter` プロトコル直接バインド (未実装 / バックログ)
- [ ] ウルトラワイド / 混合アスペクト比クロッピングポリシー (未実装 / バックログ)

------------------------------------------------------------------------

## 6. プロジェクト総括

1. **現在の状態:**
   全 7 フェーズの計画内容が **すべて完了** し、NVIDIA RTX 3080 ＋ COSMIC Desktop 実機環境にて動作確認済みです。
2. **主要成果物:**
   - 常駐壁紙デーモン: `target/release/fluffy` (3.2 MB)
   - COSMIC 設定 GUI: `target/release/fluffy-settings` (30 MB)
   - systemd サービスユニット: `data/systemd/fluffy.service`
   - デスクトップエントリ: `data/desktop/com.github.fluffy.Fluffy.desktop`
   - インストールスクリプト: `scripts/install-desktop-integration.sh`
   - ベンチマークスイート: `scripts/benchmark.sh` & `docs/BENCHMARK_REPORT.ja.md`
3. **遵守された設計原則:**
   - GUI はステートを持たない一時的な IPC クライアントであり、レイヤーサーフェスや GStreamer パイプラインを直接所有しません。
   - `cosmic-bg` と安全に共存する非破壊オーバーレイ構造 (`Layer::Bottom`) を徹底して維持しています。
