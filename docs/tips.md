# Tips：已知问题与注意事项

零散的坑，不单独开文档的都记在这里。

## 读档不能续跑剧本

**现象**：在一个剧本中途创建存档，之后读这个档，只会回到自由对话模式，剧本进度（当前章节 / 事件指针 / 变量）丢失，不会自动接着跑。

**根因**：存档里其实存了剧本状态，但读档时没有真正恢复。

- 写入侧（`src-tauri/src/api/save.rs` 的 `create_save`，约 178-191 行）：若 `game_status.script_status` 存在，会把 `folder_key` / `vars` / `current_chapter_key` / `current_event_process` 经 `SaveRepo::upsert_running_script` 落库，并把 `save_id` 关联到 `running_script_id`。
- 读取侧（`load_save`，约 262-265 行）：
  ```rust
  // 10. 恢复剧本状态（若有）
  if let Some(rs_id) = save_model.running_script_id {
      let _ = SaveRepo::get_running_script(db, rs_id).await;
  }
  ```
  取出来就 `let _ =` 丢掉了，没有回写 `game_status.script_status`，也没通知剧本引擎重定位到存档点。这是条未完成的路径。

**当前的前端处理**：读档会整体替换游戏状态，所以前端在读档后主动 `gameStore.exitStoryMode()`，把上一局残留的剧本标记（`runningScript` 里的选项、章节名等）清干净，统一按自由对话进入。也就是说目前读档后的行为是「剧本存档 → 降级为自由对话」，而非续跑。

**彻底修复的方向**（未做）：`load_save` 里把取出的 running_script 回写进 `game_status.script_status` 并驱动 `script_engine` 重建运行态；前端 `applyWebInitData` 之后再据后端状态调用 `enterStoryMode()` 恢复 UI 标记（当前 `applyWebInitData` 完全不碰 `runningScript`）。涉及存档格式与引擎恢复逻辑，改动面较大。
