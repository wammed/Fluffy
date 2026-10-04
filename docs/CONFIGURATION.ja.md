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
    "loop_crossfade_ms": 0
  }
}
```

| 設定キー | 型 | 既定値 | 説明 |
| :--- | :--- | :--- | :--- |
| `restore_on_startup` | boolean | `false` | デーモン起動時やユーザーログイン時に、前回各モニターに適用した動画壁紙を自動復元するかどうか（既定はステートレス起動）。 |
| `autostart_daemon` | boolean | `false` | systemd `--user` サービス (`fluffy.service`) によるログイン時自動起動の有効状態。 |
| `pause_on_fullscreen` | boolean | `false` | ゲームや動画などの全画面ウィンドウ表示を検知した際に動画再生を一時停止して GPU/CPU 負荷を削減するかどうか。 |
| `loop_crossfade_ms` | u32 | `0` | ループ時の末尾・先頭クロスフェード時間（ミリ秒、`0` で無効）。有効時は FFmpeg `xfade` フィルタにより末尾と先頭をブレンドした完全シームレス動画を生成。 |

### CLI からの変更例
```bash
fluffy config --restore-on-startup true
fluffy config --autostart true
fluffy config --pause-fullscreen true
fluffy config --loop-crossfade-ms 1000
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
