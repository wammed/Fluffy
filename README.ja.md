<div align="center">

<img src="./images/fluffy-icon.svg" width="96" height="96" alt="Fluffy Icon" />

# 🎬 Fluffy
### COSMIC Desktop / Wayland 向け軽量・非破壊・ハードウェアアクセラレーション動画壁紙マネージャー

![Banner](./images/fluffy-banner.png)

[![Built with libcosmic](https://img.shields.io/badge/libcosmic-Pop!_OS_COSMIC-24C8D8?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/pop-os/libcosmic)
[![Rust](https://img.shields.io/badge/Rust-1.80+-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Wayland](https://img.shields.io/badge/Wayland-Native-5277C3?style=for-the-badge&logo=wayland&logoColor=white)](https://wayland.freedesktop.org/)
[![GStreamer](https://img.shields.io/badge/GStreamer-1.28+-E95420?style=for-the-badge&logo=gstreamer&logoColor=white)](https://gstreamer.freedesktop.org/)
[![Platform](https://img.shields.io/badge/Platform-Linux_(COSMIC_/_Wayland)-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://www.kernel.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)](LICENSE)

<p align="center">
  <strong>Wayland ネイティブ Layer-Shell × 黒画面なしプリロール切替 × NVDEC ハードウェア支援 × SHA-256 キャッシュ × 独立マルチモニター</strong><br>
  Pop!_OS COSMIC Desktop および Linux Wayland 環境向けに設計された、極めて軽量かつ安定した常駐型ライブ動画壁紙システム。
</p>

<p align="center">
  <a href="README.md">English</a> | <a href="README.ja.md">日本語</a> | <a href="docs/PORTAL.ja.md">📚 ドキュメントポータル</a> | <a href="docs/BENCHMARK_REPORT.ja.md">📊 性能ベンチマーク報告書</a> | <a href="legal/IP_COMPLIANCE.ja.md">🎨 アイコン設計 & IP監査</a>
</p>

</div>

---

## 🚀 クイックスタート

### 1. 前提パッケージの導入

```bash
# Arch Linux / CachyOS / Manjaro
sudo pacman -S gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav ffmpeg
```

### 2. ビルドと実行

```bash
# クローンとリリースビルド
git clone https://github.com/wammed/Fluffy.git
cd Fluffy
cargo build --release --features gui

# デーモンの起動
./target/release/fluffy daemon &

# (推奨) シームレスループ設定（先頭・末尾 1秒クロスフェード）
./target/release/fluffy config --loop-crossfade-ms 1000

# 動画壁紙の適用
./target/release/fluffy set-video /path/to/wallpaper.mp4

# または COSMIC ネイティブ設定 GUI を起動
./target/release/fluffy-settings
```

> **💡 デスクトップ統合と自動起動**: `./scripts/install-desktop-integration.sh` を実行すると、アプリアイコン・デスクトップランチャー・systemd ユーザーサービスが自動登録されます。詳細は [デスクトップ & systemd 導入ガイド](docs/SYSTEMD.ja.md) をご覧ください。

---

## 🌟 主なハイライト

- **非破壊オーバーレイ共存 (`Layer::Bottom`)**: ネイティブ壁紙 `cosmic-bg` と干渉せず背面に描画。Fluffy 停止時も元の静止画壁紙へ即座に復帰します。
- **黒画面ゼロのシームレス切替 & ループ**: デュアルパイプライン・プリロールとクロスフェード前処理により、切り替え時や周回時の黒画面・カクつきを完全排除。
- **ハードウェア動画再生支援 (NVDEC / VA-API)**: GPU デコーダーを優先利用して CPU・ファン負荷を最小化。非対応環境でも安全にソフトウェア再生へフォールバック。
- **常駐デーモンと設定 GUI の完全分離**: わずか 4.7MB / 40MB RSS の軽量デーモンと、操作時のみ起動する `libcosmic` 製ネイティブ設定 GUI の省メモリ設計。
- **全画面ウィンドウ時の一時停止**: ゲームや動画全画面時に自動一時停止してリソースを保護（COSMIC / wlroots プロトコル両対応）。
- **動的ホットプラグ & 4K正規化ストレージ**: モニター着脱・解像度変更への自動追従と、SHA-256 重複排除による永続ストレージ管理。

---

## 💻 基本操作 & CLI

```bash
# 壁紙の設定（全画面 / 特定画面）
fluffy set-video ~/Videos/wallpaper.mp4
fluffy set-video ~/Videos/sub.mp4 --output DP-2

# 再生制御（一時停止 / 再開 / 停止）
fluffy pause
fluffy resume
fluffy stop

# 状態確認 & 設定変更
fluffy status
fluffy config --restore-on-startup true
```

> 📖 **全コマンド一覧・詳細オプション・実践例**: [CLI コマンド仕様 (docs/CLI.ja.md)](docs/CLI.ja.md) をご覧ください。

---

## 📊 性能ベンチマーク

CachyOS / NVIDIA GeForce RTX 3080 / COSMIC Desktop 実機環境にて、4K デュアルモニター再生時でも安定した低 CPU・省メモリ（アイドル時 40MB RSS、NVDEC ハードウェア支援）を実証済みです。

> 📊 **解像度別・デュアル画面での実測データおよび詳細レポート**: [性能ベンチマーク報告書 (docs/BENCHMARK_REPORT.ja.md)](docs/BENCHMARK_REPORT.ja.md) をご覧ください。

---

## 📚 ドキュメント一覧

詳しい仕様やガイドは各ドキュメントにまとめられています：

| ドキュメント | 概要 |
| :--- | :--- |
| **[ドキュメントポータル](docs/PORTAL.ja.md)** | 全ドキュメントの総合インデックスと目的別ナビゲーション |
| **[CLI コマンド仕様](docs/CLI.ja.md)** | コマンド構文、全オプション解説、シェル向け実用例 |
| **[設定・状態保存仕様](docs/CONFIGURATION.ja.md)** | XDG 準拠設定項目 (`config.json`)、状態ファイル (`state.json`) |
| **[動画規格 & ストレージ仕様](docs/STORAGE_AND_FORMATS.ja.md)** | 適合動画規格、非同期トランスコード、SHA-256 永続ストレージ |
| **[デスクトップ & systemd 導入](docs/SYSTEMD.ja.md)** | 自動インストールスクリプト、systemd `--user` サービス管理 |
| **[性能ベンチマーク報告書](docs/BENCHMARK_REPORT.ja.md)** | 解像度別 CPU/メモリ/GPU 負荷の実機検証データ |
| **[技術仕様書 (Technical Design)](docs/TECHNICAL_DESIGN.ja.md)** | システム全体構造、IPC プロトコル仕様、Layer-shell 統合 |
| **[知的財産権 (IP) 監査記録](legal/IP_COMPLIANCE.ja.md)** | アイコン意匠・商標・ライセンスデューデリジェンス記録 |

---

## 🤖 開発手法（Vibe Coding）について

本プロジェクトは、AI との対話を繰り返しながら創り上げられた **Vibe Coding** の生成物です。単なるコード自動生成にとどまらず、以下のプロセスを AI と人間が二人三脚で何度も反復・循環させて開発されました：

- 💡 **アイデア創出 (Ideation)**: Wayland Layer-shell を用いた非破壊壁紙マネージャーの着想や機能要件のブレインストーミング
- 📝 **技術提案 & 設計 (Proposals & Design)**: 常駐デーモンと GUI の疎結合分離、黒画面なしプリロール切替、IPC 通信プロトコルなどのアーキテクチャ提案
- 🛠️ **実装 (Implementation)**: Rust によるデーモン、`libcosmic` 設定 GUI、GStreamer / FFmpeg パイプラインの実装とリファクタリング
- 🔬 **検証 (Verification)**: プロトコル仕様への準拠チェック、メモリ・CPU 使用率のプロファイリング、マルチモニター挙動の確認
- 🧪 **実機テスト (Testing & Benchmarking)**: COSMIC Desktop / Wayland 実機テストベッドでの動作確認、負荷ベンチマーク計測、エッジケースの不具合修正

---

## 📄 ライセンス

Fluffy は [MIT License](LICENSE) のもとで公開されています。  
外部ランタイムや依存ライブラリのライセンス境界については [サードパーティライセンス監査記録](legal/THIRD_PARTY_LICENSES.ja.md) をご覧ください。
