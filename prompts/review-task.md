# たまった会話から復習問題を出す（出題タスク）

> **この文書を読むのは、claude.ai のスケジュールタスク（リポジトリに `ruminate-data` を
> 選んだ Claude Code タスク）として動く Claude です。** 人はこの文書をタスクの指示欄に
> 貼るだけで、手順を自分で実行することはありません。以下の「あなた」はタスクとして動く
> Claude、「本人」はこのリポジトリの持ち主です。

あなたは毎晩 1 回動き、このリポジトリ（`ruminate-data`）にたまった会話から復習問題を
出します。問題は GitHub の Issue として立て、本人は Issue のコメントに自分の答えを文章で
書きます。あなたは前回の Issue のコメントを読んで採点し、講評をコメントして記録し、
今回の Issue を立てます。出す問題があるときだけ Issue を立て、ないときは何もしません。

この環境には claude.ai の過去チャット検索はありません。読むのはこのリポジトリの
ファイルと Issue だけです。書き込むのは `review/` の中と Issue だけで、`logs/` には
触りません。

出題は『科学的根拠に基づく最強の勉強法』の方法に沿います。

- **アクティブリコール**: 読み返させず、思い出させる問いにする
- **分散学習**: 正解した問題ほど間を空けて出し直す（翌日 → 1 週間後 → 1 か月後）
- **交互学習**: 1 回の中で別の話題を混ぜる
- **自己説明**: 「なぜ」「どう違う」「何が起きる」の形で問い、理由を自分の言葉で書かせる
- **すぐのフィードバック**: 模範解答は同じ Issue の折りたたみに入れる。本人は答えを書いてから
  開いて確かめられる。正式な採点と講評は翌晩のあなたのコメント

## 時刻の扱い

- 日付（`next_due`、`asked_at` など）は **JST（UTC+9）の日付** を `2026-10-29` の形で書きます
- `logs/` の各行の `date` は UTC です。会話の日付を判断するときは 9 時間足して JST にします

## ファイル

### `review/items.jsonl`（1 行 1 問）

```json
{"id":"service-binding","question":"Worker 同士を Service Binding でつなぐと、なぜ遅くならない？","answer":"呼び出しがインターネットを経由せず Cloudflare の中で完結するから","source":"claude-code/c0ffee00","topic":"cloudflare","kind":"mechanism","stage":1,"next_due":"2026-10-30","correct":0,"wrong":1,"created_at":"2026-10-28","asked_at":"2026-10-29","issue":12}
```

| 列 | 内容 |
|---|---|
| id | 短い英数字とハイフン。ファイル内で一意 |
| question | 問い。1〜2 文 |
| answer | 答え。2〜3 文。会話の中で本人が納得した説明を使う |
| source | `<agent>/<会話 ID の先頭 8 文字>` |
| topic | 話題の短い名前。交互学習でばらすのに使う |
| kind | `mechanism`（仕組み）、`decision`（判断）、`procedure`（手順）のどれか |
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

コメントには本人の答えが番号付きの文章で書いてあります。あなたが採点します。
本人以外（あなた自身の過去のコメント）は読みません。同じ番号に複数の答えがあれば
最後のものを使います。

その Issue で出した問題（`items.jsonl` の `issue` がその番号の行）ごとに、本人の答えを
`items.jsonl` の `answer` と照らして判定します。

- **○**: 答えの核心（なぜそうなるか、何と何が違うか）を自分の言葉で押さえている。
  言い回しや細部の違いは問わない
- **×**: 核心が違う、核心に触れていない、または `○` `わからない` のような一言だけで
  説明がない。本人が `1 ○` のように自己採点だけ書いた場合も、説明がなければ × にする
- **未回答**: コメントがない、またはその番号への答えがない

記録:

- **○**: `history.jsonl` に `correct` を追記。`items.jsonl` の `correct` を 1 増やし、`stage` を
  1 つ進め、`next_due` を stage に合わせて進める（1: 翌日、2: 7 日後、3: 30 日後、4: 出さない）
- **×**: `wrong` を追記。`wrong` を 1 増やし、`stage` を 1 に戻し、`next_due` を翌日にする
- **未回答**: `unanswered` を追記。`items.jsonl` は変えない。その問題は今回もう一度出す

採点と講評をコメントして、Issue を閉じます。講評は問題ごとに 1〜2 文。○ なら何が
押さえられていたか、× なら何が抜けていたか・どう違うかを、模範解答を引きながら書きます。
責めず、次に思い出せるような言い方にします。

```sh
gh issue close <番号> --body-file /tmp/grading.md
```

講評の形:

```markdown
採点: ○ 2 / × 1

1. ○ Conscrypt APEX を見ないという核心が書けています。
2. × 「別プロセス」までは合っていますが、人間が承認ダイアログに直接答えられる点が抜けています。herdr のエージェントは会話が見え、親が落ちても残るのが違いです。
3. ○ 追跡設定まで自動になる、で正解です。
```

同じ Issue を 2 回記録しないよう、`history.jsonl` にその Issue 番号の行がすでにあれば
書かずに閉じるだけにします。

### 3. 今回出す問題を選ぶ

1. **期限が来た問題**: `items.jsonl` で `stage` が 1〜3 かつ `next_due` が今日以前のもの。
   `next_due` が古い順
2. 前の手順で `unanswered` にした問題
3. 1 と 2 で 3 問に足りなければ、**新しい問題を作る**（次の手順）

出すのは **3 問**。期限が来た問題が多い日は **最大 5 問** まで。残りは翌日に回します。
同じ `topic` が続かないよう並べ替え、`kind` もできるだけ混ぜます。

### 4. 新しい問題を作る

直近 7 日（JST）に始まった会話を `logs/` から読みます。`logs/<agent>/YYYY/MM/*.jsonl` の
1 行 1 ターンで、`role` が `user` と `assistant`、`text` が本文、`session` が会話 ID です。
DuckDB があれば `duckdb/init.sql` 相当のビューを使っても構いませんが、`jq` や普通の
テキスト処理で十分です。

問いの型は 3 つ。1 晩の中で型も混ぜます（交互学習）。

| 型 | 問いの形 | 材料 |
|---|---|---|
| 仕組み | 「なぜそうなる？」「X と Y はどう違う？」 | 本人が質問して理解した事柄、調べて分かった仕様 |
| 判断 | 「なぜ A ではなく B にした？」「B の利点と欠点は？」「どんな条件なら A に戻す？」 | コードや設計を変えた会話。`actions` に `Edit:` や `Write:` があり、返答の文章に理由や比較が書いてあるもの |
| 手順 | 「X するにはどうする？」「X が失敗したら何を見る？」 | つまずいて解決した会話 |

「判断」の問いは、答えに **理由と、比べた案との違い（利点・欠点）** を含めます。例:

- 問: ruminate の会話ログで、Markdown を置かずに JSONL + DuckDB にしたのはなぜ？ Markdown にした場合の利点と欠点は？
- 答: 人が直接読まず DuckDB で横断検索するので、整形の手間がない JSONL でよい。Markdown は GitHub 上でそのまま読める利点があるが、1 会話 2 ファイルになり、集計や出題タスクからの読み込みに変換が要る。

作り方:

- 本人が質問して理解した事柄、本人が決めたことの理由、調べて分かった仕様などから選ぶ
- 判断の問いは、理由や比較が **会話に書いてあるときだけ** 作る。変更の行（`Edit:`）だけで理由が書かれていない会話から、理由を推測して問題にしない
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
答えを番号付きの文章でコメントしてください（例: 「1. 〜だから。2. 〜」）。
書いたあとで下の模範解答を開いて確かめて構いません。採点と講評は明日の夜にコメントします。

1. Worker 同士を Service Binding でつなぐと、なぜ遅くならない？
2. Claude Code の会話ファイルで、user の行が本人の発言とは限らないのはなぜ？
3. git の linked worktree から元のリポジトリ名を知る方法は？

<details><summary>模範解答（答えを書いてから開く）</summary>

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

- 本人に質問したり、答え以外の返事を求めたりしない
- 本人の答えを甘く採点しない。核心が抜けていれば × にして、何が抜けたかを講評で伝える
- `logs/` を書き換えない
- Issue 以外の場所（PR のコメントなど）に問題を書かない
- 会話の本文を長く引用しない。問いと答えは自分の言葉で短く書く
- 出す問題がない日に、空の Issue を立てない
