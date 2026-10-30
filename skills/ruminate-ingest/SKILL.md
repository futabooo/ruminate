---
name: ruminate-ingest
description: claude.ai の最近の会話を過去チャット検索で集め、GitHub の ruminate-data リポジトリの logs/claude-ai/ に JSONL として書く。「ruminate 取り込み」「会話を ruminate-data に入れて」と頼まれたときに使う。
---

# claude.ai の会話を ruminate-data に取り込む

このスキルは、本人が claude.ai のチャットで「ruminate 取り込み」と頼んだときに動きます。
過去チャット検索でこの数日の会話を集め、GitHub コネクタで `ruminate-data`（非公開
リポジトリ）の `logs/claude-ai/` に 1 会話 1 ファイルの JSONL を書きます。

スケジュールタスクではなく、本人がいるチャットで動くので、最初に対象の会話の一覧を
見せて、書く前に一言確認を取ります。それ以外は質問せず、迷った会話は飛ばします。

書き込む場所は `logs/claude-ai/` の中だけです。`logs/claude-code/` や `review/` には触りません。

## 時刻の扱い

- **ファイル名の時刻は JST（UTC+9）** です。パソコン側の `ruminate sync` が Mac の
  ローカル時刻で付けているのと揃えるためで、ここでは JST に固定します
- **行の中の `date` は UTC** で、`2026-10-20T03:00:01Z` のように `Z` を付けます
- 会話の時刻が UTC で得られるなら、ファイル名用に 9 時間足して JST にします

## 手順

### 1. 対象の会話を決める

本人が期間を言わなければ、**直近 7 日間**（JST の今日を含めて 7 日）に作成または更新
された会話をすべて対象にします。「先月分」「10 月 20 日から」と言われたらその期間です。
前回どこまで入れたかは見ません。同じ会話は同じファイル名になるので、もう一度書いても
上書きされるだけで、重複しません。

過去チャット検索（`recent_chats` や `conversation_search`）で、更新日時がこの範囲に
入る会話を列挙します。一度に返る件数に限りがあるなら、日付や話題を変えて何度か検索し、
会話 ID（URL の `/chat/` の後ろ）で重複を除きます。

次の会話は書きません。

- いま動いているこのチャット、および過去の「ruminate 取り込み」のチャット
- 本人の発言が 1 つもない会話

一覧（タイトルと日付）を見せて、「この N 件を書きます」と確認を取ります。

### 2. 会話を JSONL にする

1 会話 1 ファイル、1 行 1 ターン。

| 列 | 値 |
|---|---|
| agent | `claude-ai` |
| channel | `web` |
| session | 会話 ID（URL の `/chat/` の後ろ） |
| title | 会話のタイトル。なければ `null` |
| project | `""`（空文字） |
| git_branch | `null` |
| url | `https://claude.ai/chat/<会話 ID>` |
| date | そのターンの時刻、UTC |
| seq | 0 から始まる連番 |
| role | `user` または `assistant` |
| text | 本文 |
| actions | ツール操作の 1 行要約の配列。なければ省略 |

例:

```json
{"agent":"claude-ai","channel":"web","session":"c0ffee00-1111-2222-3333-444444444444","title":"DuckDB で JSONL を読む","project":"","git_branch":null,"url":"https://claude.ai/chat/c0ffee00-1111-2222-3333-444444444444","date":"2026-10-20T03:00:01Z","seq":0,"role":"user","text":"DuckDB で JSONL のフォルダをまとめて読むには？","actions":["attachment: sample.jsonl"]}
{"agent":"claude-ai","channel":"web","session":"c0ffee00-1111-2222-3333-444444444444","title":"DuckDB で JSONL を読む","project":"","git_branch":null,"url":"https://claude.ai/chat/c0ffee00-1111-2222-3333-444444444444","date":"2026-10-20T03:00:05Z","seq":1,"role":"assistant","text":"`read_json('logs/**/*.jsonl', format = 'newline_delimited')` で読めます。","actions":["web_search: duckdb read_json glob"]}
```

ターンの作り方:

- 本人の発言 1 つにつき `user` を 1 行。続く Claude の返答は、ツール呼び出しで
  分かれていても 1 つの `assistant` にまとめ、文章は空行で区切ってつなげる
- 残すのは本人の発言と返答の文章だけ。思考、ツールの実行結果、画像、添付ファイルの
  中身は入れない
- ツール呼び出しは `web_search: duckdb read_json glob` のように「ツール名: 要点」を
  1 行にして `actions` に入れる。要点は 80 文字まで
- 添付ファイルは `attachment: ファイル名` を `user` の `actions` に入れる。中身は読まない
- 40 行を超えるコードブロックは先頭 40 行だけ残し、`… (N lines omitted)` を足す
- 正確な時刻が分からないターンは、直前のターンの時刻を使う
- 検索結果が抜粋しか返さず全文が取れないときは、取れた範囲をそのまま書く。
  **足りない部分を推測して埋めない**。本人には「全文が取れなかった会話」を伝える

### 3. 伏せ字

次に当てはまる文字列は `[REDACTED]` に置き換えます。`text`、`title`、`actions` すべてが
対象です。

- `sk-`、`ghp_`、`github_pat_`、`AKIA`、`xoxb-`、`AIza` などで始まる API キーやトークン
- `-----BEGIN ... PRIVATE KEY-----` から `END` までのブロック
- `Bearer` に続くトークン、JWT（`eyJ…​.eyJ…​.…`）
- 日本の電話番号（`090-1234-5678`、`+81 90 1234 5678`、`03-1234-5678`、`0120-…`）
- `api_key=…`、`password: …`、`token=…` のような「名前 = 値」の値の部分

迷ったら伏せます。伏せすぎて困ることはありません。

### 4. GitHub に書く

パス: `logs/claude-ai/YYYY/MM/YYYY-MM-DD_HHMM_<id8>.jsonl`

- `YYYY-MM-DD_HHMM` は **会話の作成時刻を JST にしたもの**
- `<id8>` は会話 ID から英数字だけを取り出した先頭 8 文字
- 同じパスがすでにあれば、中身を丸ごと置き換える（追記しない）

例: 2026-10-20 03:00 UTC に始まった会話 `c0ffee00-1111-…` は
`logs/claude-ai/2026/10/2026-10-20_1200_c0ffee00.jsonl`（JST では 12:00）。

GitHub コネクタの「ファイルを作成または更新」で、リポジトリ `ruminate-data`、ブランチ
`main` に 1 ファイルずつ書きます。コミットメッセージは
`ruminate: ingest <タイトル>`。ブランチや PR は作りません。

### 5. 終わったら

書いた件数と、全文が取れなかった会話があればその一覧を、短く伝えます。
本人はパソコンで `ruminate db` を開けば、`claude-ai` の会話として読めます。
