# Codex 订阅识图

Codex 订阅使用自己的 Responses 协议和登录凭据，不需要填写普通 OpenAI 兼容接口的 base_url 或 API Key。

- 视觉模型选择“跟随对话模型”且对话供应商为 Codex 时，发图、聊天截图和窥屏图片可直接随本轮对话发送。
- 单独指定 Codex 为视觉模型时，截图旁白、场景标注和 ReadMediaFile 图片识别复用同一订阅客户端，沿用所选模型、推理、代理和登录刷新配置。
- 视觉角色热切换后，下一次转述请求重新读取当前供应商配置。
- 图片以 input_image 与 data URL 发送。独立识图请求传递视觉配置的输出上限，并统计接口返回的 token 用量；原生对话保持聊天客户端的输出策略。
- 压缩与尺寸检查仍由已有图片处理流程负责；原生图片仅用于当前请求，不写入长期记忆。
- 本通道只接入图片识别，不把视频文件发送为 video_url。视频识别应使用支持相应输入协议的供应商。

协议参考：[OpenAI Images and vision](https://developers.openai.com/api/docs/guides/images-vision)。模型的具体图片输入能力以所选模型和账号接口回执为准。
