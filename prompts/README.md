# claude.ai のタスクを作る

このフォルダには、claude.ai のスケジュールタスクに貼る手順書が入っています。
手順書の読み手はタスクとして動く Claude です。人がやるのは、ここに書いた
準備とタスクの作成だけです。

| 手順書 | タスク | 何をする |
|---|---|---|
| [claude-ai-ingest.md](claude-ai-ingest.md) | 取り込みタスク（毎日） | claude.ai の新しい会話を `ruminate-data` の `logs/claude-ai/` に書く |
| [claude-ai-review.md](claude-ai-review.md) | 出題タスク（毎晩） | `ruminate-data` を読んで復習問題を 3 問出し、前回の採点を `review/` に書く |

claude.ai の画面の文言や場所は更新で変わります。見つからなければ設定画面の検索で
「GitHub」「Scheduled」「Search and reference chats」を探してください。

## 準備（最初の 1 回）

1. **GitHub 連携を許可する**。設定 → コネクタ（Connectors / Integrations）で GitHub を
   つなぎ、データリポジトリ（例: `<あなたのアカウント>/ruminate-data`）へのアクセスを
   許可する
2. **過去チャット検索をオンにする**。設定 → 機能（Capabilities）の
   「Search and reference chats」。これがないとタスクが会話を読めない

## 取り込みタスクを作る

1. claude.ai のスケジュールタスク（Scheduled tasks）で新規作成
2. 名前: `ruminate ingest` など
3. 頻度: 毎日。出題タスクより前に動くよう、深夜（JST）の早い時間にする。例: 2:00
4. 指示: [claude-ai-ingest.md](claude-ai-ingest.md) の中身をそのまま貼る。
   `<ruminate-data の URL>` をデータリポジトリの URL
   （例: `https://github.com/<あなたのアカウント>/ruminate-data.git`）に置き換える
5. 作成したら一度手動で実行し、`ruminate-data` の `logs/claude-ai/` にファイルが
   増えるか見る
6. 手元で `ruminate db` を開き、claude-ai の会話が読めるか確かめる

```sql
SELECT agent, count(*) AS sessions FROM sessions GROUP BY 1;
```

`ruminate import` で公式エクスポートを入れてあれば、同じ会話は同じファイル名に
上書きされます。ファイルが 2 つになっていたら、時刻（JST）か ID の切り出しが
手順書とずれているので、タスクの出力を見て手順書を直してください。

## 出題タスクを作る

取り込みタスクが一度動いたのを確かめてから作ります。

1. スケジュールタスクを新規作成。名前: `ruminate 復習` など
2. 頻度: 毎晩。取り込みタスクの **あと**、本人が通知を見られる時間。例: 21:00（JST）
3. 指示: [claude-ai-review.md](claude-ai-review.md) の中身をそのまま貼る。
   `<ruminate-data の URL>` を置き換える
4. 一度手動で実行する。初回は `review/items.jsonl` が空なので、直近の会話から 3 問作って
   出すはず。通知がスマホに届くか見る
5. 返事に `1 ○ 2 × 3 ○` のように採点を書く
6. 翌晩の実行後、`ruminate-data` の `review/history.jsonl` に採点が記録されているか見る

答えは同じ通知の下に入っています。先に自分で考えてから見てください。

## うまくいかないとき

- **push できない**: GitHub 連携がデータリポジトリに届いていない。準備の 1 を見直す
- **会話が見つからない**: 準備の 2 がオフ。または検索の返す件数が少ない。手順書の
  「対象の会話を決める」で、検索を何度かに分ける指示を足す
- **タスク自身の会話が混ざる**: 手順書の除外条件に、タスクの名前を具体的に書き足す
- **採点が記録されない**: 出題タスクが前回の会話を見つけられていない。タスクの会話の
  タイトルが「ruminate 復習」で始まっているか確かめ、違えば手順書の探し方を直す
