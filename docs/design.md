# ruminate 設計メモ

2026-10-28 時点で決めたことの記録です。

## 目的

Claude などのAIエージェントと一緒にやったことを、安川康介『科学的根拠に基づく最強の勉強法』の方法で振り返り、理解を深める。

- **アクティブリコール**: 読み返すのではなく、何も見ずに思い出す問題を出す
- **分散学習**: 翌日・約1週間後・約1か月後と間隔をあけて同じ内容に戻る
- **交互学習**: テーマを混ぜて出題する
- **自己説明**: 「自分の言葉で説明して」という問題を入れる
- **すぐのフィードバック**: 合っている点 → 抜け → 誤解 の順に返す

明示的に呼び出さなくても毎晩自動で問題が届き、パソコンを閉じていても動くことを目指す。

## 全体の構成

```
パソコン: ruminate（Rust）
  Claude Code の会話 ─────hook / sync──┐
  claude.ai・ChatGPT の                  │
  エクスポート ─────────import──────────┤
                                        ▼
claude.ai: 取り込みタスク（毎日・予定）──> ruminate-data（Private）
  claude.ai の新しい会話                     ├─ logs/<agent>/…jsonl
                                             └─ review/…jsonl
claude.ai: 出題タスク（毎晩・予定）<──DuckDBで読む──┘
  出題 → スマホに通知 → 結果を review/ に書く
```

| 役割 | どこで動く | 何をする |
|---|---|---|
| `ruminate`（このリポジトリ） | パソコン | ターミナルの会話を `ruminate-data` に送る（`sync`）。公式エクスポートの取り込み（`import`） |
| 取り込みタスク | claude.ai のスケジュール | claude.ai の新しい会話を `logs/claude-ai/` に書く。通知しない |
| 出題タスク | claude.ai のスケジュール | `ruminate-data` だけを読んで出題し、結果を `review/` に書く。出題があるときだけ通知 |

claude.ai の会話を読めるのは claude.ai の中で動くセッション（過去チャット検索のツール）だけなので、取り込みは claude.ai のタスクが受け持つ。過去分や全文の取りこぼしは、公式エクスポートを `ruminate import` で取り込んで埋める。

## リポジトリ

| リポジトリ | 公開 | 中身 |
|---|---|---|
| `ruminate` | Public | Rust 本体、Claude Code プラグイン、DuckDB の SQL、手順書 |
| `ruminate-data` | **Private** | 会話ログと復習の記録 |
| dotfiles | 任意 | `~/.config/ruminate/config.toml` |

- コード（公開）と、たまっていく個人データ（非公開）を分ける
- 同じファイルを2か所から書かない: `logs/claude-code/` はパソコンの ruminate、`logs/claude-ai/` は取り込みタスク、`review/` は出題タスクだけが書く。push の前に `pull --rebase` するので、同じ夜に複数が push してもぶつからない
- ruminate は書き込み用の clone を `~/.local/share/ruminate/data` に持ち、普段作業する clone とは分ける
- 名前は「反芻する」の `ruminate`。Claude Code 以外のエージェントにも広げられるよう `claude-` は付けない（`claude-` で始まるプラグイン名は Anthropic 用に予約されている）

## データ形式

JSONL。人が直接読むことはなく、DuckDB（CLI・UI・クライアントアプリ）で読む。

### `logs/<agent>/YYYY/MM/YYYY-MM-DD_HHMM_<project>_<id8>.jsonl`

1会話1ファイル、1行に1ターン。

| 列 | 型 | 内容 |
|---|---|---|
| agent | VARCHAR | `claude-code`、`claude-ai`、`chatgpt` など |
| channel | VARCHAR | `cli` / `web` |
| session | VARCHAR | 会話ID |
| title | VARCHAR | 会話のタイトル |
| project | VARCHAR | リポジトリ名。起動したフォルダから決める（worktree やサブフォルダはもとのリポジトリに寄せる。`src/project.rs`）。Web は空でもよい |
| git_branch | VARCHAR | |
| url | VARCHAR | Web の会話のURL |
| date | TIMESTAMPTZ | ターンの時刻（UTC） |
| seq | INTEGER | 会話の中の順番 |
| role | VARCHAR | `user` / `assistant` |
| text | VARCHAR | 本文（伏せ字済み、長いコードは切り詰め） |
| actions | VARCHAR[] | ツール操作の1行要約（`Edit: src/main.rs`） |

ファイル名は会話の開始時刻と会話IDだけから決まるので、会話が続いても同じファイルを上書きする。

### `review/items.jsonl`（出題タスクが書く・予定）

```json
{"id":"service-binding","question":"Worker 同士を Service Binding でつなぐと、なぜ遅くならない？","answer":"呼び出しがインターネットを経由せず Cloudflare の中で完結するから","source":"…","stage":1,"next_due":"2026-10-28","correct":0,"wrong":1}
```

`stage` は復習の段階（翌日 → 1週間後 → 1か月後）。正解で次へ、間違えると最初に戻る。

### `review/history.jsonl`（予定）

いつ何を出して、どう答えたか。

## Claude Code の会話の読み方

元の会話ファイル（`~/.claude/projects/<project>/<session-id>.jsonl`）の形式は Claude Code の内部仕様で、バージョンによって変わる。[claude-recall](https://github.com/babarot/claude-recall)（MIT）の読み方と、実物の確認（Claude Code 2.1.x）をもとにしている。

- `user` と `assistant` の行だけ読む。それ以外の種類（system、attachment、cost-state…）は黙って飛ばす。新しい種類はよく増える
- `isSidechain`（サブエージェント）、`isMeta`（差し込まれた文脈）、タスク通知、`isCompactSummary`（`/compact` の要約）、`textNotTyped`（ハーネスが代わりに書いた文）は飛ばす
- ツールの実行結果は `user` の行に `tool_result` ブロックとして入っている。`user` の行がすべて本人の発言とは限らない
- `<system-reminder>` タグの中身は本人の発言ではないので取り除く
- Claude の返答は、ブロックやツール呼び出しの前後で行が分かれる。次の本人の発言までを1ターンにまとめる
- `custom-title`（`/rename`）を `ai-title` より優先してタイトルにする
- 元ファイルは追記されるだけ（`/compact` も要約を追記する。claude-recall ADR-003）なので、変わったら丸ごと作り直せばよい
- 読めない行や、`user` / `assistant` なのに形が崩れている行は数えて、多ければ警告する

## エクスポートの読み方

`ruminate import` は claude.ai と ChatGPT の `conversations.json` を読む（`src/source/claude_ai.rs`、`src/source/chatgpt.rs`）。どちらも同じファイル名なので、形式は最初の会話の形（`chat_messages` か `mapping` か）で判別する。

- claude.ai: 会話ごとに `chat_messages` が並ぶ。`sender` が `human` / `assistant`。`content` のブロックのうち `text` と `tool_use` だけ使い、`thinking` と `tool_result` は捨てる。古いエクスポートは `content` がなく `text` だけ。添付（`attachments` / `files`）は名前だけ残し、`extracted_content` は読まない
- ChatGPT: 会話は `mapping` の木。`current_node` から `parent` をたどって、最後に表示していた枝だけを読む。`system` と `tool` の発言、`is_visually_hidden_from_conversation`、`thoughts`（推論）は捨てる。`recipient` が `all` でない assistant の発言はツール呼び出しなので `browser: …` のような1行にする。引用マーカー（私用領域の文字で囲まれた `cite…`）は取り除く。`create_time` が `null` のメッセージは直前の時刻を使う
- どちらも、連続する assistant の発言は1ターンにまとめる（Claude Code と同じ）。本人の発言が1つもない会話は書かない
- Web の会話に project はない。ファイル名は `YYYY-MM-DD_HHMM_<id8>.jsonl`（project の部分を省く）。`url` に会話のリンクを入れる
- `sync` と違って状態は持たない。エクスポートは一度きりのダウンロードなので、渡されたファイルの会話を全部書き、同じ名前のファイルは上書きする

## 送るタイミング

- **SessionEnd フック**: `ruminate hook` がすぐ戻り、切り離したプロセスで数秒待ってから `sync` する。SessionEnd は標準 1.5 秒（最大 60 秒）しか待ってもらえず、会話ファイルの書き込みも少し遅れるため
- **SessionStart フック**: Ctrl+C やターミナルを閉じたときに SessionEnd が動く保証がないので、次の起動時に取りこぼしを回収する
- 会話ファイルごとにサイズと更新時刻を記録し、変わったものだけ処理する。ロックをかけるので、同時に動いても大丈夫

## 送らないもの

ツールの実行結果、思考（thinking）、画像、サブエージェントの会話。秘密情報が混ざりやすく、復習にも要らないため。APIキーやトークン、電話番号らしい文字列は伏せるが完全ではないので、仕事のプロジェクトなどは `exclude_projects` で最初から除外する。

## 新しく使う人の手順（目標）

1. `cargo install` → `ruminate init`（`gh` でデータリポジトリ作成、設定ファイル、これまでの会話の変換）
2. Claude Code で `/plugin marketplace add` と `/plugin install`
3. claude.ai でセットアップ用スキル（`ruminate-setup`・予定）を入れて「ruminateをセットアップして」と頼む。取り込みタスクと出題タスクを、内容を見せて承認をもらってから作る

手でやるしかないこと: プラグインのインストール、claude.ai から GitHub に入る許可、タスク作成の承認、「Search and reference chats」の設定。手順は `prompts/README.md`。

## まだやっていないこと

- `ruminate import` の DeepSeek 対応（`Export` トレイトの実装を1つ足す）
- 出題タスクの手順書（`prompts/`）と、セットアップ用スキル。取り込みタスクの手順書は `prompts/claude-ai-ingest.md` にある（ファイル名は import と同じ、時刻は JST 固定、直近 7 日を毎回見直す）
- スケジュールで動くセッションから、非公開リポジトリと過去チャット検索が使えるかの確認
- Codex CLI など、ほかのターミナルエージェントの読み込み
