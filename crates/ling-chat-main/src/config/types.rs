//! 前端"高级设置"页面使用的结构化配置树类型。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 单个设置项。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSetting {
    pub key: String,
    pub value: String,
    pub description: String,
    #[serde(rename = "type")]
    pub setting_type: String,
}

/// 设置子分类（一组相关设置项）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subcategory {
    pub description: String,
    pub settings: Vec<ConfigSetting>,
}

/// 设置分类（包含多个子分类）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub subcategories: BTreeMap<String, Subcategory>,
}

/// 完整的配置树，前端"高级设置"页面的数据源。
pub type ConfigTree = BTreeMap<String, Category>;
