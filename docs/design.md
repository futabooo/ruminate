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
  公式エクスポート ──────import────────┤
claude.ai: Skill ruminate-ingest       │
  本人が頼んだとき、過去チャット検索 ──┤
  → GitHub コネクタで書く               ▼
                               ruminate-data（Private）
                                 ├─ logs/<agent>/…jsonl
                                 └─ review/…jsonl
claude.ai: 出題タスク（毎晩、Claude Code タスク）<──┘
  logs/ を読んで出題 → GitHub Issue → 本人が答えをコメント → 翌晩 採点・講評して review/ に記録
```

| 役割 | どこで動く | 何をする |
|---|---|---|
| `ruminate`（このリポジトリ） | パソコン | ターミナルの会話を `ruminate-data` に送る（`sync`）。公式エクスポートの取り込み（`import`） |
| Skill `ruminate-ingest` | claude.ai の普通のチャット | 本人が「ruminate 取り込み」と頼んだとき、直近の会話を過去チャット検索で集め、GitHub コネクタで `logs/claude-ai/` に書く |
| 出題タスク | claude.ai のスケジュール（リポジトリに `ruminate-data` を選んだ Claude Code タスク） | `ruminate-data` だけを読んで GitHub Issue に出題し、前回の採点を `review/` に書く |

claude.ai の会話は、スケジュールタスクからは読めない（2026-10-30 に確認: タスクの環境には過去チャット検索がなく、リポジトリを選んでも選ばなくても同じ）。一方、本人がいる普通のチャットには検索があり、GitHub コネクタは `main` に直接ファイルを書ける（同日に確認）。そこで取り込みは Skill にして、本人が一言頼む形にする。検索は抜粋しか返さないことがあるので、全文が要るときは公式エクスポートを `ruminate import` で入れる。ファイル名が同じなので両方使っても重複しない。

## リポジトリ

| リポジトリ | 公開 | 中身 |
|---|---|---|
| `ruminate` | Public | Rust 本体、Claude Code プラグイン、DuckDB の SQL、手順書 |
| `ruminate-data` | **Private** | 会話ログと復習の記録 |
| dotfiles | 任意 | `~/.config/ruminate/config.toml` |

- コード（公開）と、たまっていく個人データ（非公開）を分ける
- 同じファイルを2か所から書かない: `logs/claude-code/` はパソコンの `sync`、`logs/claude-ai/` は Skill と `import`（同じ会話は同じファイル名なので上書きになるだけ）、`review/` は出題タスクだけが書く。push の前に `pull --rebase` するので、同じ夜に複数が push してもぶつからない
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

### `review/items.jsonl`（出題タスクが書く）

```json
{"id":"service-binding","question":"Worker 同士を Service Binding でつなぐと、なぜ遅くならない？","answer":"呼び出しがインターネットを経由せず Cloudflare の中で完結するから","source":"claude-code/c0ffee00","topic":"cloudflare","stage":1,"next_due":"2026-10-30","correct":0,"wrong":1,"created_at":"2026-10-28","asked_at":"2026-10-29"}
```

`stage` は復習の段階（0 新規 → 1 翌日 → 2 1週間後 → 3 1か月後 → 4 卒業）。正解で次へ、間違えると 1 に戻る。日付は JST。列の意味は `prompts/review-task.md`。

### `review/history.jsonl`（出題タスクが書く）

```json
{"date":"2026-10-29","item":"service-binding","result":"correct"}
```

いつ何を出して、どう答えたか（`correct` / `wrong` / `unanswered`）。追記のみ。

出題は 1 晩 3 問（期限切れが多ければ最大 5 問）。問題は `ruminate-data` の GitHub Issue として立て（ラベル `ruminate-review`）、答えは同じ Issue の折りたたみに入れる。本人は答えを文章でコメントし（書いたあとで模範解答を開いて確かめてよい）、翌晩のタスクがそれを模範解答と照らして採点、講評をコメントして記録し、Issue を閉じる。自己採点だけの案は、本人が「文章で答えて採点してほしい」と希望したので取らない。採点をその場で返すには `ruminate-data` に `claude-code-action` のワークフローを置く（`templates/ruminate-data/`）。認証は Workload Identity Federation で、リポジトリに静的な鍵を置かない（OAuth トークンはアカウント全体の長寿命の鍵なので避ける）。Action は commit SHA で固定する。Issue のコメントで起動し、採点・講評・記録・クローズまで行う。夜のタスクは開いている Issue だけ見るので、両方あっても二重に採点しない。答えを書く手間はあるが、自己説明そのものなので学習効果は高い。通知は GitHub のものを使う。claude.ai の会話で返事を受ける案は、タスクに過去チャット検索がないので取れない。答えを翌晩まで伏せる案は、フィードバックが 1 日遅れるのと返事の手間で続かないので取らない。

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
3. claude.ai のスケジュール済みタスクに出題タスクを作る（`prompts/README.md`）。将来はセットアップ用スキル（`ruminate-setup`・予定）で、内容を見せて承認をもらってから作る

手でやるしかないこと: プラグインのインストール、claude.ai から GitHub に入る許可、タスク作成の承認、「Search and reference chats」の設定。手順は `prompts/README.md`。

## まだやっていないこと

- `ruminate import` の DeepSeek 対応（`Export` トレイトの実装を1つ足す）
- 出題タスクを claude.ai で一度動かして確かめる（`prompts/review-task.md`。Issue が立つか、`main` に push できるか PR になるか）
- セットアップ用スキル（`ruminate-setup`）
- スケジュールで動くセッションから、非公開リポジトリと過去チャット検索が使えるかの確認
- Codex CLI など、ほかのターミナルエージェントの読み込み
