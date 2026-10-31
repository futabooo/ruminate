# ruminate 引き継ぎメモ（2026-10-28）

claude.ai で設計・下書きしたものを、手元の Claude Code で続けるためのメモ。
設計の全体は `ruminate/docs/design.md` にある。

## 今の状態

- `origin` は `https://github.com/futabooo/ruminate`。push 済み
- `cargo test` 33 件通過、`cargo clippy` 警告なし
- 本物の Claude Code 会話ファイル（2.1.x）で変換 → push → DuckDB で読めることを確認済み
- push 前に秘密情報・個人情報がないか確認済み（保活の例を削除、テストの偽キーを EXAMPLE 入りに変更）

## 手元で試す手順

```sh
cargo install --path .
gh repo create ruminate-data --private      # または ruminate init --create ruminate-data
ruminate init --repo git@github.com:futabooo/ruminate-data.git
ruminate db                                  # 中身を確認
ruminate import --dry-run ~/Downloads/<エクスポート>/conversations.json   # claude.ai / ChatGPT
ruminate sync                                # 問題なければ送る
claude --plugin-dir plugin                   # プラグインを試す
claude plugin validate plugin
```

## 気になっている点

- 即時採点の Action（`templates/ruminate-data/`）は 2026-10-31（JST）に ruminate-data で確認済み: Issue #1 のコメントに「採点:」の講評が付き、Issue が閉じ、`review/` が `ruminate-review` 名義で `main` に push された。つまずいた点は 2 つ。(1) ruminate-data は 2026-07-15 以降に作ったので OIDC の `sub` が `repo:futabooo@944185/ruminate-data@1436689485:ref:refs/heads/main` の新形式で、ルールの subject prefix を直す必要があった。(2) Console のクレジットが 0 だと認証は通っても API 呼び出しが失敗する（Action のログには理由が出ない）
- 出題タスクは 2026-10-31（JST）に一度動かして確認済み: Issue #1（ラベル `ruminate-review`、3 問 + 折りたたみの答え）が立ち、`review/items.jsonl` が `ruminate-review` 名義で `main` に直接 commit された（PR にはならなかった）。タスクの作業ブランチ `claude/…` が `main` と同じ内容で残るが無害（2026-10-31 に一度消した。次の実行でまたできる）。`gh issue` と `main` への push はタスクから使える

- Skill `ruminate-ingest` は 2026-10-30 に一度動かして確認済み: 10 会話が `main` に 1 会話 1 コミットで入り、ファイル名は `ruminate import` の出力と一致、全文も取れていた（export の時点より後に続いた会話はターン数が増えている）。時刻は分単位（秒は `:00`）

- SessionEnd フックで `async: true` が効くかは未確認（`ruminate hook` 自体はすぐ戻る作りなので、効かなくても問題は小さい）
- Ctrl+C やターミナルを閉じたときに SessionEnd が動くかは未確認（SessionStart で回収する作り）
- 会話を再開すると、前の会話の行が新しいファイルにもコピーされるので、同じやりとりが 2 つの会話に重複して記録されることがある
- 伏せ字は正規表現なので完全ではない。仕事のプロジェクトは `exclude_projects` で除外する

## まだ作っていないもの

- 最初のリリース。cargo-dist は 2026-10-31 に入れた（`dist-workspace.toml`、`.github/workflows/release.yml`、手元で `dist build` 済み）が、タグはまだ push していない。`v0.1.0` を push すると Release ができ、README のインストーラの URL が通るようになる

- `ruminate import` の DeepSeek 対応（claude.ai と ChatGPT は 2026-10-28 に実装。`src/source/` の `Export` トレイト）
- claude.ai 用のセットアップスキル（`ruminate-setup`）
- 出題タスクの 2 晩目を確かめる: Issue #1 に答えを文章でコメントしたあと、翌晩の実行で採点と講評が付いて Issue が閉じ、`review/history.jsonl` に 3 行入り、`items.jsonl` の `stage` と `next_due` が進むか
- Codex CLI など、ほかのターミナルエージェントの読み込み

## 決めた方針（要点）

- 名前は `ruminate`（反芻）。Claude 以外のエージェントにも広げる前提で `claude-` は付けない
- コード（Public: `ruminate`）と個人データ（Private: `ruminate-data`）を分ける
- データは JSONL、見るときは DuckDB（`ruminate db`）。Markdown は置かない
- 1 会話 1 ファイル、変わったら丸ごと作り直す（元ファイルは追記のみ。claude-recall ADR-003）
- 同じファイルを 2 か所から書かない: `logs/` はパソコン（`sync`・`import`）、`review/` は出題タスク
- claude.ai の会話はスケジュールタスクからは読めない（2026-10-30 確認: 過去チャット検索がない）。普通のチャットなら検索があり、GitHub コネクタで `main` に直接書けた（同日確認）。取り込みは Skill `ruminate-ingest`（`skills/`）にして本人が頼む形。全文が要るときは公式エクスポート + `ruminate import`
- 出題は GitHub Issue 方式。Claude Code タスク（リポジトリ `ruminate-data`）が Issue を立て、本人がコメントで採点、翌晩に記録
- 出題は『科学的根拠に基づく最強の勉強法』の方法（アクティブリコール・分散学習・交互学習・自己説明・すぐのフィードバック）
- 何かを作る・設定する前に、仕組みと内容を見せて確認をもらう
