//! 角色记忆的构建、压缩和检索。台词与笔记仍是原始数据，索引只用于加快查找。

pub mod builder;
pub mod cache;
pub mod index;
pub mod persistent;
pub mod recall;
pub mod scope;
pub mod search;
