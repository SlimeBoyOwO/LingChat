---
title: 视觉感知功能使用说明
description: 学习如何配置和使用 LingChat 的视觉感知功能，让AI能够观察和描述你的桌面内容。
---

# 🌈视觉感知功能使用说明

::: tip
设定完毕后，可以通过在与AI对话的对话中，包含 `“看桌面”` 或者 `“看看我的桌面”` 来触发视觉感知，允许AI观察你的屏幕并做出回应。
:::

首先，从通义千问或者其他拥有视觉感知的大模型网站中，获取API -> [阿里云的相关视觉模型API获取网站](https://bailian.console.aliyun.com/?tab=api#/api)

然后，在设置或者根目录的 `.env` 文件中修改以下参数：

- `VD_API_KEY`（图像识别模型的 API Key）
- `VD_BASE_URL`（视觉模型的 API 访问地址）
- `VD_MODEL`（视觉模型的模型类型）

例如下面的示例：

假设你要使用 [qwen2.5-vl-7b-instruct](https://bailian.console.aliyun.com/?tab=model&accounttraceid=bef5c4d0bc384ad294f43f844ed11cd9thwc#/model-market/detail/qwen2.5-vl-7b-instruct) 模型：

1. `VD_API_KEY` 参数填写你自己的阿里云 API Key

2. 查看 `VD_BASE_URL` 需要点击 [页面](https://bailian.console.aliyun.com/?tab=model&accounttraceid=bef5c4d0bc384ad294f43f844ed11cd9thwc#/model-market/detail/qwen2.5-vl-7b-instruct) 右上角的 `查看API参考`，之后你会在页面右侧看到以下代码，其中的 `base_url` 变量值就是 `VD_BASE_URL` 的值：

```python
import os
from openai import OpenAI

client = OpenAI(
    # 若没有配置环境变量，请用百炼API Key将下行替换为：api_key="sk-xxx",
    api_key=os.getenv("DASHSCOPE_API_KEY"),
    base_url="https://dashscope.aliyuncs.com/compatible-mode/v1", # VD_BASE_URL的值
)

completion = client.chat.completions.create(
    model="qwen-vl-plus",  # 此处以qwen-vl-plus为例，可按需更换模型名称。模型列表：https://help.aliyun.com/zh/model-studio/getting-started/models
    messages=[{"role": "user","content": [
            {"type": "image_url",
             "image_url": {"url": "https://dashscope.oss-cn-beijing.aliyuncs.com/images/dog_and_girl.jpeg"}},
            {"type": "text", "text": "这是什么"},
               ]}]
    )
print(completion.model_dump_json())
```

3. `VD_MODEL` 参数是模型的名称，点击[页面](https://bailian.console.aliyun.com/?tab=model&accounttraceid=bef5c4d0bc384ad294f43f844ed11cd9thwc#/model-market/detail/qwen2.5-vl-7b-instruct)上方模型名称右侧的复制图标即可获取模型名称。

> [!NOTE]
> 阿里云 API 默认赠送额度，不需要充值， *而且对于这个项目肯定够用一辈子了* 。
