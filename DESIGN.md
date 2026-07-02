---
name: AI Toolbox Design System
version: 0.1
status: agent-readable-draft
colors:
  primary: "#0958d9"
  onPrimary: "#ffffff"
  background: "#ffffff"
  surface: "#fafafa"
  text: "rgba(0, 0, 0, 0.88)"
  textSecondary: "rgba(0, 0, 0, 0.65)"
  textTertiary: "rgba(0, 0, 0, 0.45)"
typography:
  pageTitle:
    fontSize: "20px"
    fontWeight: 600
    usage: "工作台页面标题和紧凑页面头部。"
  sectionTitle:
    fontSize: "14px"
    fontWeight: 600
    usage: "面板、弹窗分区、设置组和表格卡片标题。"
  body:
    fontSize: "13px"
    fontWeight: 400
    usage: "默认高密度应用正文。"
  helper:
    fontSize: "10px"
    fontWeight: 400
    usage: "提示、诊断、元数据和低强调辅助文字。"
spacing:
  base: "8px"
  compactGap: "6px"
  fieldGap: "12px"
  sectionGap: "16px"
  modalPadding: "20px 24px 22px"
rounded:
  control: "6px"
  card: "8px"
  section: "16px"
  modalSection: "16px"
components:
  primaryButton:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.onPrimary}"
    rounded: "{rounded.control}"
    padding: "{spacing.fieldGap}"
  card:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text}"
    rounded: "{rounded.card}"
    padding: "{spacing.sectionGap}"
  modalSection:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.modalSection}"
    padding: "{spacing.modalPadding}"
  helperText:
    backgroundColor: "{colors.background}"
    textColor: "{colors.textTertiary}"
    rounded: "{rounded.control}"
    padding: "{spacing.compactGap}"
  secondaryText:
    backgroundColor: "{colors.background}"
    textColor: "{colors.textSecondary}"
    rounded: "{rounded.control}"
    padding: "{spacing.compactGap}"
---

# AI Toolbox 设计系统

`DESIGN.md` 是给 AI coding agents 和开发者阅读的视觉设计系统文档。它描述 AI Toolbox 的产品调性、设计 token、布局密度、组件形态和常见 Do / Don't。

本文件不是应用运行时资源。除非某个任务明确要求建立运行时设计 token 管线，否则不要把它接入构建、主题切换、备份、同步或用户配置读写流程。

## 一句话调性

AI Toolbox 是面向长期使用的桌面端配置工作台。界面应当安静、克制、高信息密度、可靠、易扫描，服务于反复管理 Coding CLI、模型供应商、Gateway、会话、Skills、MCP、WSL/SSH、备份和图片渠道。

它不是营销站、落地页或展示型产品。不要为业务工作台创建 hero 区、大幅装饰背景、宣传式卡片组、插画主导布局或过度戏剧化的视觉效果。

## Source of Truth

- 运行时主题颜色来自 `web/App.css` 中的 CSS 变量和 `web/app/providers.tsx` 中的 Ant Design token。
- 亮色、暗色和 system theme 都由现有主题系统处理；新 UI 默认复用这些变量。
- frontmatter 只保留可被 `@google/design.md` lint 识别的核心颜色快照和组件 token；业务实现仍必须使用对应 CSS 变量和 Ant Design token，不要把这些快照硬编码进组件。
- `AGENTS.md` 负责工程规则、模块边界、数据语义和验证要求；本文件负责视觉调性、设计 token、组件形态和界面密度。
- 如果本文件与更近作用域的模块级 `AGENTS.md` 冲突，模块级文档的行为语义优先；颜色、密度、圆角、层级和组件视觉默认继续遵循本文件。

## 使用与校验

- 修改前端可见 UI 前，agent 必须先完整阅读本文件，再阅读目标模块的 `AGENTS.md` 和相关代码。
- UI 方案需要明确落到本文件的调性、布局密度、token、组件形态和 Do / Don't，不要只写“沿用现有风格”。
- 修改本文件后运行 `pnpm design:lint`。该命令使用 Google `@google/design.md` CLI 校验 `DESIGN.md` 的格式和可识别设计 token。
- 仅修改业务 UI 代码时，不强制运行 `pnpm design:lint`；但仍必须按本文件做亮色、暗色、长文本、空态、加载态和交互状态自查。

## 视觉原则

- 优先清晰和可维护，而不是新奇视觉。
- 优先紧凑但可读，而不是稀疏留白。
- 优先状态可扫读，而不是状态强打断。
- 优先一致组件语言，而不是每个模块重新发明一套样式。
- 优先真实数据和真实空态，而不是伪造图表、占位指标或装饰性内容。

避免单一色相铺满页面、装饰性渐变球、玻璃拟态背景、过大的圆角、嵌套卡片和与当前产品气质不一致的品牌化营销布局。

## 色彩

所有可见 UI 颜色必须使用现有 CSS 变量或 Ant Design token。不要在业务组件里硬编码只适用于亮色或暗色的颜色。

主色只用于少量关键位置：

- 主按钮和主操作。
- 选中态、焦点态和当前导航。
- 需要明确强调的链接或状态。

状态色必须表达真实语义：

- `success`：已连接、已启用、健康、完成、安全恢复。
- `warning`：降级、部分成功、等待、冷却中、需要关注。
- `error`：失败、阻塞、危险操作、不可用、无效输入。

不要用状态色做大面积装饰背景。状态色背景应低饱和，正文和边框必须保持足够对比度。

## 字体与文字层级

工作台界面使用紧凑文字层级。页面标题通常接近 Ant Design `Typography.Title level={4}` 的大小。

推荐层级：

- 页面标题：`20px / 600`
- 分区标题：`14px / 600`
- 正文：`13px / 400`
- 辅助文字：`10px / 400`

不要在仪表盘、卡片、弹窗、侧栏、表格和设置面板里使用 hero 级字号。长标签优先换行或压缩布局，不要让文字覆盖邻近内容。

## 布局

AI Toolbox 的核心页面应当像工作台，而不是内容网站。

优先使用：

- 表格：请求记录、统计、历史、定价、可比较记录。
- 紧凑列表或卡片：供应商、Skills、MCP、分组管理项。
- 横向表单行：弹窗和设置分区里的常规字段。
- 分栏工作台：会话详情、主从视图、列表加详情。

不要把卡片放进卡片。页面分区应是自然布局或全宽区域；卡片只用于重复项、弹窗分区和真正需要框定的工具表面。

固定格式 UI 必须有稳定尺寸。工具栏、图标按钮、计数器、标签、卡片标题、表格单元格和加载态不应因为 hover、动态内容或文字长度导致布局跳动。

## 弹窗

默认使用 Ant Design Modal 原生 chrome。不要重度覆盖 `.ant-modal-content`、`.ant-modal-header`、`.ant-modal-footer` 或 `.ant-modal-close`。

普通弹窗只做必要 body padding 调整。高弹窗依赖 `web/App.css` 中的全局 viewport-safe modal 规则，不要重新添加一次性的 `top` 偏移或 max-height hack。

弹窗内普通分区使用 section card：

```less
.sectionCard {
  border: 1px solid var(--color-border);
  border-radius: 16px;
  background: var(--color-bg-elevated);
  padding: 18px;
  box-shadow: none;
}
```

可折叠弹窗分区必须看起来像一个完整 section，而不是多个嵌套卡片。折叠内容不能只通过 `opacity`、`max-height` 或 `overflow` 隐藏后仍保留可聚焦控件。

## 表单

弹窗表单默认优先横向布局：左侧 label，右侧输入或值。

紧凑字段行推荐：

```less
.formFieldRow {
  display: grid;
  grid-template-columns: 108px minmax(0, 1fr);
  gap: 12px;
  align-items: center;
}
```

窄视口可以堆叠为单列。只有单字段快速输入、超长 label 或非常窄的容器才优先使用垂直布局。

保存 optional 字段时，UI 不应因为视觉简化把“用户清空字段”和“字段缺失”混为一谈；具体数据语义继续遵守 `AGENTS.md` 和模块文档。

## 卡片

Provider、配置、Skills、MCP 和管理卡片应保持一致结构：

- 头部紧凑。
- 标题在前。
- metadata 紧跟标题，不要被推到最右侧形成大空隙。
- 状态和主操作容易扫读。
- 重复卡片默认使用 `var(--color-border-card)` 和 `var(--shadow-card-sm)`。

选中或已应用状态可以使用 `var(--ant-color-primary)` 强调边框。Gateway 主供应商或 failover 状态可以使用语义状态色，但必须代表真实状态。

不要把普通卡片边框改成 `var(--color-border-card-subtle)`；它对高频管理页面过弱，容易失去层级。

## 表格与日志

请求、统计、定价、历史和诊断列表应保持高密度表格或类表格布局。

列表只展示摘要。大块 request body、response body、headers、完整 JSON、attempt 明细和 trace 详情应放在详情弹窗或详情面板中。

空态必须表达真实情况。没有数据就显示空态，不要造假数据填图表或列表。

## 管理页面

Skills 和 MCP 管理页面需要容纳几百项。设计时必须考虑性能、扫描效率和批量操作。

优先使用：

- 轻量原生控件和共享 management 组件。
- 搜索、分组、筛选、选择模式和批量操作。
- 虚拟滚动或稳定网格尺寸。
- 高密度卡片和清晰 metadata。

避免在高频列表控件中过度使用重型 Ant Design 组合件。复杂弹窗、表单和表格可以继续使用 Ant Design。

## Gateway 页面

Gateway 是运维型工作台，不是装饰性报表。

使用紧凑 Tab、状态 badge、高密度表格和明确的错误/健康文案。failover、usage-log-recorded 等后台事件只应静默刷新数据，不应弹全局 notification 打断用户。

Gateway 辅助说明文字统一使用 `font-size: 10px` 和 `var(--color-text-tertiary)`。

## 会话详情

会话详情是 workbench。使用结构化导航、紧凑消息渲染、清晰工具块、搜索和滚动定位。

普通 user/assistant 文本应保持轻量。tool、thinking、system、summary、image、unknown 等块可以使用紧凑 renderer card。

不要把每条消息都渲染成大型编号日志卡片。不要把工具卡片包在普通文本气泡里。

## 图标与操作

常见操作使用 `lucide-react` 图标：保存、下载、刷新、搜索、关闭、复制、展开、收起、设置、撤销、重做等。

图标按钮需要可理解的 tooltip。只有在命令不清晰或危险操作时才使用文字按钮补充语义。

不要为常见工具操作手写 SVG。

## 可访问性

亮色、暗色和 system theme 下都必须可读。重要状态不能只靠颜色表达。

焦点态必须可见。不要移除 outline，除非提供同等清晰的 token-based focus style。

折叠内容里的交互控件不能在隐藏状态继续被键盘聚焦。

长文本、长模型名、长路径、长供应商名和长错误信息必须在容器内稳定处理，不能覆盖相邻内容。

## Do

- 使用 `var(--color-*)` 和 Ant Design token。
- 使用 CSS Modules + Less。
- 使用紧凑、稳定、可扫读的布局。
- 让 metadata 靠近标题。
- 弹窗表单默认横向布局。
- 同时检查亮色和暗色主题。
- 用真实空态表达无数据。
- UI 文案保持短、准、操作性强。

## Don't

- 不要在 feature 样式里硬编码浅色或暗色颜色。
- 不要为工作台页面做 landing page 或 hero 区。
- 不要使用装饰性渐变球、bokeh、玻璃拟态背景。
- 不要卡片套卡片。
- 不要用过大的圆角和松散留白稀释信息密度。
- 不要为单个页面引入一套新的视觉系统。
- 不要为了局部差异新增全局 Ant Design 配置。
- 不要让文字重叠、关键状态被截断或动态内容导致布局跳动。
