# claude.ai のタスクを作る

このフォルダには、claude.ai のスケジュールタスクに貼る手順書が入っています。
手順書の読み手はタスクとして動く Claude です。人がやるのは、ここに書いた
準備とタスクの作成だけです。

| 手順書 | タスク | 何をする |
|---|---|---|
| [review-task.md](review-task.md) | 出題タスク（毎晩） | `ruminate-data` を読んで復習問題を 3 問 GitHub Issue に出し、前回の採点を `review/` に書く |

claude.ai の会話を毎日拾う「取り込みタスク」は作りません。スケジュールタスクの環境には
過去チャット検索がない（2026-10-30 に確認。リポジトリを選んでも選ばなくても同じ）ので、
タスクから claude.ai の会話は読めません。代わりに、本人がいる普通のチャットで動く
Skill（[../skills/ruminate-ingest/SKILL.md](../skills/ruminate-ingest/SKILL.md)）を使います。
入れ方は README の「claude.ai の会話を取り込む」。

claude.ai の画面の文言や場所は更新で変わります。見つからなければ設定画面の検索で
「スケジュール」「GitHub」を探してください。

## 出題タスクを作る

1. claude.ai の「スケジュール済みタスク」で新規作成
2. 名前: `ruminate 復習` など
3. 指示: [review-task.md](review-task.md) の中身をそのまま貼る
4. トリガー: スケジュール、毎日、本人が通知を見られる時間。例: 21:00（JST）
5. 詳細設定 → 一般 → **リポジトリ: `<あなたのアカウント>/ruminate-data`**。これで Claude Code の
   タスクになり、リポジトリの checkout と `git` / `gh` が使える
6. 詳細設定 → コネクタ: すべて外す（使わない）。動作: PR の自動修正はオフ。通知: オフ
   （問題は GitHub Issue で届く）
7. 作成したら一度手動で実行する。初回は `review/items.jsonl` が空なので、直近の会話から
   3 問作って Issue を立てるはず。GitHub の通知が届くか見る
8. Issue のコメントに `1 ○ 2 × 3 ○` のように採点を書く
9. 翌晩の実行後、Issue が閉じられ、`ruminate-data` の `review/history.jsonl` に採点が
   記録されているか見る

答えは Issue の「答え」を開くと見えます。先に自分で考えてから開いてください。

## うまくいかないとき

- **Issue が立たない、push できない**: タスクのリポジトリが `ruminate-data` になっているか
  見直す。リポジトリを選ばないタスクには GitHub のトークンがない
- **`main` に push できず PR になった**: 手順書の 6 で自分でマージするようにしてある。
  マージされずに PR が残るなら、タスクの実行ログを見て、`gh pr merge` が失敗した理由を
  手順書に反映する
- **採点が記録されない**: 前回の Issue が閉じていないか、ラベル `ruminate-review` が
  付いていないかを見る。手順書はラベルで前回の Issue を探す
- **会話が読めていない**: `logs/` が空。パソコンで `ruminate sync`、claude.ai の分は
  `ruminate import` を先に済ませる
