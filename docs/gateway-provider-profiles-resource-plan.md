# Gateway Provider Profiles Resource Implementation Plan

## 目标

把 Claude Code 和 Codex 自定义供应商的“供应商类型、每个 endpoint URL 对应的 API 格式、默认 Base URL、后续 Gateway body 兼容 profile”收敛到一个资源文件里，按 `preset_models.json` 的模式在启动时加载 bundled defaults、读取 app data 缓存、后台拉远端最新数据。

UI 交互改为：

1. 先选供应商。
2. API 格式放在供应商同一行的右侧。
3. 内置供应商的具体 endpoint 决定 API 格式，API 格式只读展示并随 endpoint 自动更新。
4. 选择内置供应商 endpoint 时自动填充 Base URL。
5. 选择“自定义”时保留现有能力，用户手动选择 API 格式和 Base URL。

这一步的核心产物是可信的 `provider.meta.providerType` 和 `provider.meta.apiFormat`。后续 DeepSeek/Moonshot/Zai/Doubao/Grok/Longcat/ModelScope/Bailian/MiMo 等 provider body 兼容规则只以这个显式 profile 为主，不再依赖容易误判的关键字猜测。

本文件是 `docs/outbound-adapter-migration-plan.md` 的前置数据与 UI 实施方案：先让 provider 记录准确保存 providerType + endpoint apiFormat，再执行真正的 outbound compat 迁移。

## 不做的事

- 不把 provider 专属 body rewrite 做成 JSON DSL。JSON 只存配置事实和 profile id；实际兼容逻辑仍写 Rust 代码并用单元测试覆盖。
- 不把 per-provider 兼容逻辑下沉到 `transformer/`。`transformer/` 保持 provider 无关，只做协议结构转换。
- 不让官方订阅 provider 保存 `providerType`。官方 provider 不参与 Gateway 第三方 profile 兼容。
- 不在打开旧 provider 表单时静默改写用户数据。只有用户保存时才持久化新字段。

## Source of Truth

新增资源文件：

- `tauri/resources/gateway_provider_profiles.json`

新增运行时缓存：

- app data 下的 `gateway_provider_profiles.json`

新增远端 URL：

- `https://raw.githubusercontent.com/coulsontl/ai-toolbox/main/tauri/resources/gateway_provider_profiles.json`

运行时优先级：

1. app data 缓存有效时使用缓存。
2. 缓存缺失或无效时回退 bundled defaults。
3. 前端启动后后台请求远端 JSON；成功后写缓存并刷新前端内存态。
4. 远端失败时不影响启动，不清空现有内存数据。

这与 `preset_models.json` 的链路保持一致。

## 参考源取舍原则

这次不要只照搬一个参考项目。配置事实和兼容行为分开取源：

| 事项 | 优先参考 | 原因 |
| --- | --- | --- |
| Claude/Codex 供应商名称、URL、默认模型、Codex `apiFormat`、`modelCatalog`、`codexChatReasoning` | `D:\GitHub\cc-switch` | cc-switch 已经覆盖当前产品要展示的 Claude/Codex provider preset，且 Codex preset 明确标注哪些官方 endpoint 原生走 `openai_responses`。 |
| OpenAI Chat target 的 provider body 兼容规则 | `D:\GitHub\axonhub\llm\transformer\<provider>\outbound.go` 和对应测试 | AxonHub 把 DeepSeek/Moonshot/Zai/Doubao/xAI/Longcat/ModelScope/Bailian 的 request body 限制写成了 provider transformer，规则更贴近真实上游 schema。 |
| provider SSE/stream 过滤 | AxonHub provider stream filter | xAI 空 delta 过滤、Bailian tool-call 流式过滤都在 AxonHub 有专门测试。 |
| DeepSeek/Kimi/MiMo Anthropic 直通 thinking 历史归一化 | `D:\GitHub\cc-switch\src-tauri\src\proxy\providers\claude.rs` | AxonHub 当前主要是 OpenAI-compatible outbound，没有这条 Claude Anthropic 直通历史修正；cc-switch 已覆盖 missing thinking、redacted thinking、signature 删除和 DeepSeek disabled effort 冲突。 |

落地规则：

- 第一阶段资源 JSON 的 URL、格式、模型默认值直接以 cc-switch 为准。
- 第二阶段 provider body compat 逐 provider 对照 cc-switch 与 AxonHub；如果 AxonHub 有明确代码和测试，OpenAI Chat body 规则优先采用 AxonHub。
- 如果 cc-switch 与 AxonHub 都有同类规则，优先采用更窄、更有 provider/模型条件限制的一方。
- 任何 provider 专属限制都只能通过 `provider.meta.providerType + target protocol` 命中；旧数据 fallback 只能作为兼容路径，不能成为新数据主路径。
- AxonHub 的 provider transformer 是“已知 provider”的强类型分发，不是 URL 关键字猜测；AI Toolbox 也应通过资源文件保存 providerType 来达到同样效果。

## 资源 JSON Schema

文件建议结构：

```json
{
  "schemaVersion": 1,
  "updatedAt": "2026-07-02",
  "profiles": [
    {
      "id": "deepseek",
      "providerType": "deepseek",
      "label": "DeepSeek",
      "category": "cn_official",
      "aliases": ["deepseek"],
      "tools": {
        "claude": {
          "defaultEndpointId": "anthropic",
          "endpoints": [
            {
              "id": "anthropic",
              "label": "Anthropic",
              "apiFormat": "anthropic",
              "baseUrl": "https://api.deepseek.com/anthropic",
              "models": {
                "primary": "deepseek-v4-pro",
                "haiku": "deepseek-v4-flash",
                "sonnet": "deepseek-v4-pro",
                "opus": "deepseek-v4-pro"
              }
            },
            {
              "id": "openai_chat",
              "label": "OpenAI Chat",
              "apiFormat": "openai_chat",
              "baseUrl": "https://api.deepseek.com"
            }
          ]
        },
        "codex": {
          "defaultEndpointId": "openai_chat",
          "endpoints": [
            {
              "id": "openai_chat",
              "label": "OpenAI Chat",
              "apiFormat": "openai_chat",
              "baseUrl": "https://api.deepseek.com",
              "configProviderId": "deepseek",
              "model": "deepseek-v4-flash",
              "modelCatalog": {
                "models": [
                  {
                    "model": "deepseek-v4-flash",
                    "displayName": "DeepSeek V4 Flash",
                    "contextWindow": 1000000
                  },
                  {
                    "model": "deepseek-v4-pro",
                    "displayName": "DeepSeek V4 Pro",
                    "contextWindow": 1000000
                  }
                ]
              },
              "codexChatReasoning": {
                "supportsThinking": true,
                "supportsEffort": true,
                "thinkingParam": "thinking",
                "effortParam": "reasoning_effort",
                "effortValueMode": "deepseek",
                "outputFormat": "reasoning_content"
              }
            },
            {
              "id": "anthropic_messages",
              "label": "Anthropic",
              "apiFormat": "anthropic_messages",
              "baseUrl": "https://api.deepseek.com/anthropic"
            }
          ]
        }
      },
      "compat": {
        "openaiChat": ["deepseek_json_schema", "deepseek_thinking"],
        "anthropicMessages": [
          "anthropic_tool_thinking_history",
          "deepseek_disabled_strip_effort"
        ]
      }
    }
  ]
}
```

字段说明：

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `schemaVersion` | 是 | 当前固定为 `1`。后续结构变更时递增。 |
| `profiles[]` | 是 | 供应商选项列表，数组顺序就是 UI 展示顺序。 |
| `profiles[].id` | 是 | UI 选项 id。允许 `kimi_coding`、`mimo_token_plan` 这种具体变体。 |
| `profiles[].providerType` | 是 | 后端兼容规则使用的 canonical profile。多个 UI 变体可以归到同一个 `providerType`。 |
| `profiles[].label` | 是 | UI 默认展示名。字段值来自资源文件，不走 i18n，避免远端新增供应商还要补 locale。 |
| `profiles[].category` | 否 | 仅用于分组展示，可取 `cn_official`、`aggregator`、`third_party`。 |
| `profiles[].aliases` | 否 | 旧数据 fallback 或搜索用，不作为主要识别来源。 |
| `profiles[].tools.claude` | 否 | Claude Code 的默认配置。 |
| `profiles[].tools.codex` | 否 | Codex 的默认配置。 |
| `tools.<tool>.defaultEndpointId` | 是 | 该工具下默认使用的 endpoint。供应商选中后默认 endpoint 决定 API 格式和 Base URL。 |
| `tools.<tool>.endpoints[]` | 是 | 该工具下可用 URL 列表。每个 URL 必须显式写出对应 API 格式。 |
| `endpoints[].id` | 是 | endpoint id，同一 tool 内唯一，例如 `anthropic`、`openai_chat`、`openai_responses`。 |
| `endpoints[].label` | 是 | endpoint 展示名。若一个供应商有多个 URL/格式，UI 可用它区分。 |
| `endpoints[].apiFormat` | 是 | 此 URL 对应的 API 格式。内置供应商选中后此值决定表单 `meta.apiFormat`。 |
| `endpoints[].baseUrl` | 是 | 此 API 格式对应的 Base URL。 |
| `models` / `model` / `modelCatalog` | 否 | 可用于新建 provider 时填充默认模型；编辑已有 provider 时不强行覆盖。 |
| `codexChatReasoning` | 否 | 从 cc-switch Codex preset 拷贝，用于后续 Codex Chat reasoning 参数映射。 |
| `compat` | 否 | 文档化兼容规则 id，帮助 review；实际 rewrite 仍由 Rust 按 `providerType` 分发。 |

API 格式命名统一用当前项目已有值：

- Claude 表单仍可显示 `anthropic`，但 Gateway 归一后等价于 `anthropic_messages`。
- Codex 表单使用 `openai_responses`、`openai_chat`、`anthropic_messages`、`gemini_native`。
- 资源文件中建议直接使用表单当前值，后端通过 `AiProtocol::from_api_format` 统一解析。

## 第一批供应商数据

第一批只放用户明确点名且 cc-switch 已有预设的供应商。默认 endpoint 必须直接从 cc-switch 对应工具 preset 拷贝；非默认 endpoint 可以复用 cc-switch 同供应商在另一个工具里的 URL，用于 Gateway 跨协议转换入口，但必须显式标成 endpoint，不能把它当成该工具的直连默认。每个 URL 都必须在资源里显式写出 `apiFormat`，不能让 UI 或后端靠 URL 猜格式。

| `id` | `providerType` | Tool | endpoint `id` | `apiFormat` | Base URL |
| --- | --- | --- | --- | --- | --- |
| `deepseek` | `deepseek` | Claude | `anthropic` | `anthropic` | `https://api.deepseek.com/anthropic` |
| `deepseek` | `deepseek` | Claude | `openai_chat` | `openai_chat` | `https://api.deepseek.com` |
| `deepseek` | `deepseek` | Codex | `openai_chat` | `openai_chat` | `https://api.deepseek.com` |
| `deepseek` | `deepseek` | Codex | `anthropic_messages` | `anthropic_messages` | `https://api.deepseek.com/anthropic` |
| `zai_cn` | `zai` | Claude | `anthropic` | `anthropic` | `https://open.bigmodel.cn/api/anthropic` |
| `zai_cn` | `zai` | Codex | `openai_chat` | `openai_chat` | `https://open.bigmodel.cn/api/coding/paas/v4` |
| `zai_en` | `zai` | Claude | `anthropic` | `anthropic` | `https://api.z.ai/api/anthropic` |
| `zai_en` | `zai` | Codex | `openai_chat` | `openai_chat` | `https://api.z.ai/api/coding/paas/v4` |
| `doubao` | `doubao` | Claude | `anthropic` | `anthropic` | `https://ark.cn-beijing.volces.com/api/compatible` |
| `doubao` | `doubao` | Codex | `openai_responses` | `openai_responses` | `https://ark.cn-beijing.volces.com/api/v3` |
| `bailian` | `bailian` | Claude | `anthropic` | `anthropic` | `https://dashscope.aliyuncs.com/apps/anthropic` |
| `bailian` | `bailian` | Codex | `openai_responses` | `openai_responses` | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| `bailian_coding` | `bailian` | Claude | `anthropic` | `anthropic` | `https://coding.dashscope.aliyuncs.com/apps/anthropic` |
| `moonshot` | `moonshot` | Claude | `anthropic` | `anthropic` | `https://api.moonshot.cn/anthropic` |
| `moonshot` | `moonshot` | Codex | `openai_chat` | `openai_chat` | `https://api.moonshot.cn/v1` |
| `kimi_coding` | `moonshot` | Claude | `anthropic` | `anthropic` | `https://api.kimi.com/coding/` |
| `kimi_coding` | `moonshot` | Codex | `openai_chat` | `openai_chat` | `https://api.kimi.com/coding/v1` |
| `modelscope` | `modelscope` | Claude | `anthropic` | `anthropic` | `https://api-inference.modelscope.cn` |
| `modelscope` | `modelscope` | Codex | `openai_chat` | `openai_chat` | `https://api-inference.modelscope.cn/v1` |
| `longcat` | `longcat` | Claude | `anthropic` | `anthropic` | `https://api.longcat.chat/anthropic` |
| `longcat` | `longcat` | Codex | `openai_responses` | `openai_responses` | `https://api.longcat.chat/openai/v1` |
| `mimo` | `mimo` | Claude | `anthropic` | `anthropic` | `https://api.xiaomimimo.com/anthropic` |
| `mimo` | `mimo` | Codex | `openai_responses` | `openai_responses` | `https://api.xiaomimimo.com/v1` |
| `mimo_token_plan` | `mimo` | Claude | `anthropic` | `anthropic` | `https://token-plan-cn.xiaomimimo.com/anthropic` |
| `mimo_token_plan` | `mimo` | Codex | `openai_responses` | `openai_responses` | `https://token-plan-cn.xiaomimimo.com/v1` |
| `xai` | `xai` | Codex | `openai_chat` | `openai_chat` | `https://api.x.ai/v1` |

注意：

- Doubao/Bailian/Longcat/MiMo 在 cc-switch Codex 预设里是 `openai_responses`，所以这些供应商的 Codex 默认 endpoint 是 `openai_responses`。如果后续要支持它们的 OpenAI Chat body 兼容规则，必须在资源里新增对应 `openai_chat` endpoint，明确写出该 URL 和格式，不能只靠用户手动把格式改成 Chat。
- `providerType` 不等于 UI `id`。例如 `moonshot` 和 `kimi_coding` 都保存 `providerType: "moonshot"`，这样 Gateway runtime 兼容规则只写一份。
- `xai` UI 可以显示为 `Grok / xAI`。后端兼容 profile 建议 canonical 用 `xai`，同时接受历史别名 `grok`。

## 实施步骤

### 1. 新增资源文件

新增：

- `tauri/resources/gateway_provider_profiles.json`

校验要求：

- 根对象必须是 object。
- `schemaVersion` 必须是正整数。
- `profiles` 必须是非空数组。
- 每个 profile 必须有非空 `id`、`providerType`、`label`。
- 每个 profile 至少有一个 `tools.claude` 或 `tools.codex`。
- 每个 tool profile 必须有非空 `endpoints`。
- 每个 endpoint 必须有 `id`、`apiFormat`、`baseUrl`。
- 每个 tool 内的 endpoint `id` 不允许重复。
- `defaultEndpointId` 必须指向当前 tool 的某个 endpoint。
- `id` 不允许重复。

同时更新：

- `tauri/resources/AGENTS.md`

需要补充：

- `gateway_provider_profiles.json` 是 Gateway provider profile 默认数据的源码来源。
- app data 下同名文件是运行时缓存，不是仓库内 bundled resource。
- 数组顺序是 UI 可见顺序。

### 2. 新增 Rust 资源加载模块

新增文件：

- `tauri/src/coding/proxy_gateway/provider_profiles.rs`

核心代码形态：

```rust
use crate::db::SqliteDbState;
use crate::http_client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

const CACHE_FILE_NAME: &str = "gateway_provider_profiles.json";
const DEFAULT_GATEWAY_PROVIDER_PROFILES_JSON: &str =
    include_str!("../../../resources/gateway_provider_profiles.json");

static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_cache_dir(dir: PathBuf) {
    let _ = CACHE_DIR.set(dir);
}

fn get_cache_file_path() -> Option<PathBuf> {
    CACHE_DIR.get().map(|dir| dir.join(CACHE_FILE_NAME))
}

pub fn get_gateway_provider_profiles_cache_path() -> Option<PathBuf> {
    get_cache_file_path()
}

#[tauri::command]
pub fn load_cached_gateway_provider_profiles() -> Result<Option<Value>, String> {
    if let Some(data) = read_cache_file() {
        if is_valid_gateway_provider_profiles(&data) {
            return Ok(Some(data));
        }
    }
    Ok(get_bundled_gateway_provider_profiles())
}

#[tauri::command]
pub async fn fetch_remote_gateway_provider_profiles(
    state: tauri::State<'_, SqliteDbState>,
    url: String,
) -> Result<Value, String> {
    let client = http_client::client_with_timeout(&state, 30).await?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| format!("Failed to fetch remote provider profiles: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Remote provider profiles request failed: {}",
            response.status()
        ));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse remote provider profiles JSON: {error}"))?;

    if !is_valid_gateway_provider_profiles(&json) {
        return Err("Remote provider profiles JSON is invalid".to_string());
    }

    if let Err(error) = write_cache_file(&json) {
        log::warn!("[GatewayProviderProfiles] Failed to write cache: {error}");
    }

    Ok(json)
}
```

`is_valid_gateway_provider_profiles` 不需要复杂 schema validator，按 KISS 原则做必要字段校验即可：

```rust
fn is_valid_gateway_provider_profiles(data: &Value) -> bool {
    let Some(object) = data.as_object() else {
        return false;
    };
    if object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .is_none_or(|version| version == 0)
    {
        return false;
    }
    let Some(profiles) = object.get("profiles").and_then(Value::as_array) else {
        return false;
    };
    if profiles.is_empty() {
        return false;
    }

    let mut seen_ids = HashSet::new();
    for profile in profiles {
        let Some(profile_object) = profile.as_object() else {
            return false;
        };
        let Some(id) = profile_object.get("id").and_then(Value::as_str).map(str::trim) else {
            return false;
        };
        if id.is_empty() || !seen_ids.insert(id.to_string()) {
            return false;
        }
        if profile_object
            .get("providerType")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_none_or(str::is_empty)
        {
            return false;
        }
        if profile_object
            .get("label")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_none_or(str::is_empty)
        {
            return false;
        }
        if !profile_has_valid_tool(profile_object.get("tools")) {
            return false;
        }
    }

    true
}
```

补上 tool/endpoint 校验 helper：

```rust
fn profile_has_valid_tool(tools: Option<&Value>) -> bool {
    let Some(tools_object) = tools.and_then(Value::as_object) else {
        return false;
    };

    let mut has_supported_tool = false;
    for tool_key in ["claude", "codex"] {
        let Some(tool_value) = tools_object.get(tool_key) else {
            continue;
        };
        let Some(tool_object) = tool_value.as_object() else {
            return false;
        };
        if !tool_has_valid_endpoints(tool_object) {
            return false;
        }
        has_supported_tool = true;
    }

    has_supported_tool
}

fn tool_has_valid_endpoints(tool_object: &serde_json::Map<String, Value>) -> bool {
    let Some(default_endpoint_id) = tool_object
        .get("defaultEndpointId")
        .and_then(Value::as_str)
        .map(str::trim)
    else {
        return false;
    };
    if default_endpoint_id.is_empty() {
        return false;
    }

    let Some(endpoints) = tool_object.get("endpoints").and_then(Value::as_array) else {
        return false;
    };
    if endpoints.is_empty() {
        return false;
    }

    let mut endpoint_ids = HashSet::new();
    for endpoint in endpoints {
        let Some(endpoint_object) = endpoint.as_object() else {
            return false;
        };
        let Some(endpoint_id) = endpoint_object
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
        else {
            return false;
        };
        if endpoint_id.is_empty() || !endpoint_ids.insert(endpoint_id.to_string()) {
            return false;
        }
        for field in ["label", "apiFormat", "baseUrl"] {
            if endpoint_object
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .is_none_or(str::is_empty)
            {
                return false;
            }
        }
    }

    endpoint_ids.contains(default_endpoint_id)
}
```

需要补测试：

- bundled JSON 可解析。
- 空 profiles 被拒绝。
- 重复 id 被拒绝。
- 缺 `providerType` 被拒绝。
- 缺 tool `endpoints` 被拒绝。
- endpoint 缺 `apiFormat` / `baseUrl` 被拒绝。
- `defaultEndpointId` 指向不存在 endpoint 时被拒绝。
- cache 优先于 bundled。

### 3. 注册 Rust 模块和命令

修改：

- `tauri/src/coding/proxy_gateway/mod.rs`

增加：

```rust
pub mod provider_profiles;
```

修改：

- `tauri/src/lib.rs`

在 app data 初始化处增加：

```rust
coding::proxy_gateway::provider_profiles::set_cache_dir(app_data_dir.clone());
info!("Gateway 供应商 Profile 缓存目录已初始化");
```

在 `invoke_handler` 增加：

```rust
coding::proxy_gateway::provider_profiles::fetch_remote_gateway_provider_profiles,
coding::proxy_gateway::provider_profiles::load_cached_gateway_provider_profiles,
```

### 4. 前端新增动态内存态

新增文件：

- `web/features/coding/shared/gateway/providerProfiles.ts`

核心类型：

```ts
import { normalizeGatewayApiFormat, type GatewayApiFormat } from './providerProtocol';

export type GatewayProviderToolKey = 'claude' | 'codex';

export interface GatewayProviderModelDefaults {
  primary?: string;
  haiku?: string;
  sonnet?: string;
  opus?: string;
}

export interface GatewayProviderEndpointProfile {
  id: string;
  label: string;
  apiFormat: GatewayApiFormat | 'anthropic';
  baseUrl: string;
  model?: string;
  models?: GatewayProviderModelDefaults;
  configProviderId?: string;
  modelCatalog?: {
    models?: Array<{
      model: string;
      displayName?: string;
      contextWindow?: number;
    }>;
  };
  codexChatReasoning?: Record<string, unknown>;
}

export interface GatewayProviderToolProfile {
  defaultEndpointId: string;
  endpoints: GatewayProviderEndpointProfile[];
}

export interface GatewayProviderProfile {
  id: string;
  providerType: string;
  label: string;
  category?: string;
  aliases?: string[];
  tools: Partial<Record<GatewayProviderToolKey, GatewayProviderToolProfile>>;
  compat?: Record<string, string[]>;
}

export interface GatewayProviderProfileCatalog {
  schemaVersion: number;
  updatedAt?: string;
  profiles: GatewayProviderProfile[];
}
```

动态 store 模式复用 `PRESET_MODELS`：

```ts
export const GATEWAY_PROVIDER_PROFILES_REMOTE_URL =
  'https://raw.githubusercontent.com/coulsontl/ai-toolbox/main/tauri/resources/gateway_provider_profiles.json';

export const GATEWAY_PROVIDER_PROFILE_CATALOG: GatewayProviderProfileCatalog = {
  schemaVersion: 1,
  profiles: [],
};

let gatewayProviderProfilesVersion = 0;
const gatewayProviderProfileListeners = new Set<() => void>();

export const getGatewayProviderProfilesVersion = () => gatewayProviderProfilesVersion;

export const subscribeGatewayProviderProfiles = (listener: () => void) => {
  gatewayProviderProfileListeners.add(listener);
  return () => {
    gatewayProviderProfileListeners.delete(listener);
  };
};

export const updateGatewayProviderProfiles = (catalog: GatewayProviderProfileCatalog) => {
  if (!catalog || !Array.isArray(catalog.profiles) || catalog.profiles.length === 0) {
    return;
  }
  GATEWAY_PROVIDER_PROFILE_CATALOG.schemaVersion = catalog.schemaVersion;
  GATEWAY_PROVIDER_PROFILE_CATALOG.updatedAt = catalog.updatedAt;
  GATEWAY_PROVIDER_PROFILE_CATALOG.profiles = catalog.profiles;
  gatewayProviderProfilesVersion += 1;
  gatewayProviderProfileListeners.forEach((listener) => listener());
};
```

辅助函数：

```ts
export const CUSTOM_PROVIDER_PROFILE_ID = '__custom__';
export const CUSTOM_PROVIDER_ENDPOINT_KEY = `${CUSTOM_PROVIDER_PROFILE_ID}:`;

export const toGatewayProviderEndpointKey = (
  profileId: string,
  endpointId?: string | null,
) => `${profileId}:${endpointId || ''}`;

export const parseGatewayProviderEndpointKey = (value?: string | null) => {
  if (!value || value === CUSTOM_PROVIDER_ENDPOINT_KEY) {
    return {
      providerProfileId: CUSTOM_PROVIDER_PROFILE_ID,
      providerEndpointId: undefined,
    };
  }
  const separatorIndex = value.indexOf(':');
  if (separatorIndex < 0) {
    return {
      providerProfileId: value,
      providerEndpointId: undefined,
    };
  }
  return {
    providerProfileId: value.slice(0, separatorIndex),
    providerEndpointId: value.slice(separatorIndex + 1) || undefined,
  };
};

export const getGatewayProviderProfilesForTool = (tool: GatewayProviderToolKey) =>
  GATEWAY_PROVIDER_PROFILE_CATALOG.profiles.filter((profile) => profile.tools?.[tool]);

export const findGatewayProviderProfile = (profileId?: string | null) =>
  GATEWAY_PROVIDER_PROFILE_CATALOG.profiles.find((profile) => profile.id === profileId);

export const findGatewayProviderToolProfile = (
  profileId: string | null | undefined,
  tool: GatewayProviderToolKey,
) => findGatewayProviderProfile(profileId)?.tools?.[tool];

export const findGatewayProviderEndpoint = (
  profileId: string | null | undefined,
  tool: GatewayProviderToolKey,
  endpointId?: string | null,
) => {
  const toolProfile = findGatewayProviderToolProfile(profileId, tool);
  if (!toolProfile) {
    return undefined;
  }
  const selectedEndpointId = endpointId || toolProfile.defaultEndpointId;
  return toolProfile.endpoints.find((endpoint) => endpoint.id === selectedEndpointId)
    ?? toolProfile.endpoints.find((endpoint) => endpoint.id === toolProfile.defaultEndpointId)
    ?? toolProfile.endpoints[0];
};

const normalizeEndpointBaseUrl = (baseUrl?: string | null) =>
  baseUrl?.trim().replace(/\/+$/, '').toLowerCase() || '';

export const inferGatewayProviderEndpointSelection = (params: {
  tool: GatewayProviderToolKey;
  providerType?: string | null;
  baseUrl?: string | null;
  apiFormat?: string | null;
}) => {
  const normalizedProviderType = params.providerType?.trim().toLowerCase();
  const normalizedBaseUrl = normalizeEndpointBaseUrl(params.baseUrl);
  const normalizedApiFormat = normalizeGatewayApiFormat(params.apiFormat);

  if (normalizedProviderType) {
    const providerTypeMatches = getGatewayProviderProfilesForTool(params.tool).filter(
      (profile) => profile.providerType.toLowerCase() === normalizedProviderType,
    );
    const exactEndpointMatch = providerTypeMatches.flatMap((profile) => {
      const toolProfile = profile.tools[params.tool];
      return (toolProfile?.endpoints || []).map((endpoint) => ({ profile, endpoint }));
    }).find(({ endpoint }) =>
      normalizeEndpointBaseUrl(endpoint.baseUrl) === normalizedBaseUrl &&
      normalizeGatewayApiFormat(endpoint.apiFormat) === normalizedApiFormat,
    );
    if (exactEndpointMatch) {
      return {
        providerProfileId: exactEndpointMatch.profile.id,
        providerEndpointId: exactEndpointMatch.endpoint.id,
      };
    }
    const firstProfile = providerTypeMatches[0];
    return {
      providerProfileId: firstProfile?.id ?? CUSTOM_PROVIDER_PROFILE_ID,
      providerEndpointId: firstProfile?.tools[params.tool]?.defaultEndpointId,
    };
  }

  if (normalizedBaseUrl) {
    const exactEndpointMatch = getGatewayProviderProfilesForTool(params.tool).flatMap((profile) => {
      const toolProfile = profile.tools[params.tool];
      return (toolProfile?.endpoints || []).map((endpoint) => ({ profile, endpoint }));
    }).find(({ endpoint }) =>
      normalizeEndpointBaseUrl(endpoint.baseUrl) === normalizedBaseUrl &&
      (!normalizedApiFormat || normalizeGatewayApiFormat(endpoint.apiFormat) === normalizedApiFormat),
    );
    if (exactEndpointMatch) {
      return {
        providerProfileId: exactEndpointMatch.profile.id,
        providerEndpointId: exactEndpointMatch.endpoint.id,
      };
    }
  }

  return {
    providerProfileId: CUSTOM_PROVIDER_PROFILE_ID,
    providerEndpointId: undefined,
  };
};
```

### 5. 前端 app 启动加载和远端刷新

修改：

- `web/services/appApi.ts`

新增：

```ts
import {
  GATEWAY_PROVIDER_PROFILES_REMOTE_URL,
  updateGatewayProviderProfiles,
  type GatewayProviderProfileCatalog,
} from '@/features/coding/shared/gateway/providerProfiles';

export const loadCachedGatewayProviderProfiles = async (): Promise<boolean> => {
  try {
    const json = await invoke<GatewayProviderProfileCatalog | null>(
      'load_cached_gateway_provider_profiles',
    );
    if (json && typeof json === 'object') {
      updateGatewayProviderProfiles(json);
      console.log('[GatewayProviderProfiles] Loaded from local cache');
      return true;
    }
  } catch (error) {
    console.warn('[GatewayProviderProfiles] Failed to load local cache:', error);
  }
  return false;
};

export const fetchRemoteGatewayProviderProfiles = async (): Promise<void> => {
  try {
    const json = await invoke<GatewayProviderProfileCatalog>(
      'fetch_remote_gateway_provider_profiles',
      { url: GATEWAY_PROVIDER_PROFILES_REMOTE_URL },
    );
    updateGatewayProviderProfiles(json);
    console.log('[GatewayProviderProfiles] Updated from remote');
  } catch (error) {
    console.warn('[GatewayProviderProfiles] Failed to fetch remote:', error);
  }
};
```

修改：

- `web/app/providers.tsx`

在启动初始化中，放到 preset models 附近：

```ts
await loadCachedPresetModels();
await loadCachedGatewayProviderProfiles();

fetchRemotePresetModels();
fetchRemoteGatewayProviderProfiles();
fetchRemoteModelPricing().catch(() => {});
```

要求：

- cached load 可以 await，因为它是本地快速路径。
- remote fetch 不 await，避免阻塞启动。
- remote 失败只 warn，不弹全局错误。

### 6. Claude 表单改造

修改：

- `web/features/coding/claudecode/components/ClaudeProviderFormModal.tsx`

Form values 新增：

```ts
providerEndpointKey?: string;
providerProfileId?: string;
providerEndpointId?: string;
```

同时修改 `web/types/claudecode.ts` 的 `ClaudeProviderFormValues`，加入这三个可选字段。`providerEndpointKey` 只服务 UI 下拉；保存时只使用 `providerProfileId` / `providerEndpointId` 推导 profile 与 endpoint。

把现有 `mergeApiFormatIntoMeta` 改成：

```ts
const mergeGatewayMetaIntoProviderMeta = (
  meta: ClaudeProvider['meta'] | undefined,
  apiFormat: ClaudeApiFormat | undefined,
  providerType: string | undefined,
) => {
  const nextMeta = { ...(meta || {}) };
  delete nextMeta.apiFormat;
  delete nextMeta.providerType;
  if (apiFormat) {
    nextMeta.apiFormat = apiFormat;
  }
  if (providerType) {
    nextMeta.providerType = providerType;
  }
  return Object.keys(nextMeta).length > 0 ? nextMeta : undefined;
};
```

打开表单时：

```ts
const providerEndpointSelection = inferGatewayProviderEndpointSelection({
  tool: 'claude',
  providerType: provider.meta?.providerType,
  apiFormat: provider.meta?.apiFormat,
  baseUrl: provider.settingsConfig?.env?.ANTHROPIC_BASE_URL,
});

form.setFieldsValue({
  ...providerEndpointSelection,
  providerEndpointKey: toGatewayProviderEndpointKey(
    providerEndpointSelection.providerProfileId,
    providerEndpointSelection.providerEndpointId,
  ),
});
```

新建自定义 provider 默认：

- `providerEndpointKey = CUSTOM_PROVIDER_ENDPOINT_KEY`
- `providerProfileId = CUSTOM_PROVIDER_PROFILE_ID`
- `providerEndpointId = undefined`
- `apiFormat = DEFAULT_CLAUDE_API_FORMAT`

供应商 endpoint 选择变化：

```ts
const handleClaudeProviderEndpointChange = (selectionKey: string) => {
  const { providerProfileId, providerEndpointId } =
    parseGatewayProviderEndpointKey(selectionKey);

  form.setFieldsValue({ providerProfileId, providerEndpointId });

  if (providerProfileId === CUSTOM_PROVIDER_PROFILE_ID) {
    form.setFieldsValue({
      apiFormat: form.getFieldValue('apiFormat') || DEFAULT_CLAUDE_API_FORMAT,
    });
    return;
  }

  const endpointProfile = findGatewayProviderEndpoint(
    providerProfileId,
    'claude',
    providerEndpointId,
  );
  if (!endpointProfile) {
    return;
  }
  form.setFieldsValue({
    providerEndpointKey: toGatewayProviderEndpointKey(providerProfileId, endpointProfile.id),
    providerEndpointId: endpointProfile.id,
    apiFormat: normalizeClaudeApiFormat(endpointProfile.apiFormat) ?? DEFAULT_CLAUDE_API_FORMAT,
    baseUrl: endpointProfile.baseUrl,
    model: endpointProfile.models?.primary ?? form.getFieldValue('model'),
    haikuModel: endpointProfile.models?.haiku ?? form.getFieldValue('haikuModel'),
    sonnetModel: endpointProfile.models?.sonnet ?? form.getFieldValue('sonnetModel'),
    opusModel: endpointProfile.models?.opus ?? form.getFieldValue('opusModel'),
  });
  setCurrentBaseUrl(endpointProfile.baseUrl);
};
```

如果某个供应商在同一工具下有多个 endpoint，供应商下拉可以把选项 flatten 成“供应商 + endpoint”：

```ts
const providerEndpointOptions = [
  {
    value: CUSTOM_PROVIDER_ENDPOINT_KEY,
    label: t('claudecode.provider.providerProfileCustom'),
  },
  ...getGatewayProviderProfilesForTool('claude').flatMap((profile) => {
    const toolProfile = profile.tools.claude;
    if (!toolProfile) {
      return [];
    }
    return toolProfile.endpoints.map((endpoint) => ({
      value: toGatewayProviderEndpointKey(profile.id, endpoint.id),
      label: toolProfile.endpoints.length > 1
        ? `${profile.label} / ${endpoint.label}`
        : profile.label,
    }));
  }),
];
```

这样 UI 仍然是“先选供应商”，但选择值已经精确到 URL endpoint；右侧 API 格式从 endpoint 派生，不需要用户再判断。

保存时：

```ts
const selectedProfile =
  values.providerProfileId && values.providerProfileId !== CUSTOM_PROVIDER_PROFILE_ID
    ? findGatewayProviderProfile(values.providerProfileId)
    : undefined;

const selectedProviderType = selectedCategory === 'official'
  ? undefined
  : selectedProfile?.providerType;

const selectedApiFormat = selectedCategory === 'official'
  ? undefined
  : values.apiFormat;

meta: mergeGatewayMetaIntoProviderMeta(provider?.meta, selectedApiFormat, selectedProviderType)
```

字段顺序改为：

1. category，如果当前表单允许选择。
2. name。
3. 供应商 + API 格式同一行。
4. baseUrl。
5. apiKey。
6. 模型映射。
7. 高级设置、计费、备注。

供应商 + API 格式同一行可以用 `Space.Compact` 或 CSS grid，优先 CSS grid，避免 AntD Form 嵌套值丢失：

```tsx
const selectedProviderProfileId =
  Form.useWatch('providerProfileId', form) || CUSTOM_PROVIDER_PROFILE_ID;

<Form.Item label={t('claudecode.provider.providerProfile')}>
  <div className={styles.providerProfileRow}>
    <Form.Item name="providerProfileId" noStyle>
      <Input type="hidden" />
    </Form.Item>
    <Form.Item name="providerEndpointId" noStyle>
      <Input type="hidden" />
    </Form.Item>
    <Form.Item name="providerEndpointKey" noStyle>
      <Select
        showSearch
        options={providerEndpointOptions}
        onChange={handleClaudeProviderEndpointChange}
      />
    </Form.Item>
    <Form.Item name="apiFormat" noStyle>
      <Select
        options={apiFormatOptions}
        disabled={selectedProviderProfileId !== CUSTOM_PROVIDER_PROFILE_ID}
      />
    </Form.Item>
  </div>
</Form.Item>
```

样式：

```less
.providerProfileRow {
  display: grid;
  grid-template-columns: minmax(0, 1.4fr) minmax(150px, 0.8fr);
  gap: 8px;
  align-items: center;
}

@media (max-width: 720px) {
  .providerProfileRow {
    grid-template-columns: 1fr;
  }
}
```

### 7. Codex 表单改造

修改：

- `web/features/coding/codex/components/CodexProviderFormModal.tsx`
- `web/features/coding/codex/hooks/useCodexConfigState.ts`，只有现有 baseUrl helper 无法精准更新当前 provider section 时才改。

Form values 新增：

```ts
providerEndpointKey?: string;
providerProfileId?: string;
providerEndpointId?: string;
```

同时修改 `web/types/codex.ts` 的 `CodexProviderFormValues`，加入这三个可选字段。`providerEndpointKey` 只服务 UI 下拉；保存时只使用 `providerProfileId` / `providerEndpointId` 推导 profile 与 endpoint。

打开表单时同样用 endpoint selection，必须同时传 `apiFormat` 和 Base URL：

```ts
const providerEndpointSelection = inferGatewayProviderEndpointSelection({
  tool: 'codex',
  providerType: provider.meta?.providerType,
  apiFormat: provider.meta?.apiFormat,
  baseUrl: form.getFieldValue('baseUrl'),
});

form.setFieldsValue({
  ...providerEndpointSelection,
  providerEndpointKey: toGatewayProviderEndpointKey(
    providerEndpointSelection.providerProfileId,
    providerEndpointSelection.providerEndpointId,
  ),
});
```

新建自定义 provider 默认：

- `providerEndpointKey = CUSTOM_PROVIDER_ENDPOINT_KEY`
- `providerProfileId = CUSTOM_PROVIDER_PROFILE_ID`
- `providerEndpointId = undefined`
- `apiFormat = DEFAULT_CODEX_API_FORMAT`

供应商 endpoint 选择变化：

```ts
const handleCodexProviderEndpointChange = (selectionKey: string) => {
  const { providerProfileId, providerEndpointId } =
    parseGatewayProviderEndpointKey(selectionKey);

  form.setFieldsValue({ providerProfileId, providerEndpointId });

  if (providerProfileId === CUSTOM_PROVIDER_PROFILE_ID) {
    form.setFieldsValue({
      apiFormat: form.getFieldValue('apiFormat') || DEFAULT_CODEX_API_FORMAT,
    });
    return;
  }

  const endpointProfile = findGatewayProviderEndpoint(
    providerProfileId,
    'codex',
    providerEndpointId,
  );
  if (!endpointProfile) {
    return;
  }
  const apiFormat = normalizeCodexApiFormat(endpointProfile.apiFormat) ?? DEFAULT_CODEX_API_FORMAT;

  form.setFieldsValue({
    providerEndpointKey: toGatewayProviderEndpointKey(providerProfileId, endpointProfile.id),
    providerEndpointId: endpointProfile.id,
    apiFormat,
    baseUrl: endpointProfile.baseUrl,
    model: endpointProfile.model ?? form.getFieldValue('model'),
  });

  handleBaseUrlChange(endpointProfile.baseUrl);

  if (endpointProfile.modelCatalog && !form.getFieldValue(['modelCatalog', 'models'])?.length) {
    form.setFieldValue('modelCatalog', endpointProfile.modelCatalog);
  }
};
```

Codex 下拉选项同 Claude 一样 flatten 到 endpoint：

```ts
const providerEndpointOptions = [
  {
    value: CUSTOM_PROVIDER_ENDPOINT_KEY,
    label: t('codex.provider.providerProfileCustom'),
  },
  ...getGatewayProviderProfilesForTool('codex').flatMap((profile) => {
    const toolProfile = profile.tools.codex;
    if (!toolProfile) {
      return [];
    }
    return toolProfile.endpoints.map((endpoint) => ({
      value: toGatewayProviderEndpointKey(profile.id, endpoint.id),
      label: toolProfile.endpoints.length > 1
        ? `${profile.label} / ${endpoint.label}`
        : profile.label,
    }));
  }),
];
```

UI 片段：

```tsx
const selectedProviderProfileId =
  Form.useWatch('providerProfileId', form) || CUSTOM_PROVIDER_PROFILE_ID;

<Form.Item label={t('codex.provider.providerProfile')}>
  <div className={styles.providerProfileRow}>
    <Form.Item name="providerProfileId" noStyle>
      <Input type="hidden" />
    </Form.Item>
    <Form.Item name="providerEndpointId" noStyle>
      <Input type="hidden" />
    </Form.Item>
    <Form.Item name="providerEndpointKey" noStyle>
      <Select
        showSearch
        options={providerEndpointOptions}
        onChange={handleCodexProviderEndpointChange}
      />
    </Form.Item>
    <Form.Item name="apiFormat" noStyle>
      <Select
        options={apiFormatOptions}
        disabled={selectedProviderProfileId !== CUSTOM_PROVIDER_PROFILE_ID}
      />
    </Form.Item>
  </div>
</Form.Item>
```

重要约束：

- Codex 不能只更新普通 `baseUrl` 字段，必须同步更新 `configToml`。
- 如果当前 `handleBaseUrlChange` 或 `setCodexBaseUrl` 是简单字符串替换，需要先改成只更新 active provider section，避免误改其他 `[model_providers.*]`。
- 不要因为选择供应商覆盖用户已有 API key。
- 编辑已有 provider 时，不自动覆盖已有 model/modelCatalog；只有新建或字段为空时填默认值。

保存 meta 同 Claude：

```ts
meta: mergeGatewayMetaIntoProviderMeta(provider?.meta, selectedApiFormat, selectedProviderType)
```

字段顺序改为：

1. category，如果当前表单允许选择。
2. name。
3. API Key。
4. 供应商 + API 格式同一行。
5. baseUrl。
6. model / model catalog。
7. configToml。
8. 高级设置、计费、备注。

### 8. i18n 文案

新增或更新文案必须使用脚本，不要手动编辑完整 locale JSON。

建议 key：

- `claudecode.provider.providerProfile`
- `claudecode.provider.providerProfileCustom`
- `claudecode.provider.providerProfileHelp`
- `codex.provider.providerProfile`
- `codex.provider.providerProfileCustom`
- `codex.provider.providerProfileHelp`

命令示例：

```bash
pnpm i18n:set-key claudecode.provider.providerProfile --zh-CN "供应商" --en-US "Provider" --write
pnpm i18n:set-key claudecode.provider.providerProfileCustom --zh-CN "自定义" --en-US "Custom" --write
pnpm i18n:set-key claudecode.provider.providerProfileHelp --zh-CN "内置供应商会自动设置 API 格式和 Base URL；自定义供应商可手动配置。" --en-US "Built-in providers set API format and Base URL automatically. Custom providers can be configured manually." --write
pnpm i18n:set-key codex.provider.providerProfile --zh-CN "供应商" --en-US "Provider" --write
pnpm i18n:set-key codex.provider.providerProfileCustom --zh-CN "自定义" --en-US "Custom" --write
pnpm i18n:set-key codex.provider.providerProfileHelp --zh-CN "内置供应商会自动设置 API 格式和 Base URL；自定义供应商可手动配置。" --en-US "Built-in providers set API format and Base URL automatically. Custom providers can be configured manually." --write
```

最后跑：

```bash
node scripts/i18n-keys.mjs check
```

### 9. 备份恢复缓存文件

因为运行时会写 app data 缓存 `gateway_provider_profiles.json`，需要像 `preset_models.json` 一样纳入备份恢复。

修改：

- `tauri/src/settings/backup/utils.rs`
- `tauri/src/settings/backup/local.rs`
- `tauri/src/settings/backup/webdav.rs`
- `tauri/src/settings/backup/AGENTS.md`

新增 helper：

```rust
pub fn get_gateway_provider_profiles_cache_file() -> Option<PathBuf> {
    crate::coding::proxy_gateway::provider_profiles::get_gateway_provider_profiles_cache_path()
        .filter(|path| path.exists())
}
```

备份 zip 根目录新增：

- `gateway_provider_profiles.json`

恢复时遇到这个文件，写回：

```rust
crate::coding::proxy_gateway::provider_profiles::get_gateway_provider_profiles_cache_path()
```

### 10. Gateway runtime 使用 providerType

当前 `ProviderGatewayMeta` 已有：

- `provider_type`
- `api_format`

`runtime/providers.rs` 已能读取 `meta.providerType`。需要调整的是后续兼容层的识别优先级：

1. `provider.meta.provider_type` 精确命中。
2. 旧数据 fallback：base URL 精确或安全 contains。
3. model hint fallback，只用于低风险兼容。
4. 未命中就是 Generic。

新增文件建议：

- `tauri/src/coding/proxy_gateway/runtime/provider_profiles.rs`
- `tauri/src/coding/proxy_gateway/runtime/outbound_compat.rs`

`provider_profiles.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatProfile {
    Generic,
    DeepSeek,
    Moonshot,
    Zai,
    Doubao,
    Xai,
    Longcat,
    ModelScope,
    Bailian,
    Mimo,
}

pub fn detect_compat_profile(provider: &UpstreamProvider, requested_model: &str) -> CompatProfile {
    if let Some(provider_type) = provider.meta.provider_type.as_deref() {
        if let Some(profile) = profile_from_provider_type(provider_type) {
            return profile;
        }
    }

    detect_compat_profile_from_legacy_hints(provider, requested_model)
}
```

`profile_from_provider_type` 必须支持别名：

```rust
fn profile_from_provider_type(provider_type: &str) -> Option<CompatProfile> {
    match provider_type.trim().to_ascii_lowercase().as_str() {
        "deepseek" => Some(CompatProfile::DeepSeek),
        "moonshot" | "kimi" => Some(CompatProfile::Moonshot),
        "zai" | "zhipu" | "glm" => Some(CompatProfile::Zai),
        "doubao" | "doubaoseed" => Some(CompatProfile::Doubao),
        "xai" | "grok" => Some(CompatProfile::Xai),
        "longcat" => Some(CompatProfile::Longcat),
        "modelscope" => Some(CompatProfile::ModelScope),
        "bailian" | "dashscope" => Some(CompatProfile::Bailian),
        "mimo" | "xiaomimimo" => Some(CompatProfile::Mimo),
        "custom" | "generic" => Some(CompatProfile::Generic),
        _ => None,
    }
}
```

`outbound_compat.rs` 入口：

```rust
pub fn apply_outbound_compat(
    body: Vec<u8>,
    provider: &UpstreamProvider,
    target_protocol: AiProtocol,
    conversion_route: Option<ConversionRoute>,
    requested_model: &str,
    route_streaming: bool,
) -> Result<Vec<u8>, GatewayForwardError> {
    let mut value = parse_json_body(body)?;
    filter_private_outbound_fields(&mut value, false);

    apply_target_protocol_compat(&mut value, target_protocol, conversion_route);

    let profile = detect_compat_profile(provider, requested_model);
    match (profile, target_protocol) {
        (CompatProfile::DeepSeek, AiProtocol::OpenAiChat) => {
            apply_deepseek_chat_compat(&mut value);
        }
        (CompatProfile::Moonshot, AiProtocol::OpenAiChat) => {
            rewrite_json_schema_to_json_object(&mut value);
        }
        (CompatProfile::Zai, AiProtocol::OpenAiChat) => {
            apply_zai_chat_compat(&mut value);
        }
        (CompatProfile::Doubao, AiProtocol::OpenAiChat) => {
            apply_doubao_chat_compat(&mut value);
        }
        (CompatProfile::Xai, AiProtocol::OpenAiChat) => {
            apply_xai_chat_compat(&mut value, requested_model);
        }
        (CompatProfile::Longcat, AiProtocol::OpenAiChat) => {
            apply_longcat_chat_compat(&mut value);
        }
        (CompatProfile::ModelScope, AiProtocol::OpenAiChat) => {
            remove_metadata(&mut value);
        }
        (CompatProfile::Bailian, AiProtocol::OpenAiChat) => {
            apply_bailian_chat_compat(&mut value);
        }
        (
            CompatProfile::DeepSeek | CompatProfile::Moonshot | CompatProfile::Mimo,
            AiProtocol::AnthropicMessages,
        ) if conversion_route.is_none() => {
            apply_anthropic_tool_thinking_history_compat(&mut value);
            if profile == CompatProfile::DeepSeek {
                apply_deepseek_anthropic_disabled_strip_effort(&mut value, provider);
            }
        }
        _ => {}
    }

    serialize_json_body(value)
}
```

注意：

- Anthropic 直通 thinking 归一化是直通路径例外，必须限定 `conversion_route.is_none()`。
- OpenAI Responses target 暂时不套 Chat-only body 规则。
- 现有 `apply_outbound_adapter_compat` 的规则先迁入或被新入口包住，行为不能变。
- 现有 `filter_private_outbound_fields` 要移动到新模块或保持可见，不要复制第二份逻辑。

如果实现 AxonHub 里的 stream filter，另建 response stream 入口，不要塞进 request body 函数：

```rust
pub fn outbound_stream_compat_profile(
    provider: &UpstreamProvider,
    target_protocol: AiProtocol,
    requested_model: &str,
) -> Option<OutboundStreamCompatProfile> {
    match (detect_compat_profile(provider, requested_model), target_protocol) {
        (CompatProfile::Xai, AiProtocol::OpenAiChat) => {
            Some(OutboundStreamCompatProfile::XaiFilterEmptyDelta)
        }
        (CompatProfile::Bailian, AiProtocol::OpenAiChat) => {
            Some(OutboundStreamCompatProfile::BailianToolCallFilter)
        }
        _ => None,
    }
}
```

这个入口只决定是否套 stream wrapper；具体 wrapper 应放在 `runtime/outbound_stream_compat.rs` 或 `runtime/outbound_compat.rs` 的独立子模块里，并补 SSE fixture/单元测试。

### 11. 双参考源兼容逻辑迁移清单

按 provider 单独迁移，不要一次性大改。

| 顺序 | Profile | Target | 规则 | AxonHub 参考 | cc-switch 参考 | 采用策略 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 通用 | 所有 JSON target | 保留现有 `_` 私有字段过滤、无 tools 清 `tool_choice` / `parallel_tool_calls`、转换到 OpenAI Chat 时清 Responses/Codex 专属字段 | 无 | 当前 AI Toolbox `runtime/upstream.rs` | 原样迁入 `outbound_compat.rs`，先锁行为回归。 |
| 2 | DeepSeek | OpenAI Chat | `response_format.type=json_schema` 改 `json_object`，删除 `json_schema`；`reasoning_effort=none` -> `thinking.disabled`，其他或空值 -> `thinking.enabled`；thinking enabled 时每条 assistant 历史缺 `reasoning_content` 就补 `""` | `llm/transformer/deepseek/outbound.go`、`outbound_test.go` | `src/config/codexProviderPresets.ts` 的 `codexChatReasoning` | body 规则按 AxonHub；Codex 默认模型/endpoint 按 cc-switch。 |
| 3 | Moonshot/Kimi | OpenAI Chat | `response_format.type=json_schema` 改 `json_object`，删除 `json_schema` | `llm/transformer/moonshot/outbound.go`、`outbound_test.go` | Kimi / Kimi Coding Codex preset | body 规则按 AxonHub；providerType 统一保存 `moonshot`，UI `moonshot`/`kimi_coding` 可分 profile id。 |
| 4 | Zai/GLM | OpenAI Chat | 默认 base version 是 `v4`；`json_schema -> json_object`；从 metadata 提取 `user_id/request_id`；缺 request_id 时优先用 session id；`tool_choice` 强制改 `auto`；删除 metadata；`reasoning_effort` 非空时写 `thinking.enabled/disabled` | `llm/transformer/zai/outbound.go`、`outbound_test.go`、`thinking_test.go` | Zai CN/EN preset | body 规则按 AxonHub；AI Toolbox 里没有 AxonHub 的 `shared.GetSessionID(ctx)` 时，用 Gateway trace/session id，最后才用稳定 fallback。 |
| 5 | Doubao | OpenAI Chat | 默认 base version 是 `v3`；从 metadata 提取 `user_id/request_id`；缺 request_id 时生成 `req_<unix timestamp>`；删除 metadata | `llm/transformer/doubao/outbound.go`、`outbound_test.go` | Doubao preset | body 规则按 AxonHub；image/video/embedding 不进入本轮 Gateway CLI chat compat。 |
| 6 | xAI/Grok | OpenAI Chat | `grok-4` 清 `reasoning_effort`、presence/frequency penalty、stop；`grok-3` / `grok-3-mini` 清 presence/frequency penalty、stop | `llm/transformer/xai/outbound.go`、`outbound_test.go` | xAI preset | body 规则按 AxonHub；model match 用小写精确模型名或安全前缀，避免误伤非 Grok 模型。 |
| 7 | xAI/Grok | OpenAI Chat stream | 过滤无意义空 delta：保留 done、有 choices/delta 且 delta 有 content/multiple content/tool_calls/role/finish_reason/refusal/reasoning_content 任一信息 | `llm/transformer/xai/outbound.go`、`outbound_test.go` | 无 | 这是 response stream compat，不放进 request body 函数；后续单独在 stream wrapper 层实现。 |
| 8 | Longcat | OpenAI Chat | 所有 message 如果 `content` 和 multi content 都空，补 `""`；最终 OpenAI Chat message content 强制序列化为 array 格式 | `llm/transformer/longcat/outbound.go`、`outbound_test.go` | Longcat preset | body 规则按 AxonHub；只在 OpenAI Chat target 生效，Responses endpoint 不套。 |
| 9 | ModelScope | OpenAI Chat | 删除顶层 metadata | `llm/transformer/modelscope/outbound.go` | ModelScope preset | body 规则按 AxonHub，保持最小化。 |
| 10 | Bailian | OpenAI Chat | 合并连续、纯 assistant tool-call messages，避免 DashScope 拒绝拆开的 assistant tool call 历史 | `llm/transformer/bailian/outbound.go`、`outbound_test.go` | Bailian preset | body 规则按 AxonHub；只合并空 content、无 name/refusal/messageIndex/reasoning/cache_control 等副作用字段的 assistant tool-call message。 |
| 11 | Bailian | OpenAI Chat stream | tool-call 模式下缓存 tool call 后出现的文本，到 finish 前再输出；重复 `{}` tool args 在已有有效 args 后改为空字符串 | `llm/transformer/bailian/stream_filter.go`、`stream_filter_test.go` | 无 | 这是 response stream compat；阶段 C 做，不和 request body 迁移混在一个 PR。 |
| 12 | DeepSeek/Kimi/MiMo | Anthropic Messages 直通 | assistant 消息含 `tool_use` 但没有 thinking 时插入 placeholder thinking；`redacted_thinking` 改成普通 `thinking` 占位；保留 thinking 文本但删除 `signature` | 无 | `src-tauri/src/proxy/providers/claude.rs::normalize_anthropic_tool_thinking_history_for_provider` | AxonHub 没有这条；按 cc-switch 逻辑迁移，但命中条件改为 `providerType in deepseek/moonshot/mimo`，旧数据才用 URL/model fallback。 |
| 13 | DeepSeek | Anthropic Messages 直通 | 官方 `https://api.deepseek.com/anthropic` endpoint 且 `thinking.type=disabled` 时，删除 `output_config.effort` 和顶层 `reasoning_effort` | 无 | `src-tauri/src/proxy/providers/claude.rs::normalize_deepseek_thinking_disabled_strip_effort` | 按 cc-switch 迁移；必须限定 DeepSeek 官方 Anthropic endpoint，不能只看 providerType。 |
| 14 | 多 endpoint / 多 API format 候选 | Runtime 编排 | 同 provider 可以有多个 URL/API format；实际请求要按候选 API format 选择对应 outbound/target 逻辑，不能把 provider 名当唯一格式 | `internal/server/orchestrator/outbound_test.go::TestPersistentOutboundTransformer_*CandidateAPIFormatOutbound` | cc-switch Codex preset 的 per-provider `apiFormat` | AI Toolbox 第一阶段通过 endpoint 选择保存 `meta.apiFormat`，后续 runtime 只看 `meta.apiFormat` 决定 target protocol。 |

每个规则必须有：

- 命中测试。
- 非目标协议不变测试。
- Generic provider 不变测试。
- `providerType` 精确命中优先测试。
- 至少一个关键字 fallback 反例测试。

### 12. 文档与 AGENTS 更新

实现时同步更新：

- `web/features/coding/claudecode/AGENTS.md`
- `web/features/coding/codex/AGENTS.md`
- `web/features/coding/shared/AGENTS.md`
- `tauri/src/coding/proxy_gateway/AGENTS.md`
- `tauri/src/coding/proxy_gateway/transformer/AGENTS.md`
- `tauri/resources/AGENTS.md`
- `tauri/src/settings/backup/AGENTS.md`

必须写入的长期规则：

- 自定义 provider 的 `meta.providerType` 是 Gateway provider compat profile。
- 自定义 provider 的 `meta.apiFormat` 是上游真实协议。
- 内置供应商由 `gateway_provider_profiles.json` 提供，启动时可被远端缓存刷新。
- 官方 provider 不保存 `providerType`。
- provider body compat 只能在 Gateway runtime 层，不能进 transformer。
- app data 下的 `gateway_provider_profiles.json` 是缓存文件，需要备份恢复；仓库内 `tauri/resources/gateway_provider_profiles.json` 是 bundled defaults。

### 13. 验证命令

资源 JSON 校验：

```bash
node -e "JSON.parse(require('fs').readFileSync('tauri/resources/gateway_provider_profiles.json','utf8')); console.log('ok')"
```

i18n：

```bash
node scripts/i18n-keys.mjs check
```

前端类型检查：

```bash
pnpm exec tsc --noEmit
```

Rust 针对性测试：

```bash
cd tauri && cargo test gateway_provider_profiles --lib
cd tauri && cargo test provider_profiles --lib
cd tauri && cargo test outbound_adapter --lib
```

如果本轮实现了 body compat：

```bash
cd tauri && cargo test proxy_gateway --lib
```

如果改了备份恢复：

```bash
cd tauri && cargo test backup --lib
```

如果实现范围覆盖 Claude/Codex 表单、资源加载、Gateway runtime：

```bash
pnpm test
cd tauri && cargo test
pnpm exec tsc --noEmit
```

## 一次执行清单

按以下顺序执行，避免中间状态互相阻塞：

1. 新增 `tauri/resources/gateway_provider_profiles.json`，先只放第一批供应商。
2. 新增 `tauri/src/coding/proxy_gateway/provider_profiles.rs`，实现 bundled/cache/remote 三路径和校验测试。
3. 在 `tauri/src/coding/proxy_gateway/mod.rs`、`tauri/src/lib.rs` 注册模块、cache dir 和 Tauri commands。
4. 新增 `web/features/coding/shared/gateway/providerProfiles.ts`，实现前端内存 catalog、订阅、查找、推断 helper。
5. 修改 `web/services/appApi.ts` 和 `web/app/providers.tsx`，接入启动加载和后台远端刷新。
6. 修改 Claude provider 表单：新增供应商选择，供应商和 API 格式同一行，保存 `providerType` + `apiFormat`。
7. 修改 Codex provider 表单：新增供应商选择，同步更新 `baseUrl` 和 `configToml`，保存 `providerType` + `apiFormat`。
8. 使用 i18n 脚本补字段文案并跑 `i18n check`。
9. 把 `gateway_provider_profiles.json` app data 缓存纳入本地/WebDAV 备份恢复。
10. 更新相关 `AGENTS.md`。
11. 跑资源 JSON 校验、`pnpm exec tsc --noEmit`、provider profiles Rust 测试。
12. 第一阶段确认无误后，再开始 `runtime/outbound_compat.rs`，按 provider 一个一个迁移 body 兼容规则和测试。

## 验收标准

阶段 A 完成时应满足：

- 新安装、无网络时，Claude/Codex 表单能显示 bundled provider profiles。
- 有缓存时，启动优先用缓存。
- 远端更新成功时，前端供应商列表能刷新。
- Claude 新建 DeepSeek provider 后，保存数据里有 `meta.providerType = "deepseek"` 和 `meta.apiFormat = "anthropic"`，Base URL 自动为 `https://api.deepseek.com/anthropic`。
- Codex 新建 DeepSeek provider 后，保存数据里有 `meta.providerType = "deepseek"` 和 `meta.apiFormat = "openai_chat"`，Base URL 自动为 `https://api.deepseek.com`，`config.toml` 对应 base_url 同步更新。
- 选择“自定义”时，API 格式可编辑，Base URL 不被自动覆盖，保存时不写 `providerType`。
- 官方 provider 不显示供应商选择，不写 `providerType`。
- 编辑旧 provider 时，能按 `meta.providerType` 或 Base URL 推断供应商展示；打开表单本身不改数据。
- Gateway runtime 能从 `UpstreamProvider.meta.provider_type` 读到准确 profile。

阶段 B/C 完成时应满足：

- body compat 只根据 `providerType + target protocol` 生效。
- Generic provider 不受 DeepSeek/Moonshot/Zai/Doubao/Grok/Longcat/ModelScope/Bailian/MiMo 规则影响。
- OpenAI Responses target 不误套 OpenAI Chat-only 规则。
- Anthropic 直通 thinking 归一化只影响 DeepSeek/Moonshot/MiMo 直通路径。
- 迁移文档中的每条已实现规则都有对应回归测试。
