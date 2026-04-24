---
name: "release-notes"
description: "Use whenever the user asks for release notes, changelog bullets, a GitHub Release body, a 'What's Changed' section, a summary from the last release or tag to now, or wants release copy grounded in real git history, PRs, issues, and candidate release changes. This skill is especially important when the user wants short publish-ready release bullets with repository-specific conventions."
---

# Release Notes

Draft release notes from repository evidence, not guesswork.

This skill is for producing publishable release copy for the next release draft. It is not limited to already published history. Start from the latest relevant tag or release boundary, then include committed and clearly related unreleased candidate changes when the user is drafting the upcoming release before publishing.

## Default target

Unless the user says otherwise, assume the target output is a GitHub Release body draft for the next release in this structure:

1. `## What's Changed`
2. A flat bullet list of user-facing changes
3. The repository-specific Mac install notice at the end

Do not silently switch the task to updater notes or in-app update copy. If the repository has multiple release surfaces, identify which one the user wants before drafting. If unspecified, produce the GitHub Release body draft first and mention that updater notes may need a shorter variant.

In this repository, default to Chinese release copy unless the user explicitly asks for English or another language.

## Evidence order

Always work in this order:

1. Determine the release range.
2. Read the commit and diff summary for that range.
3. Check for clearly related staged or unstaged candidate changes that belong to the same upcoming release.
4. Read merged PRs and candidate PR context when available.
5. Read related issues for confirmation and issue references.
6. Draft bullets from user-visible impact, not implementation detail.

Prefer these sources:

- `git describe --tags --abbrev=0`
- `git diff --stat <base>..HEAD`
- `git log --first-parent --oneline <base>..HEAD`
- `gh repo view --json nameWithOwner,url,defaultBranchRef`
- `gh pr list --state merged ...`
- `gh issue list --state all ...`
- `gh release list` when release publication context matters

If `gh` metadata is unavailable, degrade gracefully to commit-based notes. Do not invent PR numbers, issue numbers, or authors.

## Range rules

- By default, compare the latest relevant tag to `HEAD`.
- If the user explicitly names a range like `v0.8.4..v0.8.5`, use that exact range.
- If the latest tag points to `HEAD` and the user wants the notes for that released version, compare the previous tag to the current tag.
- Do not focus only on working tree changes. Released or tagged history is usually the real scope.

## Dirty worktree rules

- After collecting the default release range, check whether the worktree contains relevant staged or unstaged changes that belong to the same upcoming release.
- For next-release drafting in this repository, relevant staged or unstaged changes are IN scope by default even if they are not yet tagged, merged, or published.
- Do not ask for confirmation before including clearly related uncommitted changes in the draft unless the user explicitly asks for committed-only notes.
- Do not confuse "in upcoming draft" with "already published". These changes can appear in the release draft, but the surrounding explanation should not claim the release has already happened.
- If relevant uncommitted work exists, merge it into the draft by default when the topic match is strong.
- If an issue is clearly tied to unreleased candidate work, it can appear in the draft even when the issue is still open.

## Writing rules

- Prefer PR titles as the primary basis for bullets because they already encode the change unit users can trace.
- Rewrite titles only as much as needed for clarity and consistency.
- Prefer user-visible impact over refactors, internal naming, or storage details.
- Drop purely internal changes unless they materially affect user behavior, reliability, compatibility, or supported providers/models.
- Keep bullets concise and publishable.
- If one commit or PR spans multiple user-facing themes, split the release wording by product surface instead of mechanically following the commit scope, module name, or title prefix.
- You MUST merge closely related commits or fixes into one bullet when they belong to the same user-facing theme.
- Prefer 2-4 strong bullets over a long fragmented list when drafting a normal release.
- Favor product-surface labels such as `模型预设`、`会话管理`、`Claude Code / WSL / SSH` over raw commit scopes like `feat(models)` or `fix(session)` when drafting Chinese release notes for this repository.
- By default, emit at most one bullet per product surface.
- Keep each bullet as short as possible. Prefer a compact noun phrase or short verb phrase over a full explanatory sentence.
- Avoid explanatory tails such as `让...`、`避免...`、`减少...`、`更稳...` unless removing them would make the change unclear.
- Use a flat list. Do not create nested groups unless the user explicitly asks for grouped sections.
- Preserve known regressions or unverified boundaries only when the user asks for them, or when omitting them would materially mislead the release notes.

## Attribution rules

Default bullet shape:

`- <type(scope)>: <user-facing summary> in #<pr_number>`

Contributor bullet shape:

`- <type(scope)>: <user-facing summary> by @<author_login> in #<pr_number>`

Apply these attribution rules:

- If the PR author is the repository owner or the user's own account, omit `by @...`.
- In this repository, treat `coulsontl` as self-authored and omit the `@` attribution.
- For other contributors, include `by @<login>`.
- If no PR can be confirmed, fall back to a commit-based bullet without fake attribution or fake PR numbers.

Commit fallback shape for research only:

`- <internal draft only> <summary> (from <short_sha>)`

Do not emit `(from <short_sha>)` in the final publishable release draft unless the user explicitly asks for evidence annotations.

## Issue rules

- Read issues as research evidence, not as automatic output lines.
- Only mention an issue number in the release notes when the PR, commit, candidate patch, or explicit user instruction clearly ties that issue to the change being drafted.
- Do not add a separate `Closed Issues` section by default.
- If the user wants known problems called out, add a `## Known Issues` section after `What's Changed`.
- Open issues can still matter during next-release drafting. When an open issue clearly matches relevant in-progress work in the dirty worktree, it may be included in the draft as a pending fix or candidate item.

## Output template

Use this default GitHub Release body template unless the user requests another surface:

```md
## What's Changed

- 模型预设：一句话概括同一主题下的模型或预设更新
- 会话管理：一句话概括同一主题下的会话或删除流程修复
- Claude Code / WSL / SSH：一句话概括同一主题下的跨端同步或路径修复，可在末尾带 `，关联 #123`

⚠️ Mac安装提示：`"应用程序"已损坏` 解决方法：`xattr -cr /Applications/AI\ Toolbox.app`
```

Template rules:

- Replace placeholders with real repository data.
- Do not copy another project's release wording.
- Do not output placeholder bullets in the final answer.
- Keep the Mac install notice exactly as written for this repository unless the user asks to change it.
- Do not include `Full Changelog` by default in this repository unless the user explicitly asks for it.
- Prefer the shortest acceptable wording that still preserves the user-facing meaning.

## Style rules

- Keep the tone factual, compact, and release-ready.
- Do not over-explain implementation details inside the bullets.
- Avoid marketing language.
- Avoid claiming a fix is shipped and verified unless the evidence supports it.
- When evidence is incomplete, say so briefly before the draft or return a research summary first.
- In this repository, prefer concise Chinese wording for the final draft unless the user requests another language.
- When drafting the next release, it is acceptable to include not-yet-closed issues if the linked work is clearly intended for that release.
- Favor direct short phrases that can be pasted into the release page with minimal editing.
- The final release draft should read like a polished changelog, not like traced research notes or commit summaries.
- Do not use hedge words like `candidate`, `pending`, or similar qualifiers in the final release draft unless the user explicitly asks for that uncertainty to be surfaced.

## Good and bad examples

Bad:

`- 会话管理：优化 Session Manager 路径选项加载与 OpenCode 会话删除流程，删除缺失底层记录时不再误报失败，批量清理也更稳`

Good:

`- 会话管理：优化路径加载与会话删除流程`

Bad:

`- 会话管理：修复 KeepAlive 隐藏页操作的反馈竞态，避免 loading 状态卡住或提示不一致`

Good:

`- 会话管理：修复 KeepAlive 状态反馈问题`

Bad:

`- 模型预设：新增 Qwen3.6 Plus、Kimi K2.6 和 GPT-5.5 预设，并调整相关展示逻辑，让 Codex 卡片按配置显示 reasoning effort`

Good:

`- 模型预设：新增 GPT-5.5、Qwen3.6 Plus 和 Kimi K2.6`

## Minimal checklist

Before finalizing, verify:

1. The compare range is explicit.
2. Every bullet is backed by a PR or commit in that range.
3. Every `#123` actually exists.
4. Every `by @user` is real and omitted for self-authored PRs.
5. The Mac install notice is present at the end for GitHub Release body output.
6. Any unreleased candidate items are worded as next-release draft content, not falsely described as already published.
7. The final draft does not expose raw commit hashes, `(from ...)` tails, or commit-scope-first wording unless the user explicitly asks for that format.
8. Relevant uncommitted changes are included by default for next-release drafting unless the user explicitly asks for committed-only notes.
