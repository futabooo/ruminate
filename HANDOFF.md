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

- SessionEnd フックで `async: true` が効くかは未確認（`ruminate hook` 自体はすぐ戻る作りなので、効かなくても問題は小さい）
- Ctrl+C やターミナルを閉じたときに SessionEnd が動くかは未確認（SessionStart で回収する作り）
- 会話を再開すると、前の会話の行が新しいファイルにもコピーされるので、同じやりとりが 2 つの会話に重複して記録されることがある
- 伏せ字は正規表現なので完全ではない。仕事のプロジェクトは `exclude_projects` で除外する

## まだ作っていないもの

- `ruminate import` の DeepSeek 対応（claude.ai と ChatGPT は 2026-10-28 に実装。`src/source/` の `Export` トレイト）
- 手順書は `prompts/` に 2 つ（取り込み・出題）、作り方は `prompts/README.md`（2026-10-29、claude.ai 側で未検証）
- claude.ai 用のセットアップスキル（`ruminate-setup`）
- スケジュールで動く claude.ai のセッションから、非公開リポジトリと過去チャット検索が使えるかの確認
- Codex CLI など、ほかのターミナルエージェントの読み込み

## 決めた方針（要点）

- 名前は `ruminate`（反芻）。Claude 以外のエージェントにも広げる前提で `claude-` は付けない
- コード（Public: `ruminate`）と個人データ（Private: `ruminate-data`）を分ける
- データは JSONL、見るときは DuckDB（`ruminate db`）。Markdown は置かない
- 1 会話 1 ファイル、変わったら丸ごと作り直す（元ファイルは追記のみ。claude-recall ADR-003）
- 同じファイルを 2 か所から書かない: `logs/claude-code/` はパソコン、`logs/claude-ai/` は取り込みタスク、`review/` は出題タスク
- claude.ai の会話の取り込みは、出題タスクとは別の取り込みタスクが担当。過去分は公式エクスポートで埋める
- 出題は『科学的根拠に基づく最強の勉強法』の方法（アクティブリコール・分散学習・交互学習・自己説明・すぐのフィードバック）
- 何かを作る・設定する前に、仕組みと内容を見せて確認をもらう
