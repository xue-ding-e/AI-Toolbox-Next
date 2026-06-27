# 自定义全局 Skills 目录方案草案

## 背景

需求来源：GitHub issue `https://github.com/coulsontl/ai-toolbox/issues/227`。

用户希望 AI Toolbox 支持把已有的全局 Skills 目录作为统一源目录，例如：

```text
~/.agent/skills/
  code-review/
    SKILL.md
  rust-debug/
    SKILL.md
```

当前 AI Toolbox 已经有 Skills 中央仓库概念，后端也已经通过 `skill_settings:skills.central_repo_path` 保存中央仓库路径。但当前前端设置页只是展示路径，没有提供完整的更改、预览、扫描和纳管流程；现有导入逻辑也主要是把本地或 Git 来源复制到中央仓库，而不是把一个已有全局目录直接作为长期源目录接入。

本方案先作为实现前讨论文档，用于把产品语义、前端交互、后端边界和风险考虑清楚。本文档不是最终实现承诺，后续根据评审逐步收敛。

## 核心概念

### 全局 Skills 源目录

全局 Skills 源目录是 Skill 文件内容的事实源，也就是当前模块文档里所说的中央仓库 `central_repo_path`。

第一版目标是允许用户把它改成外部已有目录，例如 `~/.agent/skills`，并允许用户继续用外部编辑器、Git、同步盘等方式维护这个目录。

### AI Toolbox 管理记录

AI Toolbox 仍然需要数据库记录来保存管理元数据，包括：

- Skill 名称。
- Skill 实际源目录的相对路径。
- 分组。
- 用户备注。
- 是否启用管理。
- 已同步工具。
- 同步目标路径和同步模式。
- 内容 hash。
- source health 诊断。

因此不能把“实时扫描文件夹”直接当成 Skills 列表的唯一事实源。扫描只用于发现和纳管，纳管后仍以 DB 记录作为 UI 和同步流程的管理事实源。

### 工具运行时目标目录

Claude Code、Codex、OpenCode、OpenClaw、Cursor 等工具的 Skills 目录是同步目标，不是源。

AI Toolbox 的同步方向仍然是：

```text
全局 Skills 源目录 -> 工具运行时 Skills 目录
```

同步目标路径仍优先沿用当前 `runtime_location` 逻辑。第一版不额外做“每个内置工具单独覆盖 Skills 目标目录”的设置。

## 第一版目标

第一版需要让用户完成以下流程：

1. 在 Skills 设置中选择或输入一个全局 Skills 目录，例如 `~/.agent/skills`。
2. AI Toolbox 在保存前展示路径切换预览，包括新目录里可识别的 Skill、已有管理记录的匹配情况、缺失项和冲突项。
3. 如果用户是从旧中央仓库目录迁移到新目录，AI Toolbox 提供迁移预览，并允许把旧目录中已管理的 Skill 复制到新目录。
4. 用户确认后，AI Toolbox 将该目录保存为中央仓库路径。
5. 用户可以选择把新目录中已有但未纳管的 Skill 纳管进 DB。
6. 纳管是“登记管理记录”，不复制、不移动、不覆盖源目录内容。
7. 后续同步到各工具时，源目录直接来自该全局目录。
8. 对这类全局目录 Skill，删除默认是“取消纳管”，不删除用户源文件。

## 第一版非目标

第一版暂不做以下能力：

- 不把工具运行时 Skills 目录反向作为源目录。
- 不给每个内置工具增加独立的 Skills 目标目录 override。
- 不绕过数据库，直接把全局目录实时扫描结果当列表事实源。
- 不自动覆盖全局目录里的同名目录。
- 不默认删除用户全局目录中的源文件。
- 不在路径切换时自动处理所有历史异常数据，异常项只预览和提示。
- 不引入多套 source of truth，例如 `skill_preferences.central_repo_path`。

## 数据语义

### source_type

建议新增一种 source type：

```text
central
```

含义：

- 这个 Skill 的源文件就是当前中央仓库目录下的某个子目录。
- 它不是从本地路径复制进中央仓库的副本。
- 它也不是由 Git 安装逻辑负责拉取更新。

已有 source type 继续保留：

- `local`：从本地目录复制进中央仓库。
- `git`：从 Git 仓库复制进中央仓库。
- `import` 如未来存在，需要继续保持兼容。

### central_path

当前 `central_path` 已支持相对路径存储。第一版应继续使用相对路径。

关键规则：

- `central_path` 应表示源目录相对于当前中央仓库的实际路径。
- 不要求它一定等于 Skill name。
- 同步目标目录名仍使用 Skill name。

示例：

```text
~/.agent/skills/my-review-skill/SKILL.md
```

如果 `SKILL.md` frontmatter 中写的是：

```yaml
name: code-review
```

则记录建议为：

```json
{
  "name": "code-review",
  "source_type": "central",
  "central_path": "my-review-skill"
}
```

同步到 Claude Code 时目标仍是：

```text
~/.claude/skills/code-review
```

源路径则是：

```text
~/.agent/skills/my-review-skill
```

### content_hash

纳管时计算一次内容 hash。

用户在外部编辑器修改全局目录后，AI Toolbox 不需要实时刷新 hash。可以在以下动作中刷新：

- 打开列表时只做轻量 source health 诊断，不强制 hash 全量扫描。
- 用户点击刷新 / 更新 Skill。
- 同步 copy 目标前。
- WSL / SSH Skills 同步前。
- 扫描当前全局目录时。

对 `source_type=central`，同步前不能只相信 DB 里旧的 `content_hash`。用户可能刚在外部编辑器或 Git 中修改了 `~/.agent/skills/<name>`，此时 DB hash 仍是旧值。如果 WSL / SSH 同步直接用旧 hash 判断，远端可能误判为无需上传。

推荐语义：

- 本地 copy 目标同步前重新计算当前源目录 hash。
- WSL Skills 同步前重新计算当前源目录 hash。
- SSH Skills 同步前重新计算当前源目录 hash。
- 如果新 hash 与 DB 中不同，先使用新 hash 参与本次同步判断；同步成功后回写 DB 的 `content_hash` 和更新时间。
- symlink / junction 目标不需要复制内容，但仍应刷新 hash 和 source health，让 UI 状态与源目录一致。

## 后端命令设计

命令名称仅为草案，实际实现可按项目命名习惯调整。

### 预览路径切换

```text
skills_preview_central_repo_path(path: String) -> CentralRepoPathPreviewDto
```

用途：

- 保存新中央仓库路径前先预览。
- 不写 DB。
- 不复制文件。
- 不创建 Skill 记录。

建议 DTO：

```ts
interface CentralRepoPathPreviewDto {
  requested_path: string;
  resolved_path: string;
  current_path: string;
  default_path: string;
  current_uses_default: boolean;
  requested_is_default: boolean;
  exists: boolean;
  is_directory: boolean;
  can_create: boolean;
  can_read: boolean;
  can_write: boolean;
  detected_skills: DetectedCentralSkillDto[];
  matched_existing: CentralSkillMatchDto[];
  unmanaged_detected: DetectedCentralSkillDto[];
  missing_existing: ManagedSkillSummaryDto[];
  repair_candidates: CentralSkillRepairCandidateDto[];
  migration_candidates: CentralRepoMigrationCandidateDto[];
  migration_conflicts: CentralRepoMigrationConflictDto[];
  affected_targets: CentralRepoTargetImpactDto[];
  conflicts: CentralRepoConflictDto[];
  path_warnings: string[];
  blocking_errors: string[];
  can_apply: boolean;
}
```

`detected_skills` 表示新目录中扫描出来的 Skill。

`matched_existing` 表示当前 DB 已管理的 Skill 在新目录里能找到匹配目录。

`unmanaged_detected` 表示新目录中存在，但 DB 里尚未纳管的 Skill。

`missing_existing` 表示当前 DB 已管理，但切换到新目录后找不到源目录的 Skill。

`repair_candidates` 表示当前 DB 记录源路径缺失，但新目录中扫描到了可用于修复该记录的同名 Skill。它不是普通未纳管项，也不是普通冲突项；用户确认后应更新既有 DB 记录的 `central_path` 和 hash，而不是创建新记录。

`migration_candidates` 表示当前旧中央仓库里存在、但新目录中缺失，因此可以从旧目录复制到新目录的已管理 Skill。

`migration_conflicts` 表示旧目录迁移时遇到的新目录同名冲突、路径重叠或不可覆盖项。

`affected_targets` 表示路径切换后需要重写或重同步的工具目标，包括 symlink、junction、copy 目标。

`conflicts` 表示不能自动处理的冲突，例如多个目录解析出同一个 name。

### 默认目录和恢复默认

默认中央仓库目录仍然是当前 resolver 的 fallback：

```text
app_data_dir/skills
```

前端需要知道当前路径是否为默认目录，因此后端应提供默认路径信息。可以二选一：

1. 在 `skills_get_central_repo_path` 的返回值中扩展 metadata。
2. 新增轻量命令：

```text
skills_get_default_central_repo_path() -> String
```

恢复默认目录不能直接保存字符串。正确语义是：

1. 解析默认目录。
2. 以默认目录作为目标路径执行同一套 `skills_preview_central_repo_path`。
3. 用户确认迁移、纳管、重同步选项。
4. 应用后清除 `skill_settings:skills.central_repo_path` 的自定义覆盖，让后续继续走默认 fallback。

如果实现上暂时不方便删除 JSONB 字段，也不能把 `skill_preferences` 重新引入为第二事实源；应在 `skill_settings:skills` 中明确支持删除或置空 `central_repo_path`，并让 resolver 把空值视为缺失。

### 应用路径切换

```text
skills_apply_central_repo_path_change(
  path: String,
  options: ApplyCentralRepoPathOptionsDto
) -> ApplyCentralRepoPathResultDto
```

建议 options：

```ts
interface ApplyCentralRepoPathOptionsDto {
  adopt_detected_skill_paths: string[];
  repair_existing_skill_paths: Record<string, string>; // skill_id -> detected relative path
  migrate_existing_skill_ids: string[];
  use_default_path: boolean;
  resync_enabled_tools: boolean;
}
```

语义：

1. 解析并校验新路径。
2. 必要时创建目录。
3. 检查 source / target overlap 风险。
4. 如果 `migrate_existing_skill_ids` 非空，把当前旧中央仓库中存在、但新路径缺失的已管理 Skill 复制过去。
5. 如果 `use_default_path=true`，清除自定义 `central_repo_path` 覆盖；否则保存 `skill_settings:skills.central_repo_path`。
6. 对用户选择的 repair candidates 更新既有 DB 记录的 `central_path`、hash、source health，不创建新记录。
7. 对用户选择的 detected skills 创建 `source_type=central` 管理记录。
8. 如果 `resync_enabled_tools=true`，触发已启用工具的重同步。
9. emit `skills-changed`，让前端、托盘、WSL/SSH 后续链路感知变化。

注意：

- 不覆盖新路径下已有目录。
- 不把路径切换和纳管做成多个前端独立调用的半事务，避免用户中途关闭 Modal 后状态不一致。
- 如果复制缺失项部分失败，结果中要返回 failed items，并清楚说明哪些已应用、哪些未应用。

### 路径切换提交顺序

`skills_apply_central_repo_path_change` 必须有明确提交边界，避免出现“路径已经切换，但必选迁移或纳管失败”的半残状态。

建议顺序：

1. 重新解析并校验目标路径，确认 preview 里的关键条件仍成立。
2. 做 source / target overlap、canonical path、权限、同名冲突和 root skill 等 preflight。
3. 对用户勾选的迁移项先执行复制。可使用 staging 目录，或至少保证每个目标目录不存在时才复制。
4. 如果必选迁移项失败，不写 `central_repo_path`，不创建/修改 managed skill 记录，返回失败明细。
5. 迁移项全部成功后，在同一个 DB 事务里：
   - 清除或写入 `central_repo_path`。
   - 更新 repair candidates 的 `central_path` 和 hash。
   - 创建 adopt candidates 的 managed skill 记录。
6. DB 提交成功后再执行本地工具目标重同步。
7. 本地工具目标、WSL、SSH 的后续同步失败不回滚路径切换，但必须进入结果页，列出需要用户继续处理的失败项。

这样可以把“配置事实源切换”与“后续投影失败”分开处理：前者必须尽量原子，后者允许部分失败但不能静默。

### 扫描当前全局目录

```text
skills_scan_central_repo() -> CentralRepoScanDto
```

用途：

- 用户手动往当前全局目录新增 Skill 后，点击“扫描目录”发现未纳管项。
- 用户恢复备份或切换目录后，扫描当前目录并发现可修复的缺失管理记录。
- 不改变路径。
- 不写 DB。

### 纳管当前全局目录中的 Skill

```text
skills_adopt_central_repo_skills(paths: Vec<String>) -> AdoptCentralSkillsResultDto
```

用途：

- 将当前中央仓库目录中的已有 Skill 登记到 DB。
- 不复制源文件。
- 不覆盖源文件。

语义：

- `paths` 应是相对于当前中央仓库的路径，或后端返回的 stable id。
- 后端重新校验路径仍在当前中央仓库内。
- 路径必须是可解析目录。
- 默认创建 `source_type=central`。
- 如果 DB 中已存在同名 Skill 且该记录当前源路径健康，应返回冲突，不自动覆盖。
- 如果 DB 中已存在同名 Skill 但该记录当前源路径缺失，应作为 repair candidate 返回，由用户确认“绑定到该目录”。

### 修复缺失管理记录

```text
skills_repair_central_repo_skill(skill_id: String, relative_path: String) -> RepairCentralSkillResultDto
```

也可以合并进 `skills_adopt_central_repo_skills` 的 options；关键是语义必须独立于“创建新记录”。

用途：

- DB 中已有 Skill 记录，但 `central_path` 当前解析后缺失。
- 当前全局目录中扫描到了同名或用户明确选择的目录。
- 用户确认后，把既有记录重新绑定到该目录。

语义：

- 不复制源文件。
- 不覆盖源文件。
- 不创建新 Skill 记录。
- 更新既有记录的 `central_path`、`content_hash`、description/source health 和更新时间。
- 保留分组、备注、已同步工具、同步模式等管理元数据。
- 修复后如 `resync_enabled_tools=true`，按新 source 重建工具目标。

## 旧目录迁移到新目录

路径切换必须把“保存新路径”和“旧目录迁移”设计成同一个用户确认流程，而不是让用户先手动复制文件再回来保存。否则用户从默认 app data 目录切到 `~/.agent/skills` 时，很容易让现有 Skill 变成 source warning。

### 迁移对象

迁移对象只包括当前 DB 已管理、且旧中央仓库中源目录仍存在的 Skill。

示例：

```text
旧中央仓库：%APPDATA%/com.ai-toolbox/skills
新中央仓库：~/.agent/skills
```

如果当前 DB 中有：

```json
{
  "name": "code-review",
  "central_path": "code-review"
}
```

且旧目录存在：

```text
%APPDATA%/com.ai-toolbox/skills/code-review/SKILL.md
```

而新目录不存在：

```text
~/.agent/skills/code-review
```

则它是可迁移项。

### 迁移方式

第一版建议只做复制，不做 move。

原因：

- 复制失败时旧目录仍安全存在。
- 用户可以自己确认新目录内容后再清理旧目录。
- 避免与现有备份、回滚、同步目标清理产生复杂交互。

复制规则：

- 复制旧中央仓库中的 Skill 源目录到新中央仓库。
- 复制后的目录名默认沿用原 `central_path`。
- 不覆盖新目录中已有目录。
- 如果新目录中已有同名目录，但解析出的 Skill name 相同，也只标记为 matched，不覆盖。
- 如果新目录中已有同名目录但内容不同，标记为冲突，让用户手动处理。

### 迁移后的记录处理

路径保存后，DB 中已有 Skill 的 `central_path` 仍保持相对路径不变。

例如：

```json
{
  "name": "code-review",
  "central_path": "code-review"
}
```

从旧目录切到新目录后会自然解析为：

```text
~/.agent/skills/code-review
```

因此不需要批量改写 `central_path`，除非后续支持用户在迁移时选择不同目标目录名。

### source_type 是否转换

第一版建议：

- 从 `local` / `git` 复制进旧中央仓库的既有 Skill，迁移到新中央仓库后仍保留原 `source_type`。
- 新目录中本来就存在、通过扫描纳管的 Skill 使用 `source_type=central`。

原因：

- 保留 `git` / `local` 的更新语义，避免路径切换改变用户原有“从 Git 更新”的预期。
- `source_type=central` 专门表示“这个 Skill 原生由当前全局目录维护”。

如果用户希望把一个历史 `git` / `local` Skill 转成完全由全局目录维护，后续可以单独提供“转换为全局目录源”的动作。第一版先不自动转换。

需要在 UI 和文档中明确：迁移到用户自定义外部中央仓库后，保留 `source_type=git/local` 的 Skill 仍然保留原更新语义。也就是说：

- `git` Skill 点击更新时，仍可能从 Git 重新拉取并覆盖当前全局目录中的该 Skill 子目录。
- `local` Skill 点击更新时，仍可能从原始 `source_ref` 重新复制并覆盖当前全局目录中的该 Skill 子目录。
- 删除时则不能因为它是 `git/local` 就默认删除外部全局目录中的源文件；删除源文件仍需危险确认。

因此 Skill 卡片或更新确认里应区分：

- `全局目录源`：刷新当前目录内容。
- `Git 管理副本`：从 Git 更新到当前全局目录。
- `本地管理副本`：从原始本地来源更新到当前全局目录。

### 前端迁移选项

路径切换预览 Modal 中必须单独有一个分区：

```text
从旧目录迁移
```

展示内容：

- 可迁移数量。
- 每个 Skill 的名称、旧路径、新路径。
- 默认勾选可迁移项。
- 冲突项单独展示，不可勾选。

操作选项：

```text
[x] 复制旧目录中已管理但新目录缺失的 Skill
```

如果用户取消勾选，Modal 需要明确提示：

```text
这些 Skill 切换后会显示源目录缺失，直到你手动把文件放到新目录或重新纳管。
```

### 旧目录清理

第一版不提供自动删除旧目录。

路径切换成功后可以提示：

```text
旧目录未被删除。确认新全局目录工作正常后，你可以手动清理旧目录。
```

后续如果要做“迁移后清理旧目录”，必须作为危险操作单独设计。

## 已同步工具目标处理

路径迁移不仅影响中央仓库源文件，也会影响已经同步到各工具目录的目标。

典型情况：

```text
旧中央仓库：%APPDATA%/com.ai-toolbox/skills
新中央仓库：~/.agent/skills
工具目标：~/.claude/skills/code-review -> %APPDATA%/com.ai-toolbox/skills/code-review
```

切换中央仓库后，`~/.claude/skills/code-review` 这个目标路径通常不变，但它内部的 link target 仍可能指向旧中央仓库。因此迁移应用后必须默认重写已启用工具目标。

### sync_details 的语义

`sync_details` 继续表示“这个 Skill 期望同步到哪些工具以及目标路径是什么”。

路径切换时：

- 不因为中央仓库路径改变就清空 `sync_details`。
- 不把工具目标目录当成新的源目录。
- 对仍启用管理且源路径可解析的 Skill，按 `enabled_tools` 和 `sync_details` 重建目标。
- 对源路径缺失且用户未迁移的 Skill，不应自动删除已有工具目标；应保留记录并展示 warning，避免把用户当前仍可用的旧链接或 copy 目标直接清掉。

### symlink / junction 目标

如果现有工具目标是 symlink 或 junction，并且 link target 指向旧中央仓库：

1. 应用路径切换后，默认重新执行同步。
2. 同步时使用新中央仓库解析出的 source path。
3. 旧 symlink / junction 应被安全移除。
4. 新 symlink / junction 指向新中央仓库中的 Skill 目录。

如果目标已经指向新 source，重同步应保持幂等，不重复破坏目标。

如果目标路径本身也因为工具 runtime location 变化而变化，应沿用现有逻辑清理旧 target，并在新 target path 重建。

### copy 目标

如果现有工具目标是 copy：

- 中央仓库路径切换后，copy 目标不会自动更新。
- 默认重同步时应从新中央仓库重新复制内容到工具目标目录。
- 如果用户跳过重同步，工具目标仍是旧内容，直到用户手动刷新或重新同步。

### 跳过重同步时的提示

路径切换预览 Modal 中 `应用后重新同步已启用工具` 应默认勾选。

如果用户取消勾选，前端必须明确提示：

```text
已同步到工具目录的链接或复制内容不会立即更新。symlink/junction 可能仍指向旧目录；如果之后删除旧目录，工具里的 Skill 可能失效。
```

该提示对恢复默认目录同样适用。

### 迁移失败或源缺失时

如果某个已管理 Skill 没有迁移到新中央仓库，且新目录中也没有对应源目录：

- 不要在路径切换时自动删除它的工具目标。
- 不要把工具目标目录反向导入为源。
- 在结果中返回该 Skill 的 source warning。
- 在 UI 中提示用户选择：
  - 把旧源目录复制到新中央仓库。
  - 手动在新中央仓库补齐文件。
  - 取消该 Skill 的工具同步。
  - 删除或取消纳管该 Skill。

如果重同步部分失败：

- 路径切换和已完成的迁移不应静默回滚。
- 结果中必须返回失败的 Skill / tool / target path。
- UI 应展示“目录已切换，但以下工具目标仍需手动重同步或修复”。

### 旧目录删除前检查

第一版不自动删除旧目录，但后续如果提供“清理旧目录”危险操作，必须先检查是否仍有工具目标 symlink/junction 指向旧中央仓库。

如果仍有旧链接：

- 默认阻止删除。
- 或要求用户先重同步 / 取消同步。
- 不能在仍存在旧 link target 的情况下默认删除旧中央仓库。

## 目录扫描规则

第一版建议采用保守扫描规则。

### 扫描深度

默认只扫描一级子目录：

```text
~/.agent/skills/<skill-dir>/SKILL.md
```

原因：

- 可解释。
- 性能可控。
- 避免误扫 `.git`、依赖目录、临时目录。

后续如需要支持深层目录，可以单独加“递归扫描”开关。

### 根目录 SKILL.md

如果全局目录根部本身有 `SKILL.md`：

- 第一版只作为 warning 展示，不纳管为 root skill。
- 不允许把全局目录本身作为普通 Skill source。
- 不允许将 `central_path` 写成空字符串、`.`、`./` 或任何会解析回中央仓库根目录的值。
- 如果根目录同时还有多个子目录 Skill，预览中提示结构混合，只允许纳管一级子目录 Skill。

原因是 root skill 会让“全局目录”和“单个 Skill 源目录”变成同一个路径，后续删除、取消纳管、同步目标清理、备份恢复都更容易误伤整个全局目录。后续如确实要支持 root skill，必须单独设计 sentinel 和删除保护，不能复用普通 `central_path` 解析链路。

### Skill 名称解析

优先级：

1. `SKILL.md` frontmatter 中的 `name`。
2. 目录名。

如果多个目录解析出同一个 name：

- 标记为冲突。
- 不自动纳管。
- 前端提示用户修改目录名或 `SKILL.md` name。

### 跳过规则

跳过：

- 隐藏目录，例如 `.git`、`.cache`。
- 普通文件。
- 没有 `SKILL.md` 的目录。
- broken symlink。
- self symlink。
- 无法读取的目录。

第一版不纳管无 `SKILL.md` 的目录。因为 issue 的诉求是全局 Skills 目录，保守识别更安全。

## 路径安全规则

路径切换和纳管必须复用或扩展现有 sync overlap 防护。

必须阻止：

- 新全局目录等于某个已启用工具目标目录。
- 新全局目录位于某个工具目标目录内部。
- 某个工具目标目录位于新全局目录内部。
- 新全局目录等于当前某个 Skill 的工具 target。
- 新全局目录是文件。
- 新全局目录不可读。
- 新全局目录解析后等于当前某个 Skill 源目录但不是中央仓库根目录的合法子级。
- 新全局目录根部 `SKILL.md` 被当作 root skill 纳管。

建议强警告或阻止：

- 新全局目录不可写。
- 新全局目录在 WSL UNC 路径下，但当前本机写入/监听能力不稳定。

路径比较不能只比较用户输入字符串。实现必须在可行时对以下路径做 canonicalize / realpath 后再做 overlap 检查：

- 新全局目录。
- 当前旧中央仓库目录。
- 已管理 Skill 的解析后 source path。
- 所有已同步工具 target path。
- custom tool 的 runtime skills 目录。

如果新全局目录本身是 symlink，且真实路径与工具目标目录、旧中央仓库、任一 Skill source 重叠，必须阻止或给出 blocking error，不能只给普通 warning。

## 删除语义

这是第一版必须明确改好的地方。

删除语义不能只靠前端文案保护。后端 API 必须显式区分“取消纳管”和“删除源文件”。

建议把现有删除命令升级为：

```text
skills_delete_managed(
  skill_id: String,
  options: DeleteManagedSkillOptionsDto
) -> DeleteManagedSkillResultDto
```

```ts
interface DeleteManagedSkillOptionsDto {
  delete_source_files: boolean;
}
```

默认 `delete_source_files=false`。后端必须根据该参数决定是否删除 source path，不能继续无条件删除 `central_path`。

### source_type=central

默认删除行为：

```text
取消纳管 + 清理工具目标目录 + 删除 DB 记录
```

默认不删除源文件。

前端按钮文案建议：

- 主按钮：`取消纳管`
- 危险次级入口：`同时删除源文件`

危险操作必须二次确认，并清楚展示将删除的实际源路径。

如果 `delete_source_files=true`：

- 后端必须再次解析 source path。
- source path 必须在当前中央仓库目录内。
- source path 不能等于当前中央仓库根目录。
- source path 不能是空路径、`.`、`./`。
- source path 不能与任一工具 target path overlap。
- 删除失败时也要尽量删除 DB 记录和清理工具目标，结果中明确返回源文件删除失败。

### source_type=local / git

在默认 app data 中央仓库下，现有行为可以暂时保持：

```text
删除 DB 记录 + 清理工具目标目录 + 删除中央仓库副本
```

但当当前中央仓库是用户自定义外部目录时，例如 `~/.agent/skills`，删除语义必须更保守：

```text
取消纳管 + 清理工具目标目录 + 删除 DB 记录
```

默认不删除源文件，即使该 Skill 的 `source_type` 仍是 `local` 或 `git`。

原因是用户看到的是自己选择的全局目录，不会自然区分“这个目录里的某个 Skill 是 AI Toolbox 从 Git 复制来的副本”还是“用户自己维护的全局目录源”。如果要删除源文件，必须走同一个危险选项：

```text
同时删除源目录文件
```

前端需要按当前中央仓库是否为默认 app data 目录展示差异：

- 默认 app data 目录：可以沿用旧的“删除中央仓库副本”语义，但仍建议展示实际路径。
- 自定义外部目录：默认只取消纳管；删除源文件必须显式勾选。

### 清空所有 Skills

设置页“清空所有 Skills”不能继续简单逐个调用无参数删除。

第一版应改为：

- 默认只删除管理记录并清理工具目标。
- 对 `source_type=central` 不删除源文件。
- 对位于用户自定义外部中央仓库下的 `local` / `git` Skill 也不删除源文件。
- 如果要同时删除源文件，必须提供单独危险开关，并在确认 Modal 中列出将删除的源目录数量和示例路径。
- root skill 不允许进入清空流程，因为第一版不纳管 root skill。

## 更新语义

### source_type=central

用户点击更新时，不应该从 `source_ref` 复制内容覆盖中央仓库。

建议语义：

1. 检查源路径是否存在。
2. 重新读取 `SKILL.md` description。
3. 重新计算 content hash。
4. 更新 DB 的 hash 和更新时间。
5. 对 copy 模式目标重新复制。
6. 对 symlink/junction 目标只刷新状态，不重复写目标。

前端文案建议：

- 对 central Skill 显示：`刷新`
- tooltip：`重新读取全局目录中的 Skill 内容并同步 copy 目标`

### source_type=local / git

继续沿用现有“从源重新拉取 / 复制到中央仓库副本”的更新语义。

## 前端方案

### 设置页入口

当前 Skills 设置页中的“Skills 存储路径”建议改名为：

```text
全局 Skills 目录
```

布局建议：

```text
全局 Skills 目录    /Users/me/.agent/skills                 [打开] [更改] [恢复默认] [扫描]
                  全局目录是同步到各工具的源目录，可用外部编辑器或 Git 管理。
```

按钮语义：

- `打开`：打开当前目录。
- `更改`：选择或输入新的全局目录，进入路径切换预览。
- `恢复默认`：快捷切回默认 app data 目录，但仍进入路径切换预览和迁移确认；当前已经使用默认目录时禁用。
- `扫描`：辅助入口，扫描当前全局目录，发现未纳管 Skill。

`恢复默认` 不应藏在更多菜单里。它是路径设置的直接反向操作，应与 `更改` 放在同一行，便于用户从自定义全局目录退回 AI Toolbox 默认目录。

### 扫描目录入口位置

“扫描当前全局目录”是用户手动往当前全局目录新增 Skill 后最自然会用到的动作，因此不能只藏在设置弹窗里。

第一版建议放两个入口：

1. **主入口：Skills 页面右上角选项浮层的“数据管理”区域**
   - 文案：`扫描全局目录`
   - 位置：与分组管理、Inventory 导入/导出同一类 action item。
   - 理由：用户新增文件后通常会回到 Skills 列表页刷新管理状态；放在页面数据管理区比进入设置弹窗更直接。

2. **辅助入口：Skills 设置弹窗的“全局 Skills 目录”行**
   - 文案：`扫描`
   - 位置：路径右侧，与 `打开`、`更改` 同一行。
   - 理由：用户刚查看或修改全局目录时，顺手扫描该目录也合理。

不建议把 `扫描全局目录` 做成 Skills 页面顶部常驻主按钮。页面顶部已经有添加、导入、设置和视图选项；扫描属于数据维护动作，放进选项浮层可以避免主工具栏继续膨胀。

### 页面选项浮层调整

Skills 页面右上角 sliders 选项浮层建议继续保持“视图与筛选 / 数据管理”结构。

在 `数据管理` 分区增加 action item：

```text
扫描全局目录
查找当前全局 Skills 目录中尚未纳管的 Skill。
```

点击后：

1. 关闭选项浮层。
2. 调用 `skills_scan_central_repo`。
3. 打开“纳管 / 修复全局目录中的 Skills”Modal。
4. 如果没有未纳管项、没有可修复项、也没有需要用户处理的 warning，用轻量结果态提示“没有发现新的 Skill”，不要打开空白大 Modal。

### 更改目录流程

1. 点击 `更改`。
2. 打开目录选择器。
3. 选中目录后调用 `skills_preview_central_repo_path`。
4. 打开“更改全局 Skills 目录”预览 Modal。
5. 用户确认选项。
6. 调用 `skills_apply_central_repo_path_change`。
7. 成功后刷新：
   - managed skills 列表。
   - central repo path。
   - tool status。
   - tray menu。

### 恢复默认目录流程

1. 点击 `恢复默认`。
2. 前端获取默认目录，或让后端直接以默认目录生成 preview。
3. 调用 `skills_preview_central_repo_path`，其中目标路径为默认目录。
4. 打开同一个“更改全局 Skills 目录”预览 Modal，但顶部标记为：

```text
恢复默认目录
```

5. Modal 中继续展示：
   - 默认目录中可纳管的新 Skill。
   - 当前已管理但默认目录缺失的 Skill。
   - 从当前自定义目录迁移回默认目录的可迁移项。
   - 冲突项。
   - 应用后是否重新同步。
6. 用户确认后调用 `skills_apply_central_repo_path_change`，并传入 `use_default_path=true`。
7. 后端应用成功后清除自定义 `central_repo_path` 覆盖。
8. 成功后刷新 managed skills、central repo path、tool status 和 tray menu。

恢复默认和更改目录必须共用同一个预览/应用流程。不能在前端直接调用旧的 `skills_set_central_repo_path(defaultPath)`，否则会绕过迁移预览，也会把默认路径硬写成一个自定义路径。

### 路径切换预览 Modal

Modal 标题：

```text
更改全局 Skills 目录
```

顶部摘要：

```text
新目录：/Users/me/.agent/skills
检测到 12 个 Skill，其中 5 个未纳管，2 个当前已管理 Skill 在新目录中缺失。
```

建议分区：

1. `可纳管的新 Skill`
   - 默认勾选。
   - 展示 name、目录名、description。

2. `当前已管理但新目录缺失`
   - 展示 name、当前源路径。
   - 展示切换后将解析到的新路径。
   - 如果旧目录源文件存在，归入“从旧目录迁移”分区。
   - 如果旧目录源文件也不存在，只能作为 source warning 风险展示。

3. `从旧目录迁移`
   - 展示可复制到新目录的已管理 Skill。
   - 默认勾选可迁移项。
   - 冲突项只展示，不可勾选。
   - 恢复默认目录时，这一分区表示“从当前自定义目录迁移回默认目录”。

4. `冲突`
   - 展示冲突原因。
   - 有冲突时禁用确认，或只允许跳过冲突项。

5. `可修复缺失记录`
   - 展示当前 DB 记录源缺失，但新目录中扫描到同名 Skill 的项。
   - 默认不自动勾选，或采用明确确认文案“绑定到此目录”。
   - 用户确认后更新既有记录的 `central_path`，保留分组、备注和已同步工具。

6. `根目录 SKILL.md`
   - 如果发现全局目录根部有 `SKILL.md`，只展示 warning。
   - 第一版不允许勾选纳管 root skill。

7. `同步选项`
   - `应用后重新同步已启用工具`，默认勾选。
   - 展示将受影响的工具目标数量。
   - 如果取消勾选，展示 symlink/junction 仍可能指向旧目录、copy 仍是旧内容的警告。

底部按钮：

- `取消`
- `应用更改`

如果存在可能导致 source warning 的缺失项，确认按钮前需要明确提示：

```text
如果不复制缺失项，相关 Skill 会在切换后显示源目录缺失，且无法同步。
```

### 扫描当前目录流程

1. 点击 `扫描目录`。
2. 调用 `skills_scan_central_repo`。
3. 打开“纳管 / 修复全局目录中的 Skills”Modal。
4. 展示未纳管 Skill、可修复缺失记录、冲突和跳过项。
5. 用户勾选新增纳管项后调用 `skills_adopt_central_repo_skills`。
6. 用户确认修复项后调用 `skills_repair_central_repo_skill` 或同等后端命令。
7. 成功后刷新列表。

### 纳管 Modal

标题：

```text
纳管 / 修复全局目录中的 Skills
```

内容：

- 检测数量。
- 未纳管列表。
- 可修复缺失记录列表。
- 冲突列表。
- 跳过列表。
- 根目录 `SKILL.md` warning。

按钮：

- `取消`
- `纳管选中`

文案要强调：

```text
纳管只会创建 AI Toolbox 管理记录，不会移动、复制或覆盖源目录文件。
```

修复文案要强调：

```text
修复只会把现有管理记录绑定到当前目录中的 Skill，保留分组、备注和已同步工具设置。
```

### Skill 卡片展示

对 `source_type=central`：

- 来源标签：`全局目录`
- 打开路径：打开实际 `central_path`
- 复制路径：复制实际源路径
- 更新按钮文案/tooltip 使用“刷新”语义
- 删除菜单项显示“取消纳管”

如果 source health warning：

- 继续沿用现有 warning 视觉。
- 文案应提示用户检查全局目录中的对应文件夹是否仍存在。

### 删除确认

对 `source_type=central`：

默认确认文案：

```text
将取消 AI Toolbox 对「code-review」的管理，并从已同步工具中清理目标目录。源目录不会被删除：
/Users/me/.agent/skills/code-review
```

危险选项：

```text
同时删除源目录文件
```

只有用户勾选危险选项后，按钮文案改为：

```text
取消纳管并删除源文件
```

### i18n 约束

新增或修改文案时必须使用现有脚本：

```bash
pnpm i18n:set-key <key> --zh-CN "中文" --en-US "English" --write
```

更新已有文案时加：

```bash
--allow-overwrite
```

不要手动 patch `web/i18n/locales/*.json`。

## WSL / SSH 影响

第一版不改变 WSL/SSH Skills 同步的大方向：本机中央仓库仍是源，WSL/SSH 远端都有自己的统一中央仓库，再由远端中央仓库链接或复制到远端工具目录。这些链路不是普通 file mappings，不能把工具当前 runtime skills 目录当成源目录。

这里必须区分两个概念：

- `本机全局 Skills 源目录`：用户在本机选择的 source-of-truth，例如 `~/.agent/skills`、`D:\Skills` 或默认 `app_data_dir/skills`。
- `远端同步镜像目录`：AI Toolbox 为 WSL/SSH 同步维护的远端 staging/mirror 目录，当前实现是 `~/.ai-toolbox/skills`。

建议第一版不要因为用户把本机全局目录改成 `~/.agent/skills`，就自动把 WSL/SSH 远端镜像目录也改成 `~/.agent/skills`。原因：

- 本机 `~/.agent/skills` 和 WSL `~/.agent/skills` 不是同一个路径语义；前者是 Windows/macOS/Linux 本机用户目录，后者是 WSL distro 或 SSH 远端用户目录。
- WSL/SSH 远端目录是同步投影目标，不是新的 source-of-truth。自动改成用户习惯目录，容易让用户误以为可以直接在远端目录里编辑并回写本机。
- 如果远端 `~/.agent/skills` 已经是用户自己维护的 Git repo 或手工 Skills 仓库，自动同步会带来覆盖、删除、冲突和权限风险。
- 现有 WSL/SSH 同步清理逻辑以 app-owned mirror 为前提；把目标改到用户目录前，需要单独设计清理边界、冲突预览和“不删除远端用户文件”的保护。

因此推荐命名上把远端目录叫 `WSL/SSH Skills 镜像目录` 或 `远端同步缓存目录`，不要在 UI 里也叫“全局 Skills 目录”。如果后续确实要支持远端也使用 `~/.agent/skills`，应作为第二期的 per-distro / per-connection 高级设置，而不是跟随本机路径隐式改变。

### WSL 同步语义

WSL Skills 同步应继续使用当前 resolver 得到的本机中央仓库路径：

- 本机源：当前 `central_repo_path`，缺省时为 `app_data_dir/skills`。
- WSL 远端镜像目录：第一版继续使用既有统一目录 `~/.ai-toolbox/skills`，不自动跟随本机全局目录名改成 `~/.agent/skills`。
- WSL 工具目标：由远端中央仓库再链接或复制到 Claude / Codex / OpenCode 等工具自己的 skills 目录。

路径切换、恢复默认、扫描纳管、取消纳管、删除源文件、重同步工具目标后，都应 emit `skills-changed`。如果 WSL 自动同步已开启，并且 Skills 模块同步开关启用，事件监听应触发 WSL Skills 同步，让远端中央仓库和远端工具目标重建到新源内容。

WSL 自动同步关闭时，不应静默假装远端已更新。路径切换结果页需要展示“WSL 远端待同步”，并提供去设置页手动同步的引导。此提示对“更改目录”和“恢复默认目录”都适用。

前端展示时应明确写出远端镜像目录，例如：

```text
本机全局目录：~/.agent/skills
WSL 镜像目录：~/.ai-toolbox/skills
```

不要写成“WSL 全局目录已切换到 ~/.agent/skills”，因为第一版并没有改变 WSL 远端镜像目录。

WSL Direct 需要单独遵守既有 skip/direct 语义：

- 如果某个工具已经配置为 WSL Direct，工具 runtime 文件本身可能已经直接落在 WSL 路径。
- 普通 WSL mapping sync 对 Direct 工具通常会跳过，避免 Windows 本机文件再覆盖 Direct 目标。
- Skills 方案里不能因为全局目录切换，就强行把 Windows 本机中央仓库硬同步到某个 WSL Direct 工具 runtime 目录。
- 第一版只要求 Skills WSL 同步继续以“远端中央仓库 -> 远端工具目录”为边界，是否跳过 Direct 工具目标应沿用现有工具级规则。

源缺失时，WSL 同步不能默认把远端已有内容当成“需要删除”。推荐语义：

- `source_health=missing` 的 managed Skill 在同步预览或同步结果里标为失败/跳过。
- 不自动删除 WSL 远端中央仓库中同名目录，也不自动删除远端工具目录中的同名目标。
- 只有用户明确执行“取消纳管并清理目标”或“删除源文件并清理目标”这类业务动作时，才清理远端目标。

### SSH 同步语义

SSH 不应按 WSL 的事件驱动自动同步模型来描述。

第一版建议明确：

- SSH Skills 同步源仍是当前本机中央仓库，不复用普通 file mappings。
- SSH 远端镜像目录继续使用统一目录 `~/.ai-toolbox/skills`，不自动跟随本机全局目录名改成 `~/.agent/skills`。
- SSH 远端工具目标由远端中央仓库再链接或复制到远端工具 skills 目录。
- 路径切换成功后可以记录/返回“SSH 远端待同步”，但不假设存在 `ssh-sync-request-skills` 这类自动事件。
- 用户需要在设置页手动执行 SSH Sync Now，或通过既有“启用 SSH 同步 / 切换 active connection 后全量同步”链路，把新中央仓库内容同步到远端。

如果当前没有可用 active SSH connection，路径切换结果页应显示“SSH 未同步，待连接可用后同步”，而不是报路径切换失败。路径切换本身是本机状态变更；SSH 是后续远端投影。

源缺失时，SSH 同步也应与 WSL 保持一致：默认跳过缺失源并报告，不自动删除远端仍存在的中央仓库目录或工具目标。

SSH 结果页同样应区分：

```text
本机全局目录：~/.agent/skills
SSH 镜像目录：~/.ai-toolbox/skills
```

如果未来支持自定义 SSH 镜像目录，才把这里显示为用户配置的远端路径。

### 远端同步状态表

WSL/SSH 远端清理必须区分“DB 记录仍存在但源缺失”和“DB 记录已经不存在”。

| 本机 DB 记录 | 本机源目录 | 远端镜像目录 | 行为 |
|-------------|------------|--------------|------|
| 存在 | 存在 | 不存在或 hash 不同 | 上传/更新远端镜像，并重建启用工具链接 |
| 存在 | 存在 | 已存在且 hash 相同 | 跳过内容上传，但确认启用工具链接正确 |
| 存在 | 缺失 | 已存在 | 标记失败/跳过，不删除远端镜像，不删除远端工具目标 |
| 不存在 | 不适用 | 已存在 | 视为取消纳管后的 orphan，可清理远端镜像和远端工具链接 |
| 存在但某工具未启用 | 存在 | 已存在 | 只清理该工具链接，不删除远端镜像 |

这个表是恢复和迁移场景的安全边界：恢复 DB 后外部目录缺失时，不能因为本机源缺失就删除远端仍可用内容；用户明确取消纳管或删除记录后，远端 orphan 清理仍然可以执行。

### 需要额外评估

- 全局目录在 WSL UNC 路径时，本机 Rust 文件操作和 WSL sync 是否会重复跨边界读写。
- 全局目录在网络盘或同步盘时，文件监听和 hash 计算是否可能慢。第一版不做 watcher，只做用户触发扫描，可以降低风险。

## 设置页备份恢复影响

设置页的本地备份、WebDAV 备份和恢复是全局能力，不应塞进 Skills 页面内完成。但自定义全局 Skills 目录会改变恢复后的 source-of-truth，需要在方案里明确边界。

### 备份内容边界

备份包至少应保留 DB 里的 Skills 管理元数据：

- `skill_settings:skills.central_repo_path`。
- managed skill 记录。
- 每个 Skill 的 `source_type`、`central_path`、同步偏好和工具目标状态等 DB 元数据。

默认 app data 中央仓库属于 AI Toolbox 管理目录。如果现有备份策略已经备份默认 Skills 目录内容，恢复后应能把默认中央仓库内容恢复到新 app data 目录，并让 managed skill 记录正常解析。

用户自定义外部全局目录不应默认完整打包，例如：

- `~/.agent/skills`
- `D:\Skills`
- 某个用户自己维护的大型 Git repo
- 网络盘或同步盘目录

原因是这些目录可能很大，也可能包含用户并不希望放进 AI Toolbox 备份包的 Git 历史、私有文件或外部工具数据。第一版应只备份“指向它的配置和管理记录”，不默认备份外部目录内容。

如果未来要支持“连同外部全局目录内容一起备份”，应作为显式选项或自定义备份项单独设计，不能默认启用。恢复时还要处理 `central_repo_path` 与实际恢复路径的映射，不能把旧机器绝对路径强行写到新机器。

### 恢复后的状态检查

恢复可能来自本地备份，也可能来自 WebDAV。恢复成功后设置页本来就应要求用户重启或刷新应用，因为前端 store、路由状态、模块缓存和后端内存态都可能仍指向恢复前的数据。

重启或刷新后，Skills 页面和 Skills 设置弹窗应执行 source health 检查：

- 如果 `central_repo_path` 指向当前机器不存在的路径，显示全局目录 warning。
- 如果 managed skill 记录存在，但对应 `central_path` 源目录缺失，显示 Skill 级 source warning。
- 不自动删除 managed skill 记录。
- 不自动创建空 Skill 目录。
- 不自动把外部路径改回默认目录。

建议在实现时二选一：

- 复用 `skills_get_managed_skills` / 路径状态接口中的 source health 诊断。
- 或新增轻量 `skills_post_restore_check`，专门给设置页 restore 后的“恢复结果检查”使用。

source warning 应提供明确入口：

- `更改目录`：选择当前机器真实存在的新全局目录。
- `恢复默认`：切回 AI Toolbox 默认 app data 目录，但仍走 preview/apply。
- `扫描当前目录`：当前目录存在但有未纳管项时，用扫描纳管。
- `迁移复制`：把旧记录对应的缺失项从另一个可用目录复制到当前目录，仍不覆盖已有项。

### 对同步和托盘的影响

恢复了 DB 但没有恢复外部全局目录内容时，WSL/SSH/托盘同步不能把缺失源当作成功：

- 托盘或页面状态应展示 source warning。
- WSL 自动同步触发时，应跳过缺失源并报告，不删除远端现有内容。
- SSH 手动同步时，应跳过缺失源并报告，不删除远端现有内容。
- copy / symlink / junction 工具目标重建也应以源存在为前提。

备份文件过滤规则不能误删 DB 中的 Skill 元数据。跳过某些 auth/config 外部文件不应影响 `skill_settings` 和 managed skill 记录；它们是数据库业务状态，不是可选外部文件。

## Inventory 影响

Inventory JSON 应继续导出完整管理清单。

对 `source_type=central`：

- `source_type` 应导出为 `central`。
- `central_path` 应导出相对路径。
- 不导出绝对 `central_repo_path`，避免跨设备导入污染路径。

导入 Inventory 时：

- 如果 inventory 中包含 `source_type=central`，导入预览必须展示当前全局目录，并提示用户先确认或更改全局目录。
- 如果目标机器当前全局目录中存在对应 `central_path`，可以恢复管理关系。
- 如果不存在，应标记 source warning 或在预览里提示缺失。
- 不自动创建空 Skill 目录。
- 不从 inventory 写入绝对 `central_repo_path`；如果用户需要使用某个外部目录，应通过全局目录设置流程选择该目录。

## 测试计划

### Rust 后端测试

至少补以下测试：

1. 预览新全局目录能识别一级 Skill。
2. 预览能区分 matched / unmanaged / missing。
3. 多个目录解析出同名 name 时返回 conflict。
4. 纳管中央目录 Skill 不复制、不移动源目录。
5. `source_type=central` 更新只刷新 hash，不覆盖源目录。
6. `source_type=central` 删除默认不删除源目录。
7. 路径切换复制缺失项时不覆盖已有目录。
8. 路径 overlap 时阻止应用。
9. `central_path` 与 `name` 不一致时，同步源路径使用 `central_path`，目标目录使用 `name`。
10. 从旧中央仓库迁移到新目录时，保留既有 Skill 的相对 `central_path`。
11. 迁移时保留既有 `source_type`，扫描纳管的新 Skill 才使用 `source_type=central`。
12. 恢复默认目录时清除自定义 `central_repo_path` 覆盖，而不是把默认路径硬写为自定义值。
13. 路径切换后 symlink / junction 目标会重建到新中央仓库 source。
14. 路径切换后 copy 目标会从新中央仓库重新复制。
15. 源缺失且未迁移的 Skill 不会在路径切换时自动删除已有工具目标。
16. 备份恢复后，如果 DB 中的自定义 `central_repo_path` 在当前机器不存在，路径状态返回 source warning，而不是自动清空设置。
17. 备份恢复后，如果默认中央仓库内容随备份恢复，managed skill 能通过默认 app data 路径正常解析。
18. 外部全局目录未随备份恢复时，managed skill 元数据仍保留，缺失源只表现为 warning。
19. WSL 自动同步开启时，路径切换 / 恢复默认 / 扫描纳管成功后会触发 Skills WSL 同步请求。
20. WSL 自动同步关闭时，路径切换命令返回远端待同步提示 metadata。
21. SSH 不依赖 WSL 事件；路径切换命令返回 SSH 待手动同步提示 metadata。
22. WSL/SSH 同步使用新中央仓库内容重建远端 `~/.ai-toolbox/skills` 和远端工具链接。
23. WSL/SSH 同步遇到缺失源时跳过并报告，不默认删除远端中央仓库目录或工具目标。
24. 本机全局目录设置为 `~/.agent/skills` 时，WSL/SSH 第一版仍同步到远端镜像目录 `~/.ai-toolbox/skills`，不会隐式改写远端 `~/.agent/skills`。
25. 全局目录根部存在 `SKILL.md` 时，只返回 warning，不允许生成 `central_path=""` 或 `central_path="."` 的 managed skill。
26. 删除 central Skill 默认不删除 source path；只有 `delete_source_files=true` 才删除源目录。
27. 自定义外部中央仓库下，删除 `local` / `git` Skill 默认也不删除源目录。
28. 设置页清空所有 Skills 默认只取消纳管和清理工具目标，不删除外部全局目录源文件。
29. 路径切换 apply 在必选迁移失败时不写入 `central_repo_path`，不创建/修改 managed skill 记录。
30. 扫描当前目录时，同名但现有 source missing 的记录返回 repair candidate，可更新既有 `central_path` 并保留分组、备注和同步目标。
31. `source_type=central` 在本地 copy、WSL、SSH 同步前重新计算 hash，避免外部编辑后远端误判无需同步。
32. symlink/canonical path overlap 能阻止通过 symlink 绕过工具目标目录检查。

### 前端测试

至少覆盖：

1. 设置页显示“全局 Skills 目录”。
2. 点击更改后展示预览 Modal。
3. 缺失项和冲突项有清晰展示。
4. 扫描当前目录后展示纳管 Modal。
5. `source_type=central` Skill 卡片来源标签显示“全局目录”。
6. 删除 central Skill 时默认文案是“取消纳管”，并说明不删除源文件。
7. Skills 页面选项浮层的数据管理区包含“扫描全局目录”入口。
8. 设置弹窗路径行包含辅助“扫描”入口。
9. 设置弹窗路径行包含“恢复默认”入口，当前已是默认目录时禁用。
10. 点击“恢复默认”后展示同一套路径预览 Modal，而不是直接改路径。
11. 路径预览 Modal 展示受影响工具目标数量。
12. 用户取消“应用后重新同步已启用工具”时展示旧链接 / 旧 copy 内容风险提示。
13. 设置页 restore 成功后保留既有强制重启/刷新提示，不在当前内存态里继续展示旧路径结论。
14. 恢复后进入 Skills 页面，如果自定义全局目录不存在，展示全局目录 warning 和“更改目录 / 恢复默认 / 扫描当前目录”入口。
15. 路径切换结果页能区分本机工具已重同步、WSL 待同步、SSH 待手动同步。
16. active SSH connection 不可用时，结果页展示“SSH 未同步，待连接可用后同步”，而不是把本机路径切换标为失败。
17. 路径切换结果页明确展示“本机全局目录”和“WSL/SSH 镜像目录”，避免用户误解远端已切换到 `~/.agent/skills`。
18. 路径预览 Modal 展示“可修复缺失记录”分区，用户可选择绑定到扫描出的目录。
19. 根目录 `SKILL.md` 只作为 warning 展示，不出现可勾选纳管项。
20. 删除确认 Modal 对 central Skill 默认按钮为“取消纳管”，危险勾选后才允许“取消纳管并删除源文件”。
21. 自定义外部中央仓库下，local/git Skill 删除确认默认也说明不会删除源文件；危险勾选才删除。
22. 设置页清空所有 Skills 的确认 Modal 明确区分“只清空管理记录”和“同时删除源文件”的危险选项。
23. 扫描 Modal 同时支持新增纳管和修复绑定，并清楚说明修复不会移动/覆盖源文件。

### 手动验证

至少验证：

1. 空目录切换。
2. 已有 `~/.agent/skills` 目录切换。
3. 目录里有多个合法 Skill。
4. 目录里有同名冲突 Skill。
5. 外部编辑器修改 Skill 后点击刷新并同步 copy 目标。
6. symlink/junction 目标在切换路径后重新指向新源。
7. 删除 central Skill 后源文件仍存在。
8. WSL/SSH 同步不会把工具目录误当源目录。
9. 从默认旧中央仓库迁移到新目录，已管理 Skill 文件被复制到新目录且旧目录未删除。
10. 用户手动往当前全局目录新增 Skill 后，从 Skills 页面选项浮层扫描并纳管。
11. 从自定义全局目录恢复到默认目录，可迁移项被复制回默认目录，自定义路径覆盖被清除。
12. 迁移后 Claude/Codex 等工具目录中的 symlink/junction 指向新中央仓库，而不是旧中央仓库。
13. 迁移后 copy 模式工具目标内容来自新中央仓库。
14. 跳过重同步时，工具目录中旧 symlink 仍指向旧目录，并且 UI 给出明确风险提示。
15. WSL/SSH 自动同步关闭时，路径切换结果提示远端同步待执行。
16. 设置页本地备份恢复：默认中央仓库内容随备份恢复时，重启后 Skill 列表能正常解析源目录。
17. 设置页 WebDAV 恢复：DB 恢复了外部 `central_repo_path` 但目录不存在时，重启后出现 source warning，不自动删除记录。
18. 外部全局目录不默认进入备份包，恢复后不误创建空目录、不误清理 managed skill 元数据。
19. WSL 自动同步开启时，路径切换后远端 `~/.ai-toolbox/skills` 和远端工具目标更新为新中央仓库内容。
20. WSL 自动同步关闭时，路径切换后本机成功、远端待同步提示可见，手动同步后远端更新。
21. SSH active connection 可用时，设置页手动 Sync Now 后远端 `~/.ai-toolbox/skills` 和工具目标更新。
22. SSH active connection 不可用时，路径切换结果只提示待连接后同步，不阻断本机路径切换。
23. WSL/SSH 同步遇到恢复后的缺失源时不删除远端已有目录，除非用户执行明确的取消纳管/清理目标动作。
24. 本机全局目录为 `~/.agent/skills` 时，WSL/SSH 远端 `~/.agent/skills` 不被自动创建、覆盖或清理；实际同步镜像仍在 `~/.ai-toolbox/skills`。
25. 全局目录根部放置 `SKILL.md` 时，页面只提示 warning，不允许纳管根目录本身。
26. 自定义外部全局目录下删除 central/local/git Skill，默认源文件仍保留；勾选危险删除时才删除实际源目录。
27. 设置页清空所有 Skills 后，外部全局目录中的源目录仍保留。
28. 外部编辑 central Skill 后，不点击刷新直接执行 WSL/SSH 同步，远端内容也能更新。
29. 扫描到与缺失记录同名的 Skill 时，选择“绑定到此目录”后保留原分组、备注和已同步工具设置。
30. 新全局目录是 symlink 且真实路径落在工具目录内部时，路径切换被阻止。

## 分阶段落地

### 阶段 1：后端基础能力

- 新增路径预览命令。
- 新增当前中央目录扫描命令。
- 新增中央目录 Skill 纳管命令。
- 新增缺失记录 repair/rebind 命令或纳管命令中的 repair options。
- 新增旧中央仓库到新中央仓库的迁移预览和复制能力。
- 新增默认中央仓库路径查询或在路径状态中返回默认路径 metadata。
- 支持恢复默认目录时清除 `central_repo_path` 自定义覆盖。
- 支持路径切换后重建 symlink / junction / copy 目标，并返回失败明细。
- 新增或兼容 `source_type=central`。
- 调整 central Skill 的更新和删除语义。
- 调整删除 API，增加 `delete_source_files` 语义，并覆盖设置页清空所有 Skills。
- 禁止 root `SKILL.md` 纳管，防止 `central_path` 解析到中央仓库根目录。
- 路径切换 apply 明确 preflight、迁移、DB 事务和重同步的提交边界。
- WSL/SSH 和本地 copy 同步前刷新 central Skill hash。
- canonicalize / realpath 后执行 source-target overlap 检查。
- 路径切换 / 恢复默认 / 扫描纳管后返回 WSL/SSH 待同步 metadata。
- WSL Skills 同步确认使用当前中央仓库 resolver，并重建远端 `~/.ai-toolbox/skills` 与远端工具链接。
- SSH Skills 同步确认使用当前中央仓库 resolver，并保持手动/full-sync 语义。
- WSL/SSH 远端镜像目录第一版保持 app-owned `~/.ai-toolbox/skills`，不从本机 `central_repo_path` 推导远端路径。
- 备份恢复后的 Skills source health 检查复用现有诊断或新增 `skills_post_restore_check`。
- 补 Rust 回归测试。

### 阶段 2：前端设置入口

- 设置页增加全局目录更改入口。
- 设置页路径行增加“恢复默认”快捷入口。
- 增加路径切换预览 Modal。
- 在 Skills 页面选项浮层的数据管理区增加“扫描全局目录”主入口。
- 在设置弹窗路径行增加“扫描”辅助入口。
- 增加扫描当前目录和“纳管 / 修复”Modal。
- 路径切换预览 Modal 增加“从旧目录迁移”分区。
- 路径切换预览 Modal 增加“可修复缺失记录”和 root `SKILL.md` warning。
- 路径切换预览 Modal 展示受影响工具目标，并在跳过重同步时给出旧链接风险提示。
- 路径切换结果页展示本机工具、WSL、SSH 三类后续状态。
- 设置页备份恢复成功后继续强制用户重启/刷新；重启后 Skills 页面展示恢复后的 source warning 和修复入口。
- 删除确认和清空所有 Skills 确认按是否删除源文件分流，危险删除必须单独勾选。
- 保存后刷新列表、路径、工具状态和托盘。
- 用 i18n 脚本新增/更新文案。

### 阶段 3：列表和卡片体验

- Skill 卡片展示“全局目录”来源。
- central Skill 的更新文案改成“刷新”语义。
- central Skill 删除默认改成“取消纳管”。
- source warning 文案适配全局目录。

### 阶段 4：同步和跨端验证

- 验证路径切换后本地工具重同步。
- 验证 copy / symlink / junction 模式。
- 验证 WSL 自动同步开启 / 关闭两种路径切换结果。
- 验证 SSH 手动同步、active connection 不可用提示和恢复后同步。
- 验证设置页本地备份 / WebDAV 恢复后的默认目录、外部目录缺失和 source warning。
- 补必要测试。
- 视改动范围运行全量测试集合。

## 待确认问题

1. 第一版是否只扫描一级子目录？
   - 建议：是。

2. `SKILL.md` name 和目录名不一致时是否允许？
   - 建议：允许。`central_path` 保存实际目录，`name` 用于展示和目标目录。

3. central Skill 删除默认是否只取消纳管？
   - 建议：是。删除源文件必须是额外危险操作。

4. 路径切换时是否默认复制当前已管理但新目录缺失的 Skill？
   - 建议：默认勾选复制，但绝不覆盖已有同名目录。

5. issue 中“自定义每个 Agent 同步目标路径”是否放第二期？
   - 建议：是。第一期先解决全局源目录，目标路径 override 另开方案。

6. 当前“清空所有 Skills”功能对 central Skill 是否应只取消纳管？
   - 建议：需要改。否则清空操作会误删用户全局目录源文件。

7. Inventory 导入遇到 central Skill 缺失时是否允许恢复为 warning 状态？
   - 建议：允许，但预览必须明确提示缺失。

8. 是否需要支持无 `SKILL.md` 的目录纳管？
   - 建议：第一版不支持。后续可加高级开关。

9. 扫描全局目录是否放在 Skills 页面主工具栏？
   - 建议：不放主工具栏。主入口放在右上角选项浮层的“数据管理”区域，设置弹窗路径行只保留辅助入口。

10. 从旧目录迁移到新目录后，是否自动删除旧目录？
   - 建议：不删除。只复制，成功后提示用户旧目录仍保留。

11. 恢复默认目录是否也走迁移预览？
   - 建议：必须走。恢复默认本质上也是路径切换，不能绕过预览和迁移确认。

12. 恢复默认后数据库里如何表达默认状态？
   - 建议：清除或置空 `skill_settings:skills.central_repo_path`，让 resolver 回到默认 fallback；不要把默认路径硬写成自定义路径。

13. 外部全局目录是否默认进入设置页备份包？
   - 建议：不默认进入。只备份 DB 管理元数据和路径设置；外部目录内容必须是未来显式选项。

14. 恢复后外部 `central_repo_path` 不存在时是否自动切回默认目录？
   - 建议：不自动切。显示 source warning，并提供“更改目录 / 恢复默认 / 扫描当前目录 / 迁移复制”入口。

15. 路径切换后是否自动触发 SSH 同步？
   - 建议：不承诺自动触发。WSL 可以沿用事件驱动自动同步；SSH 仍按设置页手动同步或既有 full-sync 链路处理。

16. 本机全局目录改成 `~/.agent/skills` 后，WSL/SSH 远端镜像目录是否也改成 `~/.agent/skills`？
   - 建议：第一版不改。远端仍使用 AI Toolbox 管理的 `~/.ai-toolbox/skills` 作为镜像目录；远端自定义镜像目录作为第二期 per-distro / per-connection 高级设置单独设计。

17. 第一版是否支持全局目录根部 `SKILL.md` 作为 root skill？
   - 建议：不支持。只展示 warning，避免 `central_path` 解析到中央仓库根目录后误删整个全局目录。

18. 自定义外部中央仓库下，`local` / `git` Skill 删除时是否默认删除源目录？
   - 建议：不默认删除。只要源目录位于用户自定义外部全局目录中，删除源文件都必须走危险确认。

19. 扫描到同名 Skill 但 DB 中已有记录源缺失时，是冲突还是修复候选？
   - 建议：作为修复候选。用户确认后更新既有记录的 `central_path` 和 hash，保留分组、备注、同步目标。

20. central Skill 外部编辑后，WSL/SSH 同步是否需要重新计算 hash？
   - 建议：需要。同步前刷新 hash，避免 DB 旧 hash 导致远端误判无需上传。

## 推荐先评审的关键决策

建议先确认以下关键决策，再进入实现拆分：

1. 第一版聚焦“全局源目录”，不做内置工具目标路径 override。
2. 扫描只识别一级目录下包含 `SKILL.md` 的 Skill。
3. central Skill 删除默认只取消纳管，不删除源文件。
4. 路径切换必须先预览，不能直接保存字符串。
5. 从旧中央仓库切到新目录时，默认提供复制迁移，但不自动删除旧目录。
6. 扫描全局目录主入口放在 Skills 页面选项浮层的数据管理区，设置弹窗只保留辅助入口。
7. 路径设置行提供“恢复默认”快捷入口，且恢复默认也必须走同一套预览/迁移流程。
8. 设置页备份恢复只默认恢复 DB 元数据和默认 app data 中央仓库内容，不默认打包用户外部全局目录。
9. 恢复后路径或 Skill 源缺失只进入 source warning，不自动删除记录、不自动创建空目录、不自动切回默认路径。
10. WSL 跟随事件驱动自动同步语义；SSH 仍以手动/full-sync 为主，路径切换结果页只提示 SSH 待同步状态。
11. 本机全局目录和 WSL/SSH 远端镜像目录分开建模；第一版远端镜像目录固定为 `~/.ai-toolbox/skills`，不自动跟随本机 `~/.agent/skills`。
12. 第一版不纳管 root `SKILL.md`，避免任何 Skill source 解析到中央仓库根目录。
13. 删除 API 必须显式区分取消纳管和删除源文件；自定义外部中央仓库下默认不删除任何 Skill 源目录。
14. 路径切换 apply 的 DB 更新必须尽量原子；重同步失败进入结果页，不回滚已经成功提交的路径切换。
15. 扫描目录需要支持 repair/rebind 缺失记录，而不是把所有同名项都当冲突。
16. central Skill 同步前必须重新计算 hash，覆盖本地 copy、WSL 和 SSH。
