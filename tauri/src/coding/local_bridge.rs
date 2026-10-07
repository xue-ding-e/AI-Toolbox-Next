//! 所有 CLI 共用的「本地文件桥接态」标识。
//!
//! 每个 coding CLI 都有同一个概念：数据库里没有已应用预设时，把磁盘上那份
//! 现成配置当成一条**虚拟记录**展示，用户可以直接「收编」成预设。这条记录的
//! id 与显示名必须全仓一致——否则同一个概念在不同 CLI 上长得不一样，用户要
//! 重新学一遍（OmO 曾显示 `Local AGENTS.md`，而多数 CLI 是 `default`）。
//!
//! **用法**：构造桥接态记录时用这两个常量，不要再各写各的字面量。
//!
//! ```rust
//! use ai_toolbox_lib::coding::local_bridge;
//!
//! let record = serde_json::json!({
//!     "id": local_bridge::LOCAL_CONFIG_ID,
//!     "name": local_bridge::LOCAL_CONFIG_NAME,
//! });
//! ```

/// 本地文件桥接态的保留 id。
///
/// 前端用它判断「这条不是可管理的预设」：不可 apply、不可删除、不参与排序、
/// 托盘里不出现。**改动它等于同时改前端的判定**，所以前端也有一份同值常量
/// （`web/features/coding/shared/localConfig.ts`）。
pub const LOCAL_CONFIG_ID: &str = "__local__";

/// 本地文件桥接态的显示名。
///
/// 刻意用 `default` 而不是文件名：卡片副标题已经写了「来自本地 <文件>」，
/// 名字里再带一次文件名是重复；而多数 CLI 早就在用 `default`。
pub const LOCAL_CONFIG_NAME: &str = "default";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_config_id_keeps_the_reserved_spelling() {
        // 前端硬编码了同一个值（`__local__`），改动会静默失效——这条断言是提醒。
        assert_eq!(LOCAL_CONFIG_ID, "__local__");
    }

    #[test]
    fn local_config_name_is_the_shared_default() {
        assert_eq!(LOCAL_CONFIG_NAME, "default");
    }
}
