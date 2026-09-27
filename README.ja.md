<div align="center">

# 🎬 Fluffy
### COSMIC Desktop / Wayland 向け軽量・非破壊・ハードウェアアクセラレーション動画壁紙マネージャー

![Banner](./images/fluffy-banner.svg)

[![Built with libcosmic](https://img.shields.io/badge/libcosmic-Pop!_OS_COSMIC-24C8D8?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/pop-os/libcosmic)
[![Rust](https://img.shields.io/badge/Rust-1.80+-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Wayland](https://img.shields.io/badge/Wayland-Native-5277C3?style=for-the-badge&logo=wayland&logoColor=white)](https://wayland.freedesktop.org/)
[![GStreamer](https://img.shields.io/badge/GStreamer-1.24+-E95420?style=for-the-badge&logo=gstreamer&logoColor=white)](https://gstreamer.freedesktop.org/)
[![Platform](https://img.shields.io/badge/Platform-Linux_(COSMIC_/_Wayland)-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://www.kernel.org/)
[![Vibe Coding](https://img.shields.io/badge/Built_with-AI_Vibe_Coding-8A2BE2?style=for-the-badge&logo=sparkles&logoColor=white)](#-このプロジェクトについて-ai-vibe-coding)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)

<p align="center">
  <strong>Wayland ネイティブ Layer-Shell × 黒画面なしプリロール切替 × NVDEC ハードウェア支援 × SHA-256 キャッシュ × 独立マルチモニター</strong><br>
  Pop!_OS COSMIC Desktop および Linux Wayland 環境向けに設計された、極めて軽量かつ安定した常駐型ライブ動画壁紙システム。
</p>

<p align="center">
  <a href="README.md">English</a> | <a href="README.ja.md">日本語</a> | <a href="docs/PORTAL.ja.md">📚 ドキュメントポータル</a> | <a href="docs/BENCHMARK_REPORT.ja.md">📊 性能ベンチマーク報告書</a>
</p>

</div>

---

## 🌟 特徴・ハイライト

- **非破壊オーバーレイ共存モデル (`Layer::Bottom`)**: ネイティブの壁紙デーモン `cosmic-bg` を停止・変更することなく、ドックやパネル、ウィンドウの下層となる `Layer::Bottom` に非破壊で描画。万一 Fluffy が終了または停止した場合も、瞬時に元の静止画壁紙へ復帰します。
- **黒画面フラッシュゼロのシームレス切替**: デュアルパイプライン・プリロール機構を採用。動画切り替え時に黒画面やちらつき、解像度再交渉が一切発生しません（実機検証済み）。
- **ハードウェア動画再生支援（優先利用方針）**: ハードウェアデコードを優先利用（実際のデコーダは GStreamer 環境およびドライバに依存）。本検証環境（NVIDIA RTX 3080 / CachyOS）にて NVDEC（`nvh264dec`）による低負荷再生を確認済み。非対応環境でもソフトウェアデコード（`avdec_h264`）へ安全にフォールバックします。
- **常駐デーモンと設定 GUI の完全分離**:
  - **常駐デーモン (`fluffy`)**: わずか **3.2 MB** のリリースバイナリサイズ、アイドル時 **40 MB RSS** の省メモリ設計。
  - **設定 GUI (`fluffy-settings`)**: `libcosmic` を採用したネイティブ GUI。Unix ドメインソケット経由でデーモンを非同期操作し、設定完了後はプロセスを即座に終了してリソースを解放します。
- **決定論的 4K 境界検証 & SHA-256 原子的ストレージ**: `ffprobe` によるコンテナ・4K 超過チェック、`ffmpeg` による H.264/30fps への正規化、SHA-256 ハッシュによる自動重複排除永続ストレージ（`~/.local/share/fluffy/storage/`）を完備。
- **動的ディスプレイ・ホットプラグ & ジオメトリ追従**: モニターの接続・切断および解像度・スケール変更を Wayland イベントで自動検知し、デーモン再起動なしで壁紙を自動追従。
- **systemd `--user` サービス完全対応**: `graphical-session.target` に統合され、絶対パス指定（`%h/.local/bin/fluffy`）によるセッション連動の自動起動やクラッシュ時の自動復旧 (`Restart=on-failure`) に対応。

---

## 📊 実機性能測定結果 (Performance Benchmarks)

実機環境（**CachyOS / Arch Linux**, **NVIDIA GeForce RTX 3080**, **COSMIC Desktop `cosmic-comp`**, デュアル 1440p ディスプレイ `DP-1` + `DP-2`）にて測定：

| テストシナリオ | CPU 使用率 (%) | RSS メモリ (MB) | GPU 3D 利用率 (%) | GPU ビデオデコーダ (NVDEC) |
| :--- | :--- | :--- | :--- | :--- |
| **Daemon Idle** (未再生・待機) | **0.2%** | **40.5 MB** | 33.2% | **0.0%** |
| **1080p30** (シングル出力: DP-1) | **10.2%** | **312.7 MB** | 31.4% | **9.9%** |
| **1080p30** (デュアル出力: DP-1 + DP-2) | **18.0%** | **514.3 MB** | 25.2% | **10.5%** |
| **1440p30** (デュアル出力: DP-1 + DP-2) | **27.4%** | **615.5 MB** | 33.0% | **26.6%** |
| **4K30** (デュアル出力: DP-1 + DP-2) | **43.9%** | **854.1 MB** | 32.1% | **50.6%** |

*詳細レポート: [docs/BENCHMARK_REPORT.ja.md](docs/BENCHMARK_REPORT.ja.md)*

---

## 🚀 クイックスタート

### 前提パッケージ

必要な GStreamer およびシステム依存関係をインストールします：

```bash
# Arch Linux / CachyOS / Manjaro
sudo pacman -S gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav ffmpeg
```

### ビルドと実行

```bash
# 1. リポジトリのクローン
git clone https://github.com/wammed/Fluffy.git
cd Fluffy

# 2. リリースバイナリのビルド
cargo build --release --features gui

# 3. バックグラウンドデーモンの起動
./target/release/fluffy daemon &

# 4. CLI から動画壁紙を設定
./target/release/fluffy set-video /path/to/wallpaper.mp4

# 5. または COSMIC ネイティブ設定 GUI を起動
./target/release/fluffy-settings
```

---

## 🖥️ デスクトップ & systemd ユーザーサービス導入

デスクトップ統合用の自動インストールスクリプトが用意されています：

```bash
./scripts/install-desktop-integration.sh
```

このスクリプトは以下を実行します：
1. 最適化済みリリースバイナリ (`fluffy` および `fluffy-settings`) のビルド。
2. バイナリを `~/.local/bin/` にインストール。
3. `fluffy.service` を `~/.config/systemd/user/` に配置しリロード。
4. アプリアイコンを `~/.local/share/icons/hicolor/scalable/apps/` に配置。
5. デスクトップエントリを `~/.local/share/applications/com.github.fluffy.Fluffy.desktop` に配置。

### systemd によるデーモン管理：

```bash
# ユーザーログイン時の自動起動と今すぐの起動を有効化
systemctl --user enable --now fluffy.service

# デーモンの稼働状態と journal ログの確認
systemctl --user status fluffy.service
journalctl --user -u fluffy.service -f

# 停止および再起動
systemctl --user stop fluffy.service
systemctl --user restart fluffy.service
```

---

## 💻 CLI コマンド仕様

```text
Fluffy Video Wallpaper Manager

USAGE:
    fluffy [COMMAND] [OPTIONS]

COMMANDS:
    daemon, run          常駐型壁紙デーモンを起動
    status               IPC 経由でデーモンおよび各ディスプレイの状態を取得
    set-video <PATH>     動画壁紙を変更（キャッシュへの検証・正規化を含む）
    import <PATH>        再生を行わず事前に動画を検証・キャッシュにインポート
    pause                動画再生を一時停止
    resume               動画再生を再開
    stop                 動画再生を停止（サーフェスを破棄し元の壁紙へ復帰）
    reload               現在の動画壁紙を再読み込み
    help, --help         ヘルプメッセージを表示

OPTIONS:
    --socket <PATH>      対象デーモンの Unix ソケットパス (既定: $XDG_RUNTIME_DIR/fluffy.sock)
    --output <NAME>      対象ディスプレイ名 (例: DP-1, DP-2 / 省略時は全画面)
    --generation <NUM>   レースコンディション防止用モノトニック世代番号
    --video <PATH>       (daemon 専用) 起動と同時に再生を開始する動画パス
```

### 使用例

```bash
# すべての接続モニターに壁紙を設定
fluffy set-video ~/Videos/cyberpunk_city.mp4

# サブモニター (DP-2) のみ壁紙を変更
fluffy set-video ~/Videos/nature.mp4 --output DP-2

# メインモニター (DP-1) の再生を一時停止
fluffy pause --output DP-1

# 全ディスプレイの現在の再生状態を確認
fluffy status
```

---

## 🎬 動画フォーマット適合規格とストレージ仕様

### 1. 適合する動画規格（即時登録・変換CPU負荷なし）
以下の規格をすべて満たす動画ファイルは、**再エンコード（トランスコード）処理をバイパス**し、瞬時にストレージへ登録されて即座に再生が始まります（変換CPU負荷やファンの急回転は一切発生しません）：

| 項目 | 適合規格仕様 | 備考 |
| :--- | :--- | :--- |
| **コンテナ形式** | MP4 (`.mp4`, `format_name` が `mp4` または `mov`) | 正真正銘の MP4 コンテナ（MKV や WebM 等は、映像が H.264 であっても MP4 へ正規化） |
| **ビデオコーデック** | H.264 / AVC (`h264`, `avc1`) | ハードウェア/ソフトウェア双方で最高効率 |
| **ピクセルフォーマット** | `yuv420p` | 最も広く再生互換性のある 8-bit YUV |
| **解像度** | 偶数幅 × 偶数高さ、かつ 4K 以下 (3840×2160) | 奇数ピクセルは Wayland/GStreamer で不具合要因となるため |
| **フレームレート** | 30 fps 以下 (23.976, 24, 25, 29.97, 30 fps 等) | 常駐再生での低消費電力・低発熱を担保 |
| **音声トラック** | 任意（自動で無音化ミュート再生） | 壁紙用途のため音声出力は抑制 |

### 2. 規格外動画の自動正規化（初回変換とスムーズな2回目以降）
上記規格外の動画（例: HEVC/H.265, AV1, VP9, 60fps超の高フレームレート, 奇数解像度, MKV, WebM 等）も幅広くサポートしています。
- **初回のバックグラウンド変換**: 初回指定時に、`JobManager` 配下のバックグラウンドワーカースレッドで自動的に Fluffy 標準規格（H.264/yuv420p/30fps）への正規化変換（トランスコード）が実行されます。
  - そのため、**初回のみ変換処理に時間がかかり、一時的に CPU 負荷（およびファンの回転）が大きくなります**。
  - **黒画面ゼロ保証**: 変換中もデーモンのメインスレッドおよび既存の再生パイプラインは動き続けるため、**現在の壁紙（前の動画またはOSのデスクトップ壁紙）が途切れることなく滑らかに表示され続け、黒バックで待たされることはありません**。
  - **競合保護 & 重複排除**: リクエスト受付時点で世代番号をモノトニックに確保し、古い変換の割り込みを防止。同一ハッシュに対する同時変換要求は自動で In-Flight 重複排除されます。
- **2回目以降のスムーズな即時再生**: 一度変換を終えた動画はストレージに保存されているため、次回以降はトランスコードが完全にスキップされ、**極めてスムーズかつ瞬時に再生**されます。

### 3. 永続ストレージディレクトリ (`~/.local/share/fluffy/storage`)
変換済みの動画および適合動画は、OSやキャッシュクリーナーによって不意にパージされる一時的な cache ディレクトリではなく、**ユーザーが明示的に削除するまで永続保存される専用ストレージ**に保管されます：
- **動画保存場所**: `$XDG_DATA_HOME/fluffy/storage/videos/`（既定: `~/.local/share/fluffy/storage/videos/<hash>.mp4`）
- **メタデータ保存場所**: `$XDG_DATA_HOME/fluffy/storage/metadata/`（既定: `~/.local/share/fluffy/storage/metadata/<hash>.json`）
- **完全な重複排除**: SHA-256 コンテンツハッシュにより、同一の動画は1つのみ保存され、ディスク容量を無駄に消費しません。

### 4. 変換中インジケーター表示
- **設定 GUI (`fluffy-settings`)**: 変換処理が走っている間、回転する歯車/矢印アニメーション（`⚙️ ↑`）と「動画を最適化・変換中...」のステータスパネルがリアルタイム表示されます。
- **CLI (`fluffy status`)**: バックグラウンド変換ジョブの進行状況が `Background Task: ⚙️ Optimizing / Transcoding video` として表示されます。

---

## 🏛️ アーキテクチャ概要


```text
ユーザー / 自動起動 / 設定 GUI / CLI
       │
       ▼ (Unix ドメインソケット IPC: $XDG_RUNTIME_DIR/fluffy.sock)
┌─────────────────────────────────────────────────────────────────┐
│ 壁紙デーモン (常駐プロセス: ~3.2 MB バイナリ, ~40 MB RSS)       │
│                                                                 │
│  ├─ 永続ストレージ・正規化サブシステム (~/.local/share/fluffy/storage/) │
│  │   ├─ ffprobe 検証 (4K 解像度境界チェック)                     │
│  │   ├─ ffmpeg トランスコード (H.264 / yuv420p / 30fps / 音声無) │
│  │   └─ SHA-256 原子的キャッシュ & 重複排除                     │
│  │                                                              │
│  ├─ Wayland Layer-Shell コントローラー                          │
│  │   ├─ Layer::Bottom 非破壊オーバーレイ                        │
│  │   ├─ 動的ホットプラグ検知 (接続・切断イベント処理)            │
│  │   └─ ARGB8888 初期ベースバッファ                             │
│  │                                                              │
│  └─ マルチ出力マネージャー                                      │
│      ├── ManagedOutput [DP-1] (LayerSurface + GstVideoPlayer)   │
│      └── ManagedOutput [DP-2] (LayerSurface + GstVideoPlayer)   │
│            └─ デュアルパイプライン・黒画面ゼロプリロール再生コア │
└─────────────────────────────────────────────────────────────────┘
```

詳細なアーキテクチャ設計およびプロトコル仕様は [docs/TECHNICAL_DESIGN.ja.md](docs/TECHNICAL_DESIGN.ja.md) をご覧ください。

---

## 📚 ドキュメントポータル

| ドキュメント | 概要 |
| :--- | :--- |
| **[ドキュメントポータル](docs/PORTAL.ja.md)** | Fluffy ドキュメント全体の総合インデックスとナビゲーション |
| **[技術仕様書 (Technical Design)](docs/TECHNICAL_DESIGN.ja.md)** | アーキテクチャ仕様、IPC プロトコル、エラー処理の詳細 |
| **[セッション引継ぎ書 (Session Handover)](docs/SESSION_HANDOVER.ja.md)** | 実装フェーズの進捗履歴、検証状況、実機テストマトリクス |
| **[性能ベンチマーク報告書 (Benchmark Report)](docs/BENCHMARK_REPORT.ja.md)** | 解像度別・マルチモニター環境での CPU/メモリ/GPU 実測データ |

---

## 🤖 このプロジェクトについて (AI Vibe Coding)

本プロジェクトは、先進的な **AI Vibe Coding** と Linux Wayland 実機環境における厳格な検証を組み合わせて構築されています。すべての機能は以下の 5 段階の検証レベルを経て実装されています：
`実装済み (Implemented)` ➔ `コンパイル完了 (Compiled)` ➔ `単体テスト完了 (Unit-tested)` ➔ `結合テスト完了 (Integration-tested)` ➔ `実機検証完了 (Real-hardware-tested)`.

---

## 📄 ライセンス

本ソフトウェアは [MIT License](LICENSE) のもとで公開されています。
