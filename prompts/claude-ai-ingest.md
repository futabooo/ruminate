# claude.ai の会話を ruminate-data に取り込む（取り込みタスク）

> **この文書を読むのは、claude.ai のスケジュールタスクとして動く Claude です。**
> 人はこの文書をタスクの指示欄に貼るだけで、手順を自分で実行することはありません。
> 以下の「あなた」はタスクとして動く Claude のことです。

あなたは毎日 1 回動き、この claude.ai アカウントでした会話を `ruminate-data`
（非公開リポジトリ）の `logs/claude-ai/` に書きます。
人に通知はしません。質問も出しません。判断に迷ったら、その会話は書かずに飛ばします。

書き込む場所は `logs/claude-ai/` の中だけです。ほかのフォルダ（`logs/claude-code/`、
`review/`）には触りません。

## 時刻の扱い

- **ファイル名の時刻は JST（UTC+9）** です。パソコン側の `ruminate sync` が Mac の
  ローカル時刻で付けているのと揃えるためで、ここでは JST に固定します
- **行の中の `date` は UTC** で、`2026-10-20T03:00:01Z` のように `Z` を付けます
- 会話の時刻が UTC で得られるなら、ファイル名用に 9 時間足して JST にします

## 手順

### 1. リポジトリを用意する

```sh
git clone <ruminate-data の URL> data
cd data
```

### 2. 対象の会話を決める

直近 **7 日間**（JST の今日を含めて 7 日）に作成または更新された会話をすべて対象に
します。前回の実行で書いたかどうかは見ません。同じ会話は同じファイル名になるので、
もう一度書いても上書きされるだけで、重複しません。これが取りこぼし防止です。

過去チャット検索で、更新日時がこの範囲に入る会話を列挙します。検索が一度に返す件数に
限りがあるなら、日付や話題を変えて何度か検索し、会話 ID で重複を除きます。

次の会話は書きません。

- このタスク自身の会話、および `ruminate` の出題タスクの会話
- 本人の発言が 1 つもない会話

### 3. 会話を JSONL にする

1 会話 1 ファイル、1 行 1 ターン。列は `docs/design.md` の表のとおりです。

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

ターンの作り方は `ruminate import` と同じです。

- 本人の発言 1 つにつき `user` を 1 行。続く Claude の返答は、ツール呼び出しで
  分かれていても 1 つの `assistant` にまとめ、文章は空行で区切ってつなげる
- 残すのは本人の発言と返答の文章だけ。思考、ツールの実行結果、画像、添付ファイルの
  中身は入れない
- ツール呼び出しは `web_search: duckdb read_json glob` のように「ツール名: 要点」を
  1 行にして `actions` に入れる。要点は 80 文字まで
- 添付ファイルは `attachment: ファイル名` を `user` の `actions` に入れる。中身は読まない
- 40 行を超えるコードブロックは先頭 40 行だけ残し、`… (N lines omitted)` を足す
- 正確な時刻が分からないターンは、直前のターンの時刻を使う

### 4. 伏せ字

次に当てはまる文字列は `[REDACTED]` に置き換えます。`text`、`title`、`actions` すべてが
対象です。

- `sk-`、`ghp_`、`github_pat_`、`AKIA`、`xoxb-`、`AIza` などで始まる API キーやトークン
- `-----BEGIN ... PRIVATE KEY-----` から `END` までのブロック
- `Bearer` に続くトークン、JWT（`eyJ…​.eyJ…​.…`）
- 日本の電話番号（`090-1234-5678`、`+81 90 1234 5678`、`03-1234-5678`、`0120-…`）
- `api_key=…`、`password: …`、`token=…` のような「名前 = 値」の値の部分

迷ったら伏せます。伏せすぎて困ることはありません。

### 5. ファイルに書く

パス: `logs/claude-ai/YYYY/MM/YYYY-MM-DD_HHMM_<id8>.jsonl`

- `YYYY-MM-DD_HHMM` は **会話の作成時刻を JST にしたもの**
- `<id8>` は会話 ID から英数字だけを取り出した先頭 8 文字
- 同じ名前のファイルがあれば、中身を丸ごと置き換える（追記しない）

例: 2026-10-20 03:00 UTC に始まった会話 `c0ffee00-1111-…` は
`logs/claude-ai/2026/10/2026-10-20_1200_c0ffee00.jsonl`（JST では 12:00）。

### 6. push する

```sh
git add logs/claude-ai
git -c user.name=ruminate-ingest -c user.email=ruminate-ingest@users.noreply.github.com \
    commit -q -m "ruminate: ingest N conversation(s) from claude-ai"
git pull -q --rebase
git push -q
```

変更がなければ commit も push もしません。ほかの書き手は別のフォルダに書くので、
`pull --rebase` が衝突することは普通ありません。もし衝突したら、取り込んだ内容のほうを
残します（rebase 中は `git checkout --theirs -- <ファイル>` が自分の変更です）。

## 終わったら

何も出力しなくて構いません。人に知らせるのは出題タスクの役目です。

失敗したときも通知はしません。次の日の実行が、同じ 7 日の範囲をもう一度見ます。
