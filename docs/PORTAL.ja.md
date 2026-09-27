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
| **[ルート README](../README.ja.md)** | クイックスタート、インストール手順、機能ハイライト、CLI & GUI 使い方 | 初回導入ユーザーおよびシステム管理者 |
| **[技術仕様書 (Technical Design)](TECHNICAL_DESIGN.ja.md)** | システム全体構造、IPC JSON-RPC 仕様、キャッシュポリシー、Layer-shell 統合 | 開発者、アーキテクト、技術的コントリビューター |
| **[セッション引継ぎ書 (Session Handover)](SESSION_HANDOVER.ja.md)** | 開発ロードマップ、全フェーズ進捗、実機検証エビデンスマトリクス | 実装継続を担当する開発者 |
| **[性能ベンチマーク報告書 (Benchmark Report)](BENCHMARK_REPORT.ja.md)** | 実機実測 CPU%、メモリ RSS、GPU 3D 利用率、NVDEC デコーダ負荷 | パフォーマンスエンジニア、システム最適化担当者 |

---

## 🎯 目的別ナビゲーション

### 1. インストールとデスクトップ環境への統合
- **クイックインストール**: [ルート README: クイックスタート](../README.ja.md#-クイックスタート) を参照してください。
- **systemd `--user` サービス登録**: `scripts/install-desktop-integration.sh` を実行して、`fluffy.service` および `com.github.fluffy.Fluffy.desktop` を自動配置します。
- **systemctl による運用管理**: [ルート README: デスクトップ導入](../README.ja.md#%EF%B8%8F-デスクトップ--systemd-ユーザーサービス導入) にてコマンド一覧を確認できます。

### 2. CLI および GUI からの壁紙操作
- **CLI コマンド一覧**: [ルート README: CLI コマンド仕様](../README.ja.md#-cli-コマンド仕様) にて `fluffy set-video`, `fluffy pause`, `fluffy resume`, `fluffy stop`, `fluffy status` の利用法を確認できます。
- **GUI による直感操作**: `fluffy-settings` を起動して、対象ディスプレイの選択、動画ファイルの参照・キャッシュ変換、適用をマウス操作で行えます。

### 3. アーキテクチャとプロトコルの詳細理解
- **非破壊オーバーレイ共存モデル**: [TECHNICAL_DESIGN.ja.md: セクション 3 & 4](TECHNICAL_DESIGN.ja.md) を参照し、`cosmic-bg` と干渉せず `Layer::Bottom` で描画・復帰する仕組みを理解できます。
- **黒画面ゼロ・プリロール切替**: [TECHNICAL_DESIGN.ja.md: セクション 8](TECHNICAL_DESIGN.ja.md) にてデュアルパイプライン切り替え構造を詳解しています。
- **IPC 通信仕様**: [TECHNICAL_DESIGN.ja.md: セクション 9](TECHNICAL_DESIGN.ja.md) にて Unix ドメインソケット上の JSON Lines プロトコル仕様を確認できます。

### 4. 性能実績と実機検証エビデンスの確認
- **ベンチマーク実測値**: [BENCHMARK_REPORT.ja.md](BENCHMARK_REPORT.ja.md) にて、NVIDIA RTX 3080 ＋ 1440p デュアルディスプレイ環境下での 1080p / 1440p / 4K 負荷データを閲覧できます。
- **検証レベル**: [SESSION_HANDOVER.ja.md: セクション 2](SESSION_HANDOVER.ja.md) にて Phase 1〜7 の実機検証済みステータスを確認できます。

---

<p align="center">
  <a href="../README.ja.md">← ルート README へ戻る</a> | <a href="PORTAL.md">To English Portal →</a>
</p>
