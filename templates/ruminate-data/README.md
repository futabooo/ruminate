# ruminate-data に置くファイル

`ruminate-data`（あなたの非公開データリポジトリ）にコピーして使うものです。

| ファイル | 何をする |
|---|---|
| `.github/workflows/ruminate-review.yml` | 復習の Issue に答えをコメントすると、その場で Claude が採点・講評して Issue を閉じ、`review/` に記録する |

## 入れ方

1. ruminate-data の clone で Claude Code を開き、`/install-github-app` を実行する。Claude の
   GitHub App を ruminate-data に入れ、secret（`CLAUDE_CODE_OAUTH_TOKEN`。Pro / Max の
   サブスクリプションで動かす場合。API キーなら `ANTHROPIC_API_KEY`）を登録してくれる。
   このとき作られるサンプルの `claude.yml` は消してよい
2. `ruminate-review.yml` を ruminate-data の `.github/workflows/` にコピーして push する
3. 復習の Issue に答えをコメントし、数分で「採点:」のコメントが付いて Issue が閉じるか見る

毎晩の出題タスク（`prompts/review-task.md`）はそのまま使えます。Action が採点した Issue は
閉じているのでタスクは触りません。Action が動かなかった日は、タスクが翌晩に採点します。
