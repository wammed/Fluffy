# 🖥️ Fluffy デスクトップ & systemd ユーザーサービス導入ガイド

<p align="center">
  <a href="SYSTEMD.md">English</a> | <a href="SYSTEMD.ja.md">日本語</a> | <a href="PORTAL.ja.md">📚 ドキュメントポータル</a>
</p>

Fluffy をデスクトップ環境に常駐させ、ログイン時の自動起動やクラッシュ時の自動復帰を行うためのセットアップガイドです。

---

## 自動インストーラーによるワンストップ導入

リポジトリ内にデスクトップ統合用のスクリプトが用意されています：

```bash
./scripts/install-desktop-integration.sh
```

このスクリプトは以下を自動実行します：
1. 最適化済みリリースバイナリ (`fluffy` および `fluffy-settings`) のビルド
2. バイナリを `~/.local/bin/` にインストール
3. `fluffy.service` を `~/.config/systemd/user/` に配置し daemon-reload
4. アプリアイコンを `~/.local/share/icons/hicolor/scalable/apps/` に配置
5. デスクトップエントリを `~/.local/share/applications/com.github.fluffy.Fluffy.desktop` に配置

---

## systemd `--user` によるライフサイクル管理

```bash
# ログイン時の自動起動と今すぐの起動を有効化
systemctl --user enable --now fluffy.service

# デーモンの稼働状態の確認
systemctl --user status fluffy.service

# ログ（journalctl）のリアルタイム追従
journalctl --user -u fluffy.service -f

# デーモンの停止
systemctl --user stop fluffy.service

# デーモンの再起動
systemctl --user restart fluffy.service

# 自動起動の無効化
systemctl --user disable fluffy.service
```

---

## サービス定義 (`fluffy.service`) のポイント

- `After=graphical-session.target`: デスクトップセッションおよび Wayland コンポジター起動後に実行されます。
- `Restart=on-failure`: 不意のプロセス終了時に自動で再起動します。
- `ExecStart=%h/.local/bin/fluffy daemon`: ユーザーホーム配下の絶対パスで確実にバイナリを呼び出します。
