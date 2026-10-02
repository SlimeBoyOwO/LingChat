# Codex 推理摘要接收

GPT-6 Astra 等 Codex 模型通过 Responses API 返回可显示的推理摘要。摘要的长度和是否生成由服务端决定；`reasoning_tokens` 表示内部推理用量，不代表返回了同样长度的可读文本。`reasoning.encrypted_content` 是加密的会话状态，不能作为摘要显示。

选择「默认」推理深度时，请求 `reasoning.summary: "auto"`，让服务端决定 effort；显式指定 high 等档位时同时发送 effort 和 summary。旧配置中的 off 保持省略 reasoning 的行为。

接收链路支持摘要／推理文本 delta、text.done、summary_part.done、output_item.done，以及 response.completed／response.incomplete 中的 reasoning 输出项。按输出项、文本类型和分段索引累积，完整快照只补充未收到的后缀，避免一份摘要被重复输出。不同段落用空行分开，正文与工具参数不进入推理缓冲。

所有可见摘要统一转换成 `LlmChunk::Reasoning`，继续交给现有链路：

- 生成过程中更新 `ai:thinking_progress` 的字数。
- 最终台词携带 thinking，供「对话历史 → 思考过程」展开查看。
- 流结束时写入 `[Codex Thinking]` 日志。

如果服务端上报了推理 token，但没有返回任何可见摘要，日志会明确记录该情况，便于区分「模型进行了推理」和「应用收到了可显示的摘要」。完成事件才到达的摘要只能在相应事件收到后显示，不能提前展示。

协议依据：[OpenAI Responses 流式事件](https://developers.openai.com/api/reference/resources/responses/streaming-events)与 [GPT-6 Astra 模型说明](https://developers.openai.com/api/docs/models/gpt-6-astra)。
