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

[platform.claude.com/settings/workload-identity-federation](https://platform.claude.com/settings/workload-identity-federation)
を開き、**Connect workload** → **GitHub Actions** を選びます。ウィザードが issuer、
service account、federation rule の 3 つをまとめて作ります。

| 項目 | 値 |
|---|---|
| Issuer URL | `https://token.actions.githubusercontent.com`（プリセット） |
| JWKS source | `discovery` |
| Subject prefix | `repo:<あなたのアカウント>/ruminate-data:*`（末尾の `*` で前方一致） |
| Audience | `https://api.anthropic.com`（既定のまま） |
| Scope | `workspace:developer`（既定） |
| Token lifetime | 600 秒（既定） |
| Service account | 名前は `ruminate-review` など。採点に使う workspace のメンバーにする |

ウィザードの最後の「Test the connection」は、次の手順でワークフローを一度動かせば通ります
（15 分の窓を逃しても、ルールの詳細ページから再テストできます）。

作られた ID を控えます: `fdrl_…`（rule）、`svac_…`（service account）、組織の UUID、
workspace の `wrkspc_…`。

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
