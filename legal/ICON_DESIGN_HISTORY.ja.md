# アイコンデザイン・IPレビュー履歴

> **公開リポジトリ記録**
>
> 本ドキュメントは、Fluffy、Waddle、Rooney、Toodle の4アプリケーションで使用する
> アイコンについて、開発履歴、AIを用いた類似性レビュー、デザイン変更、およびプロヴェナンス
> （由来）を記録するものです。
>
> **対象範囲と限界:** 本ドキュメントはプロジェクトの開発履歴およびデューデリジェンス記録であり、
> 法的意見、商標クリアランス、著作権クリアランス、意匠権クリアランス、または第三者の権利が
> 関係しないことを保証するものではありません。本文中に登場する「Zero Risk」「Approved」
> 「完全適合」等の断定的な表現は、履歴として保存されたAI生成レビューの出力であり、
> **本ドキュメントにおける法的結論として採用するものではありません**。
>
> 本ドキュメントの目的は、アイコンのデザインおよびプロヴェナンスの過程を透明かつ追跡可能な
> 形で記録することです。

## 公開記録の読み方

- **過去のAI出力**は、プロヴェナンスのために必要な範囲で履歴として保存しています。これを法的助言や、プロジェクトが法的クリアランスを取得したことを示す記述として解釈しないでください。
- **プロジェクトによる対応**は、類似性に関する懸念が確認された際に、どのデザイン要素を削除・維持・再設計したかという観点から記録しています。
- **現在のリポジトリ向け記録**は、Fluffy、Waddle、Rooney、Toodle の各プロジェクトに配置された個別の `IP_COMPLIANCE.md` です。

# 開発中4アプリ アイコンAI生成・知的財産（IP）監査履歴

本ドキュメントは、Geminiとの初期の対話からアイコンの類似性レビュー、要約ドキュメントの作成、
外部レビューを経て最終確定版（Due Diligence Record）に至るまでのプロセスを時系列に沿って
記録した統合記録です。

## 1. プロジェクト共通のデザイン方針と監査基準

- **統一デザイン言語:**
  - COSMIC Desktop Environment（libcosmic）およびLinux Wayland環境に調和する、光沢やネオン発光、
    偽の3Dシャドウ（エンボス）を排除した**完全なマット調（艶消し）・フラットデザイン**。
  - GitHub READMEおよびストア掲載用アセットバナーは、背景を「**完全なフラットのチャコールグレー単色**」
    とし、中央にアプリアイコンとアプリ名（AppName）のみをミニマルに配置する構成で統一。
- **IPコンプライアンス（由来証明）管理:**
  - AI生成物特有の権利問題やストア審査（Flathub、COSMIC Store等）でのリジェクト（他者資産との誤認・混同）
    を防ぐため、プロヴェナンス（生成由来・監査結果）を記録した `IP_COMPLIANCE.md` を各プロジェクトの
    リポジトリへ配備。

## 2. 各アプリケーションの経緯・類似性指摘・再生成評価

### ① Fluffy（Linux / COSMIC DE専用 動画壁紙マネージャ）

#### 1. 初回生成と類似性指摘

- **初期デザイン:** 雲や液体のような流線型デザインの中央に再生マーク（▶）を配置したモダンなアイコン。
- **Geminiによる類似性・リスク指摘:**
  - **Fluid Video Player:** 液体・水流が渦巻く中に再生マーク（▶）を配置した構図が類似していると評価された。
  - **Videoクラウド:** 雲の輪郭線内に再生マークを配置した構図が類似していると評価された。
  - **IRIS player:** アメーバ状の有機的な形状に再生マークを内包する構成が類似していると評価された。
  - **QNAP cloud surveillance:** 雲のマークと再生ボタンを組み合わせた構成が比較対象として挙げられた。
- これらはAIによる視覚的比較結果であり、法的な侵害判断ではありません。

#### 2. 再生成の経緯

1. **方向性の転換:** 汎用的な動画プレイヤーとの混同可能性を避けるため、アプリの頭文字「F」と
   Linux/COSMIC DE向けの要素を融合させる方針を採用。
2. **質感・構成の調整:** 発光エフェクトやガラス反射を削ぎ落とし、完全なマット調＋チャコールグレー背景に変更。
   背景のコードスニペットや余計な装飾を排除し、アイコンとアプリ名のみのミニマルバナーへ移行。
3. **COSMIC公式意匠・陳腐化の懸念による再修正:**
   - 途中の案に含まれていた「左下のオレンジ色の多重リング」がSystem76 COSMIC DE公式ロゴの軌道ラインを
     想起させるとの指摘を受けた。
   - 中央のオレンジの意匠が一般的な原子（アトム）マークに見えるというデザイン上の懸念も確認された。
   - **最終修正方針:** オレンジ多重リング・原子マーク・グラデーションを撤廃。頭文字「F」を動画タイムラインや
     ディスプレイフレームに見立てたマットホワイトの幾何学的ラインとし、中央にシアンブルー単色の
     「ディスプレイ＋再生マーク」を配置する構成へ刷新。

#### 3. 再生成アイコンの評価

- **当時のAI評価:** 汎用的な動画・クラウド系アイコンとの差別化が進んだと評価された。
- **Adobe Creative Cloud:** 流線型のループシルエットについて広い意味での想起可能性が比較された。
  現在の構成はF字型フレームとして再設計されている。
- **Papyrus:** 同じCOSMIC向け動画壁紙カテゴリのアプリとして比較された。Papyrusの巻物・紙モチーフに対し、
  FluffyはF字型フレーム＋ディスプレイ構成を採用。
- **System76公式ロゴ:** オレンジの多重環を撤廃し、公式ロゴに依存しない構成へ変更した。

### ② Waddle（完全ローカルAI統合型 次世代Linuxターミナルエミュレータ）

#### 1. 初期デザインと類似性指摘

- **初期デザイン:** 青/緑系の「W」の文字を背景に、両手を広げたLinuxペンギン（Tux）と稲妻を配置。
- **Geminiによる類似性指摘:**
  - **WSL (Windows Subsystem for Linux):** 「W＋Tux」の構図がWSL関連のビジュアルアイデンティティと
    強く類似すると評価された。
  - **WezTerm:** ターミナルエミュレータというカテゴリと頭文字「W」が比較対象として挙げられた。
- これらはAIによる視覚的比較結果であり、法的な侵害判断ではありません。

#### 2. 再生成の経緯

- **ペンギンの完全排除:** WSLとの混同要因として認識されたTuxを削除。
- **ターミナルプロンプト統合案:** 単純な「W」をやめ、Linuxターミナルの入力プロンプト（`>`）やコードラインを
  組み合わせた幾何学的ラインで「W」のシルエットを再構築。
- **要素の統合とフラット化:** 中央にローカルAIと高速処理を象徴するミニマルな稲妻シンボルを配置し、
  完全なマット質感・チャコールグレー単色背景へ統一。

#### 3. 再生成アイコンの評価

- **当時のAI評価:** WSLとの視覚的な関連を生じさせる要素が減少したと評価された。
- 現在の「W」は端末プロンプトを含む幾何学構成として再設計され、Tuxは使用していない。

### ③ Rooney（COSMIC / Wayland向け AI統合型コード＆Markdownエディタ）

#### 1. 初期デザインと類似性指摘

- **初期デザイン:** 大きな歯車を背景に、中央に `</>` マーク入りディスプレイ、下部に太い「R」の文字。
- **Geminiによる類似性指摘:**
  - **Rust言語公式ロゴ:** 「歯車＋R」の構図がRustの公式ロゴと強く類似すると評価された。
  - **RStudio / Posit:** 開発ツールというカテゴリおよび「R」という頭文字の共通性から比較対象となった。
  - **AI生成の痕跡:** 初期画像にAI生成を示す透かし等があったことも品質上の検討事項となった。
- これらはAIによる視覚的比較結果であり、法的な侵害判断ではありません。

#### 2. 再生成の経緯

- **歯車モチーフの完全撤廃:** Rustとの視覚的関連を避けるため、歯車を削除。
- **アーキテクチャの視覚化:** Rooneyのコア機能である「2分割ペイン（Split-View / Ctrl + \）」と
  内部バッファ構造である「Rope」をモチーフとし、頭文字「R」の縦棒部分を2分割ペインに見立てた
  幾何学的モノグラムへ刷新。
- **フラット化:** 単色チャコールグレー背景にマットなラインで構成。

#### 3. 再生成アイコンの評価

- **当時のAI評価:** Rust公式ロゴとの視覚的な関連が減少したと評価された。
- 現在のモノグラムは、R、Rope、Split-Viewというアプリ固有の概念を基礎としている。

### ④ Toodle（COSMIC Desktop Environment向け モダン・デジタルクロック＆ウェザーウィジェット）

#### 1. 初期デザインと類似性指摘

- **初期デザイン:** チャコール背景に「T」と時計の文字盤を組み合わせた白いモノグラム、右上に発光する太陽。
- **Geminiによる類似性指摘:**
  - **Toodledo:** 名前の前方一致およびタスク/時間管理という類似カテゴリが比較対象となった。
  - **Todoist:** 「T」と円形要素を組み合わせるタスク管理アイコンの傾向が比較対象となった。
  - **デザイン上の課題:** 太陽のグローや外枠の3Dエンボスが、プロジェクト共通のフラット・マット方針から逸脱していた。

#### 2. 再生成の経緯

- 太陽の発光グラデーションをフラットなソリッドイエローへ変更。
- 外枠の立体シャドウを除去。
- チャコール単色背景、「T」×時計文字盤、マットイエローの太陽によるミニマルな構成へ整理。

#### 3. 再生成アイコンの評価

- ToodleではWaddleやRooneyと同様の構造的な再設計は行わず、既存コンセプトを維持しながら質感・装飾を整理した。
- **T**、時計、天候という機能的要素を維持している。
- 法的クリアランスを主張するものではありません。

## 3. 外部レビューによる改訂経緯（初期ドラフトから確定版への移行）

要約ドキュメント作成後、GitHubなどのオープンソースリポジトリへ配備する前に、
法務および知的財産デューデリジェンスの観点から外部レビューが実施されました。

### AI特有の法的過言の整理

初期ドラフト内の「Zero Risk」「Approved」「完全適合」といった表現は、Geminiが生成した会話上の
定性的な表現であり、包括的な全世界商標調査や弁理士・弁護士による法的意見書を意味するものでは
ないことが確認されました。

そのため、これらの表現は履歴資料として保持する一方、リポジトリの正式な記録では法的結論として
採用しない方針としました。

### Scope & Limitations の明確化

各 `IP_COMPLIANCE.md` は、以下を目的とするデューデリジェンス記録として整理されています。

- デザインの由来（Provenance）
- 類似性レビューの履歴
- 類似性を踏まえて実施したデザイン変更
- 第三者資産・ブランド要素の取り扱い
- AI支援レビューの限界

これらは包括的な法的クリアランスを意味しません。

### 記録体裁の統一

会話ログで本文が十分に提示されていなかったWaddleを正式に追加し、Fluffyの重複版を整理した上で、
4アプリについて統一された `IP_COMPLIANCE.md` を2026-09-28付で策定しました。

## 4. 変更後・最終確定版：各リポジトリ配備 `IP_COMPLIANCE.md`

外部レビューに基づいて整理された各リポジトリの公開用記録では、AIによる断定的な法的表現を採用せず、
以下のような事実ベースの記述へ統一しています。

### Waddle

- 初期案ではW＋Tuxを使用。
- GeminiによるレビューでWSL関連の視覚的類似性が指摘された。
- Tuxを削除し、`>` プロンプトやコードラインを基礎とするWへ再設計。
- AIによる評価、検証は `IP_COMPLIANCE.md` では法的結論として採用しない。

### Rooney

- 初期案では歯車＋Rを使用。
- GeminiによるレビューでRust公式ロゴとの強い視覚的類似性が指摘された。
- 歯車を削除し、R、Rope、Split-Viewを基礎とするモノグラムへ再設計。
- 公開用文書では法的クリアランスを主張しない。

### Toodle

- T＋時計＋天候という基本コンセプトを維持。
- グロー、グラデーション、疑似3D表現を整理。
- Meteocons由来の天候アイコンは、アプリブランドアイコンとは別の第三者資産としてライセンス管理を継続。
- アイコンについて包括的な法的クリアランスを主張しない。

### Fluffy

- 初期案では雲・液体＋再生マークを使用。
- 途中案に含まれたCOSMIC公式ブランドを想起させる多重リング要素を削除。
- 現在のコンセプトは、F字型の流線フレーム＋ディスプレイ／再生マーク。
- 公式COSMICロゴに依存しない構成へ変更。

## 5. 記録の役割分担

4アプリのアイコン関連記録は、次の2層で管理します。

```text

ICON_DESIGN_HISTORY.md
ICON_DESIGN_HISTORY.ja.md
    ↓
4アプリを横断した公開用のデザイン変更・プロヴェナンス履歴

IP_COMPLIANCE.md
    ↓
各プロジェクトにおける現在の公開用Due Diligence Record
```

これらの文書はそれぞれ目的が異なります。AIによる過去の評価を保存することと、
プロジェクトとして現在採用している法的立場を区別することを意図しています。

## 6. 最終確定版
 
 【変更後・最終確定版】各リポジトリ配備 IP_COMPLIANCE.md（原本・無省略）外部レビューに基づき再策定され、各リポジトリのルートに配備された最終確定版（2026-09-28策定）の全文です。一切の要約・省略を行わず、原本テキストそのまま掲載します。

 ## ① Waddle 最終確定版（IP_COMPLIANCE.md）Markdown# Icon Design & IP Compliance Due Diligence Record

This document records the provenance, design-review history, and IP due-diligence process for the **Waddle** application icon.

> **Scope:** This is a project provenance and due-diligence record. It is not a legal opinion, trademark clearance, or guarantee that no third-party rights are implicated.

## 1. Asset & Provenance

- **Application:** Waddle
- **Purpose:** Linux / COSMIC-oriented AI-integrated terminal emulator.
- **Generation method:** AI-assisted image generation and iterative refinement using Google Gemini.
- **Current design language:** Flat geometric matte styling with terminal/CLI elements.
- **Current visual concept:** A custom **W** constructed from terminal-prompt/code-like geometric segments, with a central lightning symbol.

## 2. Design Review History

### Initial concept

The initial Waddle icon combined a large W with a Linux Tux penguin and a lightning symbol.

The Gemini-assisted review identified a strong visual similarity between the W + Tux composition and WSL-related visual identity.

This was treated as a design-collision concern rather than as a definitive legal infringement finding.

### Revision

The project removed the penguin/Tux element and adopted a terminal-oriented design:

- the W is reconstructed from CLI prompt/code-like geometry;
- the `>` prompt concept is integrated into the letterform;
- the lightning symbol communicates AI/high-speed functionality;
- the visual treatment is matte and minimal.

This change deliberately moves the icon away from the earlier W + Tux composition.

## 3. Similarity Review

| Reference | Observation | Project action |
| :--- | :--- | :--- |
| WSL / Windows Subsystem for Linux | Gemini identified a strong similarity in the earlier W + Tux composition. | Removed Tux and rebuilt the W using terminal-oriented geometry. |
| WezTerm | Same broad terminal-emulator category and W initial. | Current icon uses a distinct geometric W built from CLI prompt segments. |
| Alacritty / other terminal applications | Same general software category. | No known direct reuse of their branding was identified in the recorded review. |

## 4. Current Design Rationale

The current icon is based on the application's own name and function:

- **W** → Waddle;
- **`>` / terminal geometry** → terminal emulator;
- **lightning** → local AI / high-performance behavior.

The previous Tux mascot was intentionally removed as part of the design revision.

## 5. AI-Assisted Review Limitations

The Gemini conversation contains strong wording such as “zero risk,” “fully compatible,” and similar conclusions. Those statements are preserved as part of the historical source record but are not adopted here as legal conclusions.

The visual review was not a comprehensive trademark, copyright, design-right, or unfair-competition clearance.

No claim is made that no third-party mark anywhere in the world is similar to the current design.

## 6. Record Keeping

The detailed Gemini conversation is retained separately as the underlying development/provenance record.

This document is the concise public-facing record and should be updated if the icon changes materially.

## 7. Current Status

**Status:** The current icon uses the revised terminal-prompt W design without the earlier Tux element.

**Review date:** 2026-09-28


## ② Rooney 最終確定版（IP_COMPLIANCE.md）Markdown# Icon Design & IP Compliance Due Diligence Record

This document records the provenance, design-review history, and IP due-diligence process for the **Rooney** application icon.

> **Scope:** This is a project provenance and due-diligence record. It is not a legal opinion, trademark clearance, or guarantee that no third-party rights are implicated.

## 1. Asset & Provenance

- **Application:** Rooney
- **Purpose:** COSMIC / Wayland-oriented AI-integrated Linux code and Markdown editor.
- **Generation method:** AI-assisted image generation and iterative refinement using Google Gemini.
- **Current design language:** Flat, non-glossy, matte styling on a solid charcoal background.
- **Current visual concept:** A geometric **R** incorporating the application's **Rope** concept and **Split-View / two-pane editing** concept.

## 2. Design Review History

### Initial concept

The earlier Rooney icon used a prominent gear combined with a large R and editor/code imagery.

The Gemini-assisted review identified a strong visual resemblance between the gear + R arrangement and the Rust programming language's official logo.

Because Rooney itself is implemented in Rust, the project considered it particularly important to avoid unnecessary visual association with Rust branding.

### Revision

The project replaced the gear-based composition with a geometric monogram based on:

- the letter **R**;
- the internal **Rope** buffer concept;
- **Split-View / two-pane editing**.

The revised icon uses a solid charcoal background and matte treatment.

## 3. Similarity Review

| Reference | Observation | Project action |
| :--- | :--- | :--- |
| Rust programming language branding | Earlier gear + R composition was identified as strongly similar. | Removed the gear motif and redesigned the symbol from application-specific concepts. |
| RStudio / Posit | Shares the broad R + developer-tool category. | Current design uses a different geometric construction and application-specific Split-View/Rope concept. |

## 4. Current Design Rationale

The final monogram is intended to represent the application itself rather than its implementation language:

- **R** → Rooney;
- **Rope** → editor buffer/data structure concept;
- **Split-View** → a core editor feature.

The gear motif associated with the earlier draft was removed.

## 5. AI-Assisted Review Limitations

The original Gemini conversation contains definitive phrases such as “zero risk,” “complete compliance,” and “approved.” Those phrases are historical AI output and are not adopted here as legal findings.

The recorded comparison does not constitute comprehensive trademark or copyright clearance.

In particular, the use of Rust as a development language does not by itself determine whether an icon is legally permissible; the relevant question is the actual visual asset and surrounding use.

## 6. Record Keeping

The detailed Gemini conversation is retained separately as the underlying development/provenance record.

This document is the concise repository-facing summary.

## 7. Current Status

**Status:** The current icon uses the redesigned geometric R / Rope / Split-View concept and does not intentionally reproduce the earlier gear-based composition.

**Review date:** 2026-09-28


## ③ Toodle 最終確定版（IP_COMPLIANCE.md）Markdown# Icon Design & IP Compliance Due Diligence Record

This document records the provenance, design-review history, and IP due-diligence process for the **Toodle** application icon.

> **Scope:** This is a project provenance and due-diligence record. It is not a legal opinion, trademark clearance, or guarantee that no third-party rights are implicated.

## 1. Asset & Provenance

- **Application:** Toodle
- **Purpose:** Modern digital clock and weather widget for COSMIC Desktop.
- **Generation method:** AI-assisted image generation and iterative refinement using Google Gemini.
- **Current design language:** Flat, non-glossy, matte styling on a solid charcoal background.
- **Current visual concept:** The application initial **T**, an integrated **clock dial**, and a simplified sun/weather symbol.

## 2. Design Review History

The Toodle icon was reviewed for visual similarity and branding concerns.

Unlike Waddle and Rooney, the Toodle review did not result in a comparable structural redesign. The current concept remains based on the combination of:

- **T** → Toodle;
- **clock dial** → digital clock/time function;
- **sun/weather symbol** → weather function.

The Gemini record also discusses the use of Meteocons-inspired weather symbolism.

## 3. Similarity / Naming Considerations

The recorded review identified that clock + letter designs are a common visual approach and noted the potential for name/category similarity with time-management applications, including Toodledo.

This should be understood as a naming/marketplace observation, not as a conclusion that the Toodle name or icon infringes another party's rights.

No definitive legal clearance is asserted by this document.

## 4. Third-Party Asset Consideration

Toodle uses Meteocons-derived weather icon assets in the application separately from the application brand icon. Those assets should continue to be tracked under the project's third-party asset/license documentation.

The application icon itself is documented here as a separate branding asset.

## 5. Current Design Rationale

The icon communicates Toodle's functionality through a compact combination:

- **T** — application identity;
- **clock** — time;
- **sun/weather** — weather.

The design was intentionally simplified by removing excessive glow, lighting gradients, and skeuomorphic effects.

## 6. AI-Assisted Review Limitations

The Gemini source record contains statements describing the icon as “approved,” “original,” or otherwise legally safe. Those statements are historical AI output and are not adopted here as legal conclusions.

The review is not a comprehensive worldwide trademark, copyright, design-right, or unfair-competition search.

## 7. Record Keeping

The detailed Gemini conversation is retained separately as the underlying development/provenance record.

This document is the concise repository-facing summary and should be updated if the icon or its underlying third-party assets materially change.

## 8. Current Status

**Status:** Current T + clock + weather icon retained following the recorded visual review. No structural redesign was required as part of the recorded icon review.

**Review date:** 2026-09-28


## ④ Fluffy 最終確定版（IP_COMPLIANCE.md）Markdown# Icon Design & IP Compliance Due Diligence Record

This document records the provenance, design-review history, and IP due-diligence process for the **Fluffy** application icon.

> **Scope:** This is a project provenance and due-diligence record. It is not a legal opinion, trademark clearance, or guarantee that no third-party rights are implicated.

## 1. Asset & Provenance

- **Application:** Fluffy
- **Purpose:** Video wallpaper manager for Linux / COSMIC DE
- **Generation method:** AI-assisted image generation and iterative refinement using Google Gemini.
- **Design direction:** Flat, non-glossy, matte visual language with a solid charcoal background.
- **Current concept:** A custom **F-shaped flowing frame** combined with a minimal **display + play** symbol.
- **Source record:** The detailed Gemini conversation log is retained separately as the development/provenance source record.

## 2. Design Review History

### Initial concept

The initial icon used a flowing cloud/liquid form with a central play symbol. The Gemini-assisted review identified visual similarities with the general design language of video/cloud applications, including examples such as Fluid Video Player, Videoクラウド, IRIS player, and QNAP cloud surveillance.

These observations are recorded as AI-assisted visual comparison results, not as legal determinations.

### COSMIC branding consideration

A later design incorporated a circular/multi-ring visual element intended to communicate the COSMIC environment. The review record described this element as resembling the official COSMIC branding.

Because use of an official third-party brand mark is a separate branding/trademark question from the originality of the surrounding artwork, the project decided not to rely on that element in the final icon.

### Current revision

The icon was revised to:

- remove the COSMIC-like circular/multi-ring element;
- remove unnecessary code snippets and decorative information;
- retain the Fluffy-specific **F** structure;
- retain the **display + play** functional symbol;
- use a simple, matte, solid-color presentation.

The purpose of this revision is to make the application identity independent of an official COSMIC logo or similar third-party brand element.

## 3. Third-Party / Similarity Review

| Reference | Observation | Project action |
| :--- | :--- | :--- |
| Video/cloud application icons | The initial cloud/liquid + play composition used common visual conventions in the video category. | Reworked the composition around a distinctive F-shaped frame and display symbol. |
| System76 COSMIC branding | A previous draft contained a visual element resembling the official COSMIC mark. | Removed that element from the current icon. |
| Papyrus | Same general COSMIC animated-wallpaper category. | Differentiation is based on the Fluffy-specific F/frame construction rather than reuse of Papyrus branding. |
| Adobe Creative Cloud | Gemini noted a broad flowing/loop-form resemblance in an earlier draft. | Current design is structured as an F-shaped frame rather than an Adobe-style loop. |

## 4. Current Design Rationale

The current icon intentionally communicates two things without relying on third-party branding:

1. **Fluffy** — represented by the custom flowing **F** frame.
2. **Video wallpaper** — represented by the display/play symbol.

The icon does not intentionally incorporate the official COSMIC logo.

## 5. AI-Assisted Review Limitations

The Gemini log contains AI-generated similarity assessments and recommendations. Those statements are retained as part of the provenance record, but they should not be treated as authoritative legal conclusions.

In particular, statements such as “zero risk,” “fully compliant,” “approved,” or “no infringement” in the original AI conversation are not adopted here as legal findings.

No claim is made here that the icon has undergone comprehensive worldwide trademark, copyright, design-right, or unfair-competition clearance.

## 6. Record Keeping

The detailed Gemini conversation should be retained as an underlying development record. This document is the concise public-facing summary.

The two records serve different purposes:

- `gemini-log.md`: detailed source/provenance record;
- `IP_COMPLIANCE.md`: concise, factual due-diligence summary suitable for repository publication.

## 7. Current Status

**Status:** Current icon revised to remove the COSMIC-like official-brand element.

The project should reassess the icon if its visual design changes materially or if new third-party similarity concerns are identified.

**Review date:** 2026-09-28
   

## 7. 更新方針

以下の場合には、この文書および該当する `IP_COMPLIANCE.md` を更新します。

- アイコンの主要な構造・意匠を変更した場合
- 新たな第三者資産を使用した場合
- 新たな類似性・ブランド上の懸念を確認した場合
- 既存のプロヴェナンス情報を修正・補足する必要が生じた場合

**Review date:** 2026-09-28
