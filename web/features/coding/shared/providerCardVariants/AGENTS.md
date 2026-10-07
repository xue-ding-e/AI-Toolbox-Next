# 供应商卡片三样式（providerCardVariants）

## 一句话职责

- 把「CLI 供应商卡片长什么样」这件事**固定成三种样式**，各 CLI 按自己的 provider 模型形状选用其一，不再各写各的布局。

## Source of Truth

- 样式的事实源就是这三个组件文件本身。`index.ts` 的对照表是**选型规则**（哪个 CLI 用哪种、依据是什么），不是样式定义。
- 各 CLI 传给卡片的 `ProviderCardVariantProps` 是**纯展示数据**：卡片不读任何 CLI 的存储格式，映射由各 CLI 自己的 `*ProviderCard.tsx` 完成。
- `CardShell` 是三种样式共用的外框，**拖拽注册、选中复选框、卡片边框/阴影只在这里定义一次**。

## 三种样式与选型

| 样式 | 何时用 | 判定问题 | 第二行 | 头部主操作 |
|---|---|---|---|---|
| `ClaudeStyleCard` | 没有模型目录（模型写在 provider 配置里） | CLI 是否管理一个模型清单？否 → 此样式 | 带标签的绑定（`默认: …` / `Haiku: …`） | 文字链「应用」 |
| `CodexStyleCard` | 有模型目录，且有单一 active provider | 是，且「应用」有意义 → 此样式 | 自由格式（端点/模型/masked key/备注） | 文字链「应用」 |
| `OpenCodeStyleCard` | 有模型目录，且**没有**单一 active provider | 是，且「启用哪个渠道」不成立 → 此样式 | 固定顺序 `ID • SDK • 端点` | **无**（默认在模型行上选） |

**选型依据是 provider 模型形状，不是个人偏好，也不是「这个 CLI 现在长什么样」。** 一个 CLI 后来获得或失去模型目录，就应该换样式——这是设计意图，不是例外。

## 核心设计决策（Why）

- **三种，不是一套可配置的 prop 组合。** 样式差异的本质是**语义差异**（有没有模型目录、有没有单一 active provider），不是参数差异。做成一套带十几个开关的组件，每个调用点都要重新决定一遍组合，等于把分叉从「文件级」挪到「调用点级」，没有解决问题。
- **`OpenCodeStyleCard` 刻意没有头部「应用」按钮。** 这类 CLI 里多个渠道同时可用，「应用」会暗示其他渠道被关掉。它的「用哪个」表达在**模型行**的「设为默认」上。给这类卡片加回一个头部应用按钮是**语义错误**，不是样式偏好。
- **样式组件只做布局，不做数据映射。** 各 CLI 的 `*ProviderCard.tsx` 保留为薄映射层（把存储形状转成 `ProviderCardVariantProps`）。这样样式统一了，但各 CLI 的存储语义仍留在自己的模块里。
- **`metaEntries` 由调用方决定内容，样式只决定位置。** 这是三种样式能共用同一份 props 的原因：差异在「这行放在哪、怎么排」，不在「这行装什么」。

## 关键约束（改这里之前必读）

- **`CardShell` 的 `setNodeRef` 必须无条件挂载。** dnd-kit 需要测量节点来计算 transform；未注册的节点在父级重新启用拖拽时拖不动。写成 `draggable ? setNodeRef : undefined` 会引入「禁用一次就再也拖不动」的隐性 bug。
- **`useSortable` 每次渲染都必须执行。** 没有 id 时传占位符并置 `disabled`，不要条件调用 hook——违反 hooks 规则会在拖拽开关切换时崩。
- **卡片自己留底部间距（`marginBottom: 12`），不靠父容器的 `gap`。** 这样重排后间距不会错位；靠父容器 gap 时，把官方账号卡片之类的异类插进列表会丢间距。
- **改通用行为（间距、按钮、拖拽、选中态）改在变体组件里。** 只在某个 CLI 的卡片里改，样式就会重新分叉——这正是本模块要消除的问题（见根 `AGENTS.md` Hard Rule 14）。

## 跨模块依赖

- `features/coding/shared/ModelListSection`：模型折叠区的**唯一实现**。Codex 式和 OpenCode 式都用它，不要另写一份。它不支持 official models，需要展示只读目录的 CLI 走 `footer` 插槽。
- `features/coding/shared/management`（`ManagementCheckbox`）：批量选择的复选框。
- `features/coding/shared/providerConnectivity/ProviderConnectivityStatus`：连通性状态点。
- `components/common/ProviderNameLink`、`components/common/SdkTag`：名称链接与 SDK 标签。
- 使用方：`zcode`（OpenCode 式）。其余 CLI 尚未迁移，各自仍持有 bespoke 卡片。

## 迁移一个 CLI 的步骤

1. 按上表判定该 CLI 属于哪种样式（看它有没有模型目录、有没有单一 active provider）。
2. 把原 `*ProviderCard.tsx` 改成薄映射层：解析自己的存储形状 → 构造 `ProviderCardVariantProps` → 渲染对应样式组件。
3. 原卡片里**该 CLI 特有的业务逻辑**（如 ZCode 的 `isDefault` 双写、网关接管按钮）留在页面或映射层，不要塞进样式组件。
4. 检查被删掉的 props 是否在页面侧变成死代码（`onApply`、`onTest` 之类），一并清理。
5. 核对原卡片是否有样式组件没有的能力（官方模型只读列表、网关标签、优先级徽章）——有则用 `footer` / `nameTags` / `inlineActions` 插槽补，**不要**为此给样式组件加 CLI 专属 prop。

## 最小验证

- 该 CLI 页面能正常列出卡片：名称、第二行、操作按钮、模型折叠区（若有）与迁移前一致。
- 拖拽：能拖动排序；在搜索/非自定义排序下拖拽被禁用；批量选择模式下拖拽句柄换成复选框。
- 选中态：批量选择时卡片边框变主色。
- 若该 CLI 有「设为默认」：点击后卡片标记与磁盘上的运行时指针**同时**更新（只查一处会漏掉脱节）。
- 禁用开关（若有）：能翻转状态，卡片随之变灰。

## 何时更新本文件

- 新增第四种样式时：先确认它确实是**语义差异**而非现有样式的参数变体；是的话在根 `AGENTS.md` 的 Index 与 SOP §4.2.1 同步登记。
- 某条经验上升为跨模块通用规则时，同步补到根 `AGENTS.md`。
