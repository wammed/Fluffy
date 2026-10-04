# 💻 Fluffy CLI コマンド仕様

<p align="center">
  <a href="CLI.md">English</a> | <a href="CLI.ja.md">日本語</a> | <a href="PORTAL.ja.md">📚 ドキュメントポータル</a>
</p>

Fluffy のコマンドラインインターフェース (`fluffy`) の詳細仕様および使用例です。

---

## 構文

```text
fluffy [COMMAND] [OPTIONS]
```

---

## コマンド一覧

| コマンド | エイリアス | 概要 |
| :--- | :--- | :--- |
| `daemon` | `run` | 常駐型壁紙デーモンをフォアグラウンド起動 |
| `status` | - | IPC 経由でデーモンの稼働状況および各ディスプレイの状態を取得 |
| `set-video <PATH>` | `set_video` | 動画壁紙を変更（動画の検証・正規化・キャッシュ格納を含む） |
| `import <PATH>` | - | 再生を行わず事前に動画を検証しストレージにインポート |
| `pause` | - | 指定ディスプレイ（または全ディスプレイ）の動画再生を一時停止 |
| `resume` | - | 一時停止中の動画再生を再開 |
| `stop` | - | 動画再生を停止（サーフェスを破棄し元の静止画壁紙へ復帰） |
| `reload` | - | 現在の動画壁紙を再読み込み・再適用 |
| `config` | - | 起動時設定・動作設定の確認および変更 |
| `mark <LABEL>` | - | ベンチマーク・プロファイリング用マーカーを記録 |
| `help` | `--help` | ヘルプメッセージを表示 |

---

## グローバルオプション

| オプション | 引数 | 説明 | 既定値 |
| :--- | :--- | :--- | :--- |
| `--socket` | `<PATH>` | 対象デーモンの Unix ソケットパス | `$XDG_RUNTIME_DIR/fluffy.sock` |

---

## コマンド別詳細オプション

### `set-video` / `pause` / `resume` / `stop` / `reload`
- `--output <NAME>`: 対象ディスプレイ名 (例: `DP-1`, `DP-2`)。省略時は接続中の全ディスプレイに適用。
- `--generation <NUM>`: (set-video のみ) レースコンディション防止用の単調増加世代番号。
- `--timeout <SECS>`: (set-video のみ) IPC 応答待ちタイムアウト秒数（既定: `60` 秒）。

### `daemon`
- `--output <NAME>`: 特定のディスプレイのみにバインドして動作。
- `--video <PATH>`: 起動と同時に再生を開始する動画パス。

### `import`
- `--crossfade-ms <MS>`: シームレスループ用の先頭・末尾クロスフェード時間（ミリ秒単位。省略時は config 設定値を使用）。

### `config`
- `--restore-on-startup <BOOL>`: デーモン起動時・ログイン時に前回の壁紙を自動復元 (`true` / `false`)。
- `--autostart <BOOL>`: systemd `--user` 経由でログイン時のデーモン自動起動を有効化/無効化 (`true` / `false`)。
- `--pause-fullscreen <BOOL>`: ウィンドウ全画面表示時の一時停止設定 (`true` / `false`)。
- `--loop-crossfade-ms <MS>`: ループ時のクロスフェード時間（ミリ秒、`0` で無効化）。

---

## 実践使用例

```bash
# 全てのモニターに動画壁紙を設定
fluffy set-video ~/Videos/ambient_city.mp4

# サブモニター (DP-2) のみ動画壁紙を変更
fluffy set-video ~/Videos/nature.mp4 --output DP-2

# メインモニター (DP-1) の再生を一時停止
fluffy pause --output DP-1

# メインモニター (DP-1) の再生を再開
fluffy resume --output DP-1

# 全ディスプレイの現在の再生状態・変換状態を確認
fluffy status

# 現在の設定内容および保存された壁紙状態の確認
fluffy config

# 起動時・ログイン時の壁紙自動復元を有効化（オプトイン）
fluffy config --restore-on-startup true

# systemd ユーザーサービスによるログイン時デーモン自動起動を有効化
fluffy config --autostart true

# 全画面ウィンドウ検出時の自動一時停止を有効化
fluffy config --pause-fullscreen true

# ループ時のクロスフェードを 1.0 秒 (1000 ms) に設定
fluffy config --loop-crossfade-ms 1000

# 1.0 秒のシームレスクロスフェードをかけて事前インポート（即時再生なし）
fluffy import ~/Videos/loop_bg.mp4 --crossfade-ms 1000
```
