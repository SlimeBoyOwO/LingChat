//! 内置工具定义。
//!
//! 每个文件一组同类工具（时钟、日程、记忆、场景、角色、文件等），本模块只负责
//! 声明子模块并汇总出聊天侧使用的注册表；工具系统的运行部件（注册表、执行器、
//! 检索、工具闭环）都在上一级模块。

pub mod background_effect;
pub mod character;
pub mod clock;
pub mod god_agent;
pub mod memory;
pub mod read_media_file;
pub mod scene;
pub mod schedule;
pub mod skill_files;
pub mod status;
pub mod tool_search;
pub mod weather;
pub mod web_search;

use std::sync::Arc;

use anyhow::Result;

use crate::ai_service::tools::permissions::CONFIG_FILE_NAME;
use crate::ai_service::tools::permissions::ToolPermissionConfig;
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::tools::settings::SharedToolSettings;

use background_effect::SetBackgroundEffect;
use character::{CharacterGetClothes, CharacterList, CharacterSetClothes, CharacterSwitch};
use clock::CurrentTimeTool;
use memory::{AddNote, DeleteNote, GetCurrentMemory, GetNotes, UpdateNote};
use read_media_file::ReadMediaFileTool;
use scene::{SceneList, SceneSwitch};
use schedule::{AddTodo, DeleteTodo, GetAllSchedule, UpdateTodo};
#[cfg(desktop)]
use skill_files::ExecuteCommand;
use skill_files::{
    DeleteFile, EditFile, Glob, Grep, GrepFiles, ListFiles, ListSkills, ReadFile, ReadSkill,
    SearchFiles, WriteFile,
};
use status::{CurrentStatus, SceneStatus};
use weather::WeatherTool;
use web_search::WebSearchTool;

/// 创建并注册所有内置聊天工具。
pub fn built_in_registry(
    role_names: impl IntoIterator<Item = (String, String)>,
    tool_settings: SharedToolSettings,
) -> Result<ToolRegistry> {
    let data_dir = crate::api::data_dir();
    let registry = ToolRegistry::new();
    registry.register(Arc::new(CurrentTimeTool))?;
    registry.register(Arc::new(WebSearchTool::new(tool_settings.clone())))?;
    // Tool RAG 的检索入口：常驻下发（且不归任何设置分组，语义就是「总是可用」），
    // 让它在注册顺序里紧随网页搜索，两者都是常驻工具。
    registry.register(Arc::new(tool_search::ToolSearch))?;
    registry.register(Arc::new(GetAllSchedule))?;
    registry.register(Arc::new(AddTodo))?;
    registry.register(Arc::new(UpdateTodo))?;
    registry.register(Arc::new(DeleteTodo))?;
    registry.register(Arc::new(GetCurrentMemory))?;
    registry.register(Arc::new(GetNotes::new(&data_dir)))?;
    registry.register(Arc::new(AddNote))?;
    registry.register(Arc::new(UpdateNote))?;
    registry.register(Arc::new(DeleteNote))?;
    registry.register(Arc::new(CurrentStatus))?;
    registry.register(Arc::new(SceneStatus))?;
    registry.register(Arc::new(SceneList))?;
    registry.register(Arc::new(SceneSwitch))?;
    registry.register(Arc::new(SetBackgroundEffect))?;
    registry.register(Arc::new(WeatherTool))?;
    registry.register(Arc::new(CharacterList))?;
    registry.register(Arc::new(CharacterSwitch))?;
    registry.register(Arc::new(CharacterGetClothes))?;
    registry.register(Arc::new(CharacterSetClothes))?;
    registry.register(Arc::new(ListSkills))?;
    registry.register(Arc::new(ReadSkill))?;
    registry.register(Arc::new(ReadMediaFileTool::new(tool_settings.clone())))?;
    registry.register(Arc::new(ListFiles::new(tool_settings.clone())))?;
    registry.register(Arc::new(ReadFile::new(tool_settings.clone())))?;
    registry.register(Arc::new(WriteFile::new(tool_settings.clone())))?;
    registry.register(Arc::new(DeleteFile::new(tool_settings.clone())))?;
    registry.register(Arc::new(EditFile::new(tool_settings.clone())))?;
    registry.register(Arc::new(SearchFiles::new(tool_settings.clone())))?;
    registry.register(Arc::new(GrepFiles::new(tool_settings.clone())))?;
    registry.register(Arc::new(Glob::new(tool_settings.clone())))?;
    registry.register(Arc::new(Grep::new(tool_settings.clone())))?;
    // Android/iOS 没有稳定、可审批的桌面 shell 环境。移动端不注册命令工具，
    // 避免模型选中 execute_command 后才得到系统级执行失败。
    #[cfg(desktop)]
    registry.register(Arc::new(ExecuteCommand::new(tool_settings.clone())))?;
    let mut permissions = ToolPermissionConfig::load_or_create(
        &data_dir,
        registry
            .definitions()
            .into_iter()
            .map(|definition| definition.function.name),
    )?;
    permissions.set_available_tools(
        registry
            .definitions()
            .into_iter()
            .map(|definition| definition.function.name)
            .collect(),
    );
    permissions.initialize_characters(&data_dir, role_names.into_iter().map(|(_, name)| name))?;
    // 启动时按用户配置同步各工具权限：已启用的工具组/web_search 放开给
    // default 角色组（新建配置的 default 组默认关闭）。
    tool_settings.get().sync_to_permissions(&mut permissions);
    // 覆盖写 available_tools 展示列表（仅展示，运行时不被读取）
    permissions.save(&data_dir.join(CONFIG_FILE_NAME))?;
    registry.set_permissions(permissions);
    Ok(registry)
}
