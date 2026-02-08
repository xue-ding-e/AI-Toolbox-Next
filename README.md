# AI Toolbox

<p align="center">
  <img src="tauri/icons/128x128@2x.png" alt="AI Toolbox Logo" width="128" height="128">
</p>

<p align="center">
  <strong>个人 AI 工具箱</strong> - 一站式管理 AI 编程助手配置
</p>

<p align="center">
  <a href="https://github.com/coulsontl/ai-toolbox/releases">
    <img src="https://img.shields.io/github/v/release/coulsontl/ai-toolbox?style=flat-square" alt="Release">
  </a>
  <a href="https://github.com/coulsontl/ai-toolbox/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/coulsontl/ai-toolbox?style=flat-square" alt="License">
  </a>
  <a href="https://github.com/coulsontl/ai-toolbox/releases">
    <img src="https://img.shields.io/github/downloads/coulsontl/ai-toolbox/total?style=flat-square" alt="Downloads">
  </a>
</p>

---

## 简介

AI Toolbox 是一个跨平台桌面应用，旨在帮助开发者高效管理各类 AI 编程助手的配置。支持 **Windows**、**macOS** 和 **Linux**。

### 主要功能

- **OpenCode 配置管理** - 可视化管理 OpenCode 的供应商和模型配置，支持列表页快速启停
- **Oh-My-OpenCode / Slim 插件配置管理** - 可视化管理 Oh-My-OpenCode 及 Oh-My-OpenCode Slim 插件的配置
- **Claude Code 配置管理** - 一键切换 Claude Code 的 API 供应商配置，支持动态获取模型列表
- **Codex 配置管理** - 管理 OpenAI Codex CLI 的供应商配置
- **MCP 服务器管理** - 集中管理 MCP（Model Context Protocol）服务器配置，支持导入/导出
- **Skills 技能管理** - 管理 Claude Code 技能（Skills），支持从 Git 仓库安装和自定义工具
- **WSL 同步** - 将 Windows 端的各种CLI配置、MCP 和 Skills 配置同步到 WSL 环境
- **供应商管理** - 统一管理多个 AI 供应商（OpenAI、Anthropic、自定义代理等）
- **系统托盘** - 通过系统托盘快速切换各模块的配置，无需打开主窗口
- **数据备份** - 支持本地备份和 WebDAV 云端备份
- **主题切换** - 支持亮色/暗色/跟随系统主题
- **多语言** - 支持中文和英文界面
- **自动更新检查** - 启动时自动检查新版本

## 截图

<p align="center">
  <img src="docs/screenshots/app_screenshot_opencode_model.jpg" alt="OpenCode 配置管理" width="80%">
  <img src="docs/screenshots/app_screenshot_opencode_provider.jpg" alt="OpenCode 供应商管理" width="80%">
  <img src="docs/screenshots/app_screenshot_opencode_tray.jpg" alt="系统托盘快速切换配置" width="30%">
  <br>
  <em>OpenCode 和 Oh-My-OpenCode 插件配置管理</em>
</p>

<p align="center">
  <img src="docs/screenshots/app_screenshot_claudecode.jpg" alt="Claude Code 配置管理" width="80%">
  <img src="docs/screenshots/app_screenshot_codex.jpg" alt="Codex 配置管理" width="80%">
  <br>
  <em>Claude Code / Codex 配置管理</em>
</p>

<p align="center">
  <img src="docs/screenshots/app_screenshot_mcp.jpg" alt="MCP 服务器管理" width="80%">
  <img src="docs/screenshots/app_screenshot_skills.jpg" alt="Skills 技能管理" width="80%">
  <br>
  <em>MCP 服务器管理 / Skills 技能管理</em>
</p>

<p align="center">
  <img src="docs/screenshots/app_screenshot_settings.jpg" alt="设置页面" width="80%">
  <img src="docs/screenshots/app_screenshot_wsl.jpg" alt="WSL 同步" width="80%">
  <br>
  <em>设置页面 / WSL 同步</em>
</p>

## 下载安装

前往 [Releases](https://github.com/coulsontl/ai-toolbox/releases) 页面下载适合您系统的安装包：

| 系统 | 安装包 |
|------|--------|
| Windows | `.msi` / `.exe` |
| macOS | `.dmg` |
| Linux | `.deb` / `.AppImage` |

## 技术栈

| 类别 | 技术 |
|------|------|
| **桌面框架** | Tauri 2.x |
| **前端** | React 19 + TypeScript 5 |
| **UI 组件库** | Ant Design 6 |
| **状态管理** | Zustand |
| **国际化** | i18next (中文/英文) |
| **数据库** | SurrealDB 2.x (嵌入式 SurrealKV) |
| **构建工具** | Vite 7 |
| **包管理器** | pnpm |

## 项目结构

```
ai-toolbox/
├── web/                          # 前端源码
│   ├── app/                      # 应用层（App、路由、Provider）
│   ├── components/               # 通用组件
│   │   └── layout/               # 布局组件（MainLayout）
│   ├── features/                 # 功能模块（按业务划分）
│   │   ├── daily/                # 【日常】模块
│   │   │   └── notes/            # 笔记功能（Markdown）
│   │   ├── coding/               # 【编码】模块
│   │   │   ├── opencode/         # OpenCode 配置管理
│   │   │   ├── claudecode/       # Claude Code 配置管理
│   │   │   ├── codex/            # Codex 配置管理
│   │   │   ├── mcp/              # MCP 服务器管理
│   │   │   ├── skills/           # Skills 技能管理
│   │   │   └── claude/           # Claude CLI 配置管理
│   │   └── settings/             # 【设置】模块
│   ├── stores/                   # 全局状态（Zustand）
│   ├── services/                 # API 服务层
│   ├── i18n/                     # 国际化配置
│   ├── constants/                # 常量（模块配置）
│   ├── hooks/                    # 全局 Hooks
│   ├── types/                    # 全局类型定义
│   └── utils/                    # 工具函数
├── tauri/                        # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── main.rs               # 入口
│   │   ├── lib.rs                # 库入口、命令注册
│   │   └── coding/               # 编码模块
│   │       ├── claude_code/      # Claude Code 后端
│   │       ├── codex/            # Codex 后端
│   │       ├── open_code/        # OpenCode 后端
│   │       ├── oh_my_opencode/   # Oh-My-OpenCode 后端
│   │       ├── oh_my_opencode_slim/ # Oh-My-OpenCode Slim 后端
│   │       ├── mcp/              # MCP 服务器后端
│   │       ├── skills/           # Skills 技能后端
│   │       └── wsl/              # WSL 同步后端
│   ├── Cargo.toml                # Rust 依赖
│   └── tauri.conf.json           # Tauri 配置
├── package.json                  # 前端依赖
├── vite.config.ts                # Vite 配置
└── tsconfig.json                 # TypeScript 配置
```

## 开发指南

### 前置要求

- Node.js 18+
- pnpm 9+
- Rust 1.86+ (SurrealDB 2.x 需要)
- 参考 [Tauri 前置要求](https://tauri.app/start/prerequisites/)

### 安装依赖

```bash
pnpm install
```

### 启动开发服务器

```bash
pnpm tauri dev
```

### 构建生产版本

```bash
pnpm tauri build
```

### 代码检查

```bash
# TypeScript 类型检查
pnpm tsc --noEmit

# Rust 代码检查
cd tauri && cargo check
```

## 功能模块

| 模块 | 子模块 | 状态 | 描述 |
|------|--------|------|------|
| 编码 | OpenCode | ✅ 完成 | OpenCode 供应商/模型配置管理 |
| 编码 | Oh-My-OpenCode | ✅ 完成 | Oh-My-OpenCode 插件配置管理 |
| 编码 | Oh-My-OpenCode Slim | ✅ 完成 | Oh-My-OpenCode Slim 插件配置管理 |
| 编码 | Claude Code | ✅ 完成 | Claude Code API 配置切换 |
| 编码 | Codex | ✅ 完成 | OpenAI Codex CLI 供应商配置管理 |
| 编码 | MCP 服务器 | ✅ 完成 | MCP 服务器配置管理，支持导入/导出 |
| 编码 | Skills 技能 | ✅ 完成 | Claude Code 技能管理，支持 Git 仓库安装 |
| 编码 | WSL 同步 | ✅ 完成 | MCP 和 Skills 配置同步到 WSL 环境 |
| 设置 | 通用设置 | ✅ 完成 | 语言切换、主题切换、版本更新检查 |
| 设置 | 备份设置 | ✅ 完成 | 本地/WebDAV 数据备份恢复 |
| 设置 | S3 设置 | ✅ 完成 | S3 兼容存储配置 |
| 设置 | 供应商设置 | ✅ 完成 | AI 供应商统一管理 |
| 日常 | 笔记 | 🚧 开发中 | Markdown 笔记管理、搜索 |

## 数据存储

使用 SurrealDB 嵌入式模式（SurrealKV 引擎），数据存储在本地应用数据目录。

### 设计原则

- **本地优先**：所有数据存储在本地，保护隐私
- **服务层 API**：前端通过服务层与后端交互，不直接使用 localStorage
- **灵活备份**：支持本地 ZIP 和 WebDAV 云端备份

### 数据表

| 表名 | 描述 |
|------|------|
| `settings` | 应用设置 |
| `opencode_provider` | OpenCode 供应商配置 |
| `opencode_common_config` | OpenCode 通用配置 |
| `claude_provider` | Claude Code 供应商配置 |
| `claude_common_config` | Claude Code 通用配置 |
| `codex_provider` | Codex 供应商配置 |
| `codex_common_config` | Codex 通用配置 |
| `oh_my_opencode_config` | Oh-My-OpenCode 配置 |
| `oh_my_opencode_global_config` | Oh-My-OpenCode 全局配置 |
| `oh_my_opencode_slim_config` | Oh-My-OpenCode Slim 配置 |
| `oh_my_opencode_slim_global_config` | Oh-My-OpenCode Slim 全局配置 |
| `mcp_preferences` | MCP 服务器偏好配置 |
| `skill_preferences` | Skills 技能偏好配置 |
| `skill_settings` | Skills 设置 |
| `wsl_sync_config` | WSL 同步配置 |

## 贡献

欢迎提交 Issue 和 Pull Request！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 提交 Pull Request

## 推荐 IDE 配置

- [VS Code](https://code.visualstudio.com/)
- [Tauri 插件](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## License

[MIT](LICENSE)

## Acknowledgments

- [skills-hub](https://github.com/qufei1993/skills-hub)
- [cc-switch](https://github.com/farion1231/cc-switch)
