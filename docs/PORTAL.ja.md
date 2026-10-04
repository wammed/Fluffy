# 📚 Fluffy ドキュメントポータル

**Fluffy** の公式ドキュメントポータルへようこそ。  
Fluffy は、Pop!_OS COSMIC Desktop および Linux Wayland コンポジター向けにネイティブ開発された、軽量・非破壊・ハードウェアアクセラレーション対応の動画壁紙マネージャーです。

本ポータルを起点として、各仕様書、アーキテクチャ設計書、実機ベンチマーク記録、運用ガイドへアクセスできます。

<p align="center">
  <a href="PORTAL.md">English</a> | <a href="PORTAL.ja.md">日本語</a>
</p>

---

## 🧭 ドキュメント総合インデックス

| ドキュメント | 主要テーマ | 対象読者 |
| :--- | :--- | :--- |
| **[ルート README](../README.ja.md)** | クイックスタート、基本操作、主なハイライト | 初回導入ユーザーおよび一般利用者 |
| **[CLI コマンド仕様 (CLI Reference)](CLI.ja.md)** | CLI コマンド構文、オプション詳細、実践コマンド例 | 端末操作メインのユーザー・自動化シェル作成者 |
| **[設定・状態仕様 (Configuration)](CONFIGURATION.ja.md)** | XDG 設定項目 (`config.json`)、状態ファイル (`state.json`) | 設定調整およびシステム管理者 |
| **[動画規格 & ストレージ仕様 (Formats & Storage)](STORAGE_AND_FORMATS.ja.md)** | 適合動画規格、自動トランスコード、SHA-256 重複排除ストレージ | 動画作成者・ストレージ管理担当者 |
| **[デスクトップ & systemd 連携 (Systemd Guide)](SYSTEMD.ja.md)** | 自動インストーラー、systemd `--user` サービス管理 | デスクトップ統合および常駐管理担当者 |
| **[性能ベンチマーク報告書 (Benchmark Report)](BENCHMARK_REPORT.ja.md)** | 実機実測 CPU%、メモリ RSS、GPU 3D 利用率、NVDEC デコーダ負荷 | パフォーマンスエンジニア、システム最適化担当者 |
| **[技術仕様書 (Technical Design)](TECHNICAL_DESIGN.ja.md)** | システム全体構造、IPC JSON-RPC 仕様、キャッシュポリシー、Layer-shell 統合 | 開発者、アーキテクト、技術的コントリビューター |
| **[セッション引継ぎ書 (Session Handover)](SESSION_HANDOVER.ja.md)** | 開発ロードマップ、全フェーズ進捗、実機検証エビデンスマトリクス | 実装継続を担当する開発者 |
| **[知的財産権 (IP) デューデリジェンス記録書](../legal/IP_COMPLIANCE.ja.md)** | アプリアイコンの由来、独自性検証、商標・知的財産権クリアランス記録 | パッケージメンテナ、デスクトップインテグレーター、コントリビューター |
| **[サードパーティライセンス監査記録](../legal/THIRD_PARTY_LICENSES.ja.md)** | Rust 依存クレート、GStreamer & FFmpeg ランタイムライセンス、下流パッケージング方針 | パッケージャー、法務監査担当者、ディストリビューター |
| **[アイコン意匠設計・IPレビュー履歴](../legal/ICON_DESIGN_HISTORY.ja.md)** | 4アプリ横断アイコン創出経緯、AI プロンプト履歴、反復監査ログ | メンテナ、リポジトリアーキビスト |

---

## 🎯 目的別ナビゲーション

### 1. インストールとデスクトップ環境への統合
- **クイックスタート**: [ルート README: クイックスタート](../README.ja.md#-クイックスタート) を参照してください。
- **systemd `--user` サービス運用**: [デスクトップ & systemd 連携ガイド](SYSTEMD.ja.md) にてコマンド一覧やサービス設定を確認できます。
- **アプリアイコンと意匠**: `images/fluffy-icon.svg` および `data/icons/` にスケーラブル SVG アイコンが配置されています。権利クリアランス詳細は [知的財産権 (IP) デューデリジェンス記録書](../legal/IP_COMPLIANCE.ja.md) を参照してください。

### 2. 壁紙の操作と設定
- **CLI コマンド詳細**: [CLI コマンド仕様](CLI.ja.md) にて全コマンド・オプションおよび実用ワンライナーを確認できます。
- **GUI による直感操作**: `fluffy-settings` を起動して、対象ディスプレイの選択、動画ファイルの参照、適用をマウス操作で行えます。
- **設定ファイル・状態保存**: [設定・状態仕様](CONFIGURATION.ja.md) にて `~/.config/fluffy/config.json` および `state.json` の仕様を確認できます。
- **動画フォーマット適合規格**: [動画規格 & ストレージ仕様](STORAGE_AND_FORMATS.ja.md) にて、即時再生できる適合規格（H.264/yuv420p/30fps）と、初回バックグラウンド変換の仕様を確認できます。

### 3. アーキテクチャとプロトコルの詳細理解
- **非破壊オーバーレイ共存モデル**: [TECHNICAL_DESIGN.ja.md: セクション 3 & 4](TECHNICAL_DESIGN.ja.md) を参照し、`cosmic-bg` と干渉せず `Layer::Bottom` で描画・復帰する仕組みを理解できます。
- **黒画面ゼロ・プリロール切替 & 非同期変換**: [TECHNICAL_DESIGN.ja.md: セクション 8 & 11](TECHNICAL_DESIGN.ja.md) にてデュアルパイプライン切り替え構造および非同期バックグラウンド変換を詳解しています。
- **IPC 通信仕様**: [TECHNICAL_DESIGN.ja.md: セクション 9](TECHNICAL_DESIGN.ja.md) にて Unix ドメインソケット上の JSON Lines プロトコル仕様を確認できます。

### 4. 性能実績と実機検証エビデンスの確認
- **ベンチマーク実測値**: [BENCHMARK_REPORT.ja.md](BENCHMARK_REPORT.ja.md) にて、NVIDIA RTX 3080 ＋ 1440p デュアルディスプレイ環境下での 1080p / 1440p / 4K 負荷データを閲覧できます。
- **検証レベル**: [SESSION_HANDOVER.ja.md: セクション 2](SESSION_HANDOVER.ja.md) にて Phase 1〜7 の実機検証済みステータスを確認できます。

### 5. 法務・ライセンス・知的財産 (IP) プロヴェナンス
- **プロジェクトライセンス**: Fluffy は MIT License ([LICENSE](../LICENSE)) のもとで公開されています。
- **サードパーティおよび外部ランタイムライセンス**: Fluffy は GStreamer や FFmpeg を同梱しません。詳細なランタイム境界および `cargo-deny` 監査結果は [サードパーティライセンス監査記録](../legal/THIRD_PARTY_LICENSES.ja.md) を参照してください。
- **アイコン意匠デューデリジェンス**: アプリアイコンの由来・意匠レビュー履歴については [知的財産権 (IP) デューデリジェンス記録書](../legal/IP_COMPLIANCE.ja.md) および [アイコン意匠設計・IPレビュー履歴](../legal/ICON_DESIGN_HISTORY.ja.md) を参照してください。

---

<p align="center">
  <a href="../README.ja.md">← ルート README へ戻る</a> | <a href="PORTAL.md">To English Portal →</a>
</p>
