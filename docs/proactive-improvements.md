# 主动回复与视觉理解增强

## 当前状态

- [x] 步骤 1：richer 屏幕上下文 prompt
- [x] 步骤 2：一步式屏幕搭话决策
- [x] 步骤 3：增加反重复机制
- [x] `cargo test --lib` 通过（18 个测试）
- [x] `pnpm build` 通过
- [x] `pnpm tauri build --debug --no-bundle` 通过

## 改动摘要

### screen_analyzer.rs
- 新增 `ScreenContext`：携带 `ai_name`、`user_name`、`recent_chat_summary`
- `analyze_screen` 增加 `context` 参数
- 新增 `analyze_screen_for_proactive`：让 VLM 直接判断说不说
- `VISION_SYSTEM_PROMPT` 和 `build_screen_prompt` 融入角色上下文

### strategy_dispatcher.rs
- SCREEN 模式改为调用 `analyze_screen_for_proactive`
- 模型返回 `[PASS]` 时跳过本次，降级到 TOPIC
- 新增 `ProactiveDeduplicator`，对 ImportantDay/Todo/Screen/Topic 都进行反重复检测

### proactive_system
- 将屏幕分析移出全局锁，避免慢请求阻塞主动回复循环
- 为日程提醒、用户状态与前端投递状态增加统一门控
- 仅在消息成功投递后消耗兴趣值，并修复兴趣度衰减下限
- 提供手动测试主动消息入口与主动思考状态提示

### 视觉模型兼容
- Kimi for Coding 使用 Anthropic Messages API，并解析 thinking/text 多 block 响应
- 可选择跟随聊天模型或使用独立轻量视觉模型
- 支持视觉优先模式、主动截图压缩和耗时日志

### proactive_history.rs（新增）
- 维护最近 10 条、1 小时内的主动搭话历史
- 归一化 + 编辑距离相似度，阈值 0.85
- 自动过期清理和数量限制

### api/chat.rs
- `test_screen_analyzer` 调用更新为 `analyze_screen(&prompt, None)`

### mod.rs
- 注册新模块 `proactive_history`

## 后续可继续优化

- 在 `ScreenContext` 中传入真实对话历史摘要
- 把反重复历史持久化到文件（目前只存内存，重启丢失）
- 增加 AI 正在说话 / 用户正在输入时的触发门控
- 多屏截图和隐藏自身窗口再截图
- 把 TODO/TOPIC/SCREEN 等做成可配置权重的 dispatcher
