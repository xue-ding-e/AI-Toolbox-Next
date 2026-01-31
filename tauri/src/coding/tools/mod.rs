//! Shared Tools Module
//!
//! This module provides unified tool adapter functionality for both Skills and MCP features.
//! It contains built-in tool configurations, custom tool management, and detection logic.

pub mod types;
pub mod builtin;
pub mod detection;
pub mod custom_store;

pub use types::*;
pub use builtin::*;
pub use detection::*;
