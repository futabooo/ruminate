# たまった会話から復習問題を出す（出題タスク）

> **この文書を読むのは、claude.ai のスケジュールタスク（リポジトリに `ruminate-data` を
> 選んだ Claude Code タスク）として動く Claude です。** 人はこの文書をタスクの指示欄に
> 貼るだけで、手順を自分で実行することはありません。以下の「あなた」はタスクとして動く
> Claude、「本人」はこのリポジトリの持ち主です。

あなたは毎晩 1 回動き、このリポジトリ（`ruminate-data`）にたまった会話から復習問題を
出します。問題は GitHub の Issue として立て、本人は Issue のコメントで採点を返します。
前回の Issue のコメントを読んで記録し、今回の Issue を立てます。
出す問題があるときだけ Issue を立て、ないときは何もしません。

この環境には claude.ai の過去チャット検索はありません。読むのはこのリポジトリの
ファイルと Issue だけです。書き込むのは `review/` の中と Issue だけで、`logs/` には
触りません。

出題は『科学的根拠に基づく最強の勉強法』の方法に沿います。

- **アクティブリコール**: 読み返させず、思い出させる問いにする
- **分散学習**: 正解した問題ほど間を空けて出し直す（翌日 → 1 週間後 → 1 か月後）
- **交互学習**: 1 回の中で別の話題を混ぜる
- **自己説明**: 「なぜ」「どう違う」「何が起きる」の形で問い、理由を言わせる
- **すぐのフィードバック**: 答えは同じ Issue の下のほうに入れる。本人は考えてから見て、自己採点する

## 時刻の扱い

- 日付（`next_due`、`asked_at` など）は **JST（UTC+9）の日付** を `2026-10-29` の形で書きます
- `logs/` の各行の `date` は UTC です。会話の日付を判断するときは 9 時間足して JST にします

## ファイル

### `review/items.jsonl`（1 行 1 問）

```json
{"id":"service-binding","question":"Worker 同士を Service Binding でつなぐと、なぜ遅くならない？","answer":"呼び出しがインターネットを経由せず Cloudflare の中で完結するから","source":"claude-code/c0ffee00","topic":"cloudflare","stage":1,"next_due":"2026-10-30","correct":0,"wrong":1,"created_at":"2026-10-28","asked_at":"2026-10-29","issue":12}
```

| 列 | 内容 |
|---|---|
| id | 短い英数字とハイフン。ファイル内で一意 |
| question | 問い。1〜2 文 |
| answer | 答え。2〜3 文。会話の中で本人が納得した説明を使う |
| source | `<agent>/<会話 ID の先頭 8 文字>` |
| topic | 話題の短い名前。交互学習でばらすのに使う |
| stage | 0: 新規、1: 翌日、2: 1 週間後、3: 1 か月後、4: 卒業（出さない） |
| next_due | 次に出す日（JST） |
| correct / wrong | 累計 |
| created_at | 作った日（JST） |
| asked_at / issue | 最後に出した日と、その Issue 番号 |

同じ `id` の行が複数あってはいけません。更新するときは、その行を書き換えます。

### `review/history.jsonl`（1 行 1 出題）

```json
{"date":"2026-10-29","item":"service-binding","result":"correct","issue":12}
```

`result` は `correct`、`wrong`、`unanswered` のどれか。追記だけで、書き換えません。

## 手順

### 1. 準備

リポジトリはすでに checkout されています。`review/items.jsonl` と `review/history.jsonl`
がなければ空で作ります。

### 2. 前回の採点を記録する

ラベル `ruminate-review` の **開いている Issue** を探します。

```sh
gh issue list --label ruminate-review --state open --json number,title,createdAt
```

それぞれの Issue について、本人のコメントを読みます。

```sh
gh issue view <番号> --comments
```

コメントは `1 ○ 2 × 3 ○` のような採点です。○ / ×、o / x、正解 / 不正解、◯ / ✕ はどれも
同じ意味に読みます。複数のコメントがあれば最後のものを使います。

その Issue で出した問題（`items.jsonl` の `issue` がその番号の行）ごとに:

- **○**: `history.jsonl` に `correct` を追記。`items.jsonl` の `correct` を 1 増やし、`stage` を
  1 つ進め、`next_due` を stage に合わせて進める（1: 翌日、2: 7 日後、3: 30 日後、4: 出さない）
- **×**: `wrong` を追記。`wrong` を 1 増やし、`stage` を 1 に戻し、`next_due` を翌日にする
- **採点がない**（コメントなし、またはその番号への採点がない）: `unanswered` を追記。
  `items.jsonl` は変えない。その問題は今回もう一度出す

記録したら Issue を閉じます。

```sh
gh issue close <番号> --comment "記録しました。○ 2 / × 1"
```

同じ Issue を 2 回記録しないよう、`history.jsonl` にその Issue 番号の行がすでにあれば
書かずに閉じるだけにします。

### 3. 今回出す問題を選ぶ

1. **期限が来た問題**: `items.jsonl` で `stage` が 1〜3 かつ `next_due` が今日以前のもの。
   `next_due` が古い順
2. 前の手順で `unanswered` にした問題
3. 1 と 2 で 3 問に足りなければ、**新しい問題を作る**（次の手順）

出すのは **3 問**。期限が来た問題が多い日は **最大 5 問** まで。残りは翌日に回します。
同じ `topic` が続かないよう並べ替えます。

### 4. 新しい問題を作る

直近 7 日（JST）に始まった会話を `logs/` から読みます。`logs/<agent>/YYYY/MM/*.jsonl` の
1 行 1 ターンで、`role` が `user` と `assistant`、`text` が本文、`session` が会話 ID です。
DuckDB があれば `duckdb/init.sql` 相当のビューを使っても構いませんが、`jq` や普通の
テキスト処理で十分です。

作り方:

- 本人が質問して理解した事柄、本人が決めたことの理由、調べて分かった仕様などから選ぶ
- 1 つの会話から一度に作るのは 1 問まで。`items.jsonl` の `source` にすでにある会話は、
  別の話題が残っていない限り使わない
- 単語の暗記ではなく、理由や仕組みを言わせる問いにする
- 答えは会話の中の説明を要約する。会話にない知識を足さない
- `stage` は 0、`next_due` は今日、`created_at` は今日で `items.jsonl` に追記する

作れる会話がなければ、作りません。問題が 1 つもなければ Issue は立てず、`review/` を
変更したときだけ手順 6 を行って終わります。

### 5. Issue を立てる

タイトルは「ruminate 復習 YYYY-MM-DD」（JST）、ラベルは `ruminate-review`（なければ作る）。
本人はこれを GitHub の通知で読み、コメントに採点だけを書きます。

```sh
gh label create ruminate-review --color 5319e7 --description "ruminate の復習問題" 2>/dev/null || true
gh issue create --label ruminate-review --title "ruminate 復習 2026-10-29" --body-file /tmp/issue.md
```

本文の形:

```markdown
考えてから、下の答えを見て採点してください。コメントは「1 ○ 2 × 3 ○」の形で。

1. Worker 同士を Service Binding でつなぐと、なぜ遅くならない？
2. Claude Code の会話ファイルで、user の行が本人の発言とは限らないのはなぜ？
3. git の linked worktree から元のリポジトリ名を知る方法は？

<details><summary>答え</summary>

1. 呼び出しがインターネットを経由せず Cloudflare の中で完結するから。
2. ツールの実行結果も tool_result ブロックとして user の行に入るから。
3. git rev-parse --git-common-dir が主リポジトリの .git を指すので、その親フォルダ名。

</details>
```

立てたら、出した問題の `items.jsonl` の行に `asked_at`（今日）と `issue`（番号）を書きます。
`history.jsonl` には **まだ書きません**。採点は翌晩の手順 2 で記録します。

### 6. 変更をリポジトリに入れる

```sh
git add review
git -c user.name=ruminate-review -c user.email=ruminate-review@users.noreply.github.com \
    commit -q -m "ruminate: review YYYY-MM-DD"
git pull -q --rebase origin main
git push -q origin HEAD:main
```

`main` に直接 push できなければ、作業ブランチを push して PR を作り、自分でマージします。

```sh
git push -q -u origin HEAD
gh pr create --fill --title "ruminate: review YYYY-MM-DD" --body "出題タスクの記録"
gh pr merge --squash --delete-branch
```

変更がなければ commit も push もしません。ほかの書き手（パソコンの `ruminate sync`、
`ruminate import`）は `logs/` に書くので、`review/` が衝突することはありません。

## してはいけないこと

- 本人に質問したり、採点以外の返事を求めたりしない
- `logs/` を書き換えない
- Issue 以外の場所（PR のコメントなど）に問題を書かない
- 会話の本文を長く引用しない。問いと答えは自分の言葉で短く書く
- 出す問題がない日に、空の Issue を立てない
