//! LingChat 安全加固模块。
//!
//! 对标 NORP Agent 的安全体系，为智能体/剧本代理等可执行任意操作的模块提供纵深防御：
//! - [`command_guard`]：危险命令前置拦截（递归删除根目录、格式化磁盘、关机重启、远程代码执行等）
//! - [`injection_guard`]：提示词注入 / 越狱检测（正则模式库 + Unicode 混淆 + Base64 载荷）
//! - [`audit`]：安全审计日志（命令执行、文件操作、审批决策、设置变更、注入告警）
//! - [`secrets`]：敏感凭据加密存储（系统 keyring / DPAPI，替代 settings.json 明文）

pub mod audit;
pub mod command_guard;
pub mod injection_guard;
pub mod secrets;
