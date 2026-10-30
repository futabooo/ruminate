# ruminate-data に置くファイル

`ruminate-data`（あなたの非公開データリポジトリ）にコピーして使うものです。

| ファイル | 何をする |
|---|---|
| `.github/workflows/ruminate-review.yml` | 復習の Issue に答えをコメントすると、その場で Claude が採点・講評して Issue を閉じ、`review/` に記録する |

認証は Workload Identity Federation（WIF）です。リポジトリに API キーや OAuth トークンを
置かず、GitHub Actions の OIDC トークンを Anthropic の短命トークン（既定 10 分）に
交換します。漏れて困る鍵がそもそも存在しない形です。

前提: Claude Console の組織（API の従量課金）で admin 以上の権限。Max の枠ではなく
API の利用料がかかります。採点は一晩 3 問なので、月に数十円〜数百円です。

## 入れ方

### 1. Claude の GitHub App を入れる

ruminate-data の clone で Claude Code を開き、`/install-github-app` を実行します。
Claude の GitHub App を ruminate-data に入れてくれます。鍵の登録を聞かれたら
**スキップ**し、作られたサンプルの `claude.yml` は消します。

### 2. Console で federation を設定する

やることは「GitHub Actions が出す身分証明書（OIDC トークン）を Anthropic に信用させ、
`ruminate-data` の `main` で動くワークフローだけに、採点用の service account として
API を使わせる」設定です。Console のウィザードが 3 つのリソース（issuer、service
account、federation rule）をまとめて作ってくれます。

1. 組織 ID を控える。[platform.claude.com/settings/organization](https://platform.claude.com/settings/organization)
   に UUID（`00000000-0000-…`）がある
2. [platform.claude.com/settings/workload-identity-federation](https://platform.claude.com/settings/workload-identity-federation)
   を開き、**Connect workload** を押す
3. プロバイダの一覧から **GitHub Actions** のタイルを選ぶ
4. 画面の指示に沿って入力する。項目名は画面の更新で変わるので、意味で読み替えてください

   | 画面の項目 | 入れる値 | 意味 |
   |---|---|---|
   | Issuer URL | `https://token.actions.githubusercontent.com`（プリセット済み） | GitHub Actions の身分証明書を発行する人 |
   | JWKS | `discovery`（既定） | 署名の検証鍵を GitHub から自動で取る |
   | Subject prefix | `repo:<あなたのアカウント>/ruminate-data:ref:refs/heads/main` | `ruminate-data` の `main` で動くワークフローだけ |
   | Claims（追加の条件） | `repository_owner` = `<あなたのアカウント>` | 念のための二重チェック |
   | Audience | `https://api.anthropic.com`（既定） | そのまま |
   | Service account 名 | `ruminate-review` など | 採点用の非人間アカウント |
   | Workspace | 採点に使う workspace（既定のものでよい） | 利用料とレート制限がここに付く |
   | Rule 名 | `ruminate-review-main` など | |
   | Scope | `workspace:developer`（既定） | API キーと同じ権限 |
   | Token lifetime | `600`（既定） | 発行されるトークンは 10 分で切れる |

   `issue_comment` で動くワークフローは `main` の文脈で走るので、subject は
   `…:ref:refs/heads/main` に固定できます。`repo:…/ruminate-data:*` のように緩めると
   PR からの実行も通るので、緩めません。
5. **Verify issuer** があれば押す（GitHub の鍵を取れるかの事前確認）
6. 作成すると「Test the connection」で 15 分間、接続を待つ画面になる。ここは一度閉じて
   構いません。手順 3〜5 でワークフローを動かしたときに通ります（ルールの詳細ページから
   再テストもできる）
7. 作られた 3 つの ID を控える: **federation rule** の `fdrl_…`、**service account** の
   `svac_…`、**workspace** の `wrkspc_…`。手順 1 の組織 UUID と合わせて 4 つ

### 3. リポジトリに variables を入れる

秘密ではないので **variables** です（Settings → Secrets and variables → Actions → Variables）。

```sh
gh variable set ANTHROPIC_FEDERATION_RULE_ID --body fdrl_... -R <あなたのアカウント>/ruminate-data
gh variable set ANTHROPIC_ORGANIZATION_ID    --body 00000000-0000-0000-0000-000000000000 -R <あなたのアカウント>/ruminate-data
gh variable set ANTHROPIC_SERVICE_ACCOUNT_ID --body svac_... -R <あなたのアカウント>/ruminate-data
gh variable set ANTHROPIC_WORKSPACE_ID       --body wrkspc_... -R <あなたのアカウント>/ruminate-data
```

`ANTHROPIC_API_KEY` という secret が残っていたら消します。あると federation より
優先されてしまいます。

### 4. ワークフローを置く

`ruminate-review.yml` を ruminate-data の `.github/workflows/` にコピーして push します。

### 5. 試す

復習の Issue に答えをコメントし、数分で「採点:」のコメントが付いて Issue が閉じるか見ます。
失敗したら Actions のログと、Console の
[認証履歴](https://platform.claude.com/settings/workload-identity-federation?tab=history)
（どのルールで何が拒否されたか）を見ます。

## 毎晩の出題タスクとの関係

毎晩の出題タスク（`prompts/review-task.md`）はそのまま使えます。Action が採点した Issue は
閉じているのでタスクは触りません。Action が動かなかった日は、タスクが翌晩に採点します。
