---
title: 使用反代和网络代理
description: 当你需要走中转商、自建反代或机场访问 LLM API 时，参考这里的配置方法。
---

# 🌐 使用反代和网络代理

## 概念区分

- **API 反代/中转**：把 `CHAT_BASE_URL` 等改成中转商或自建反代地址。这个功能已经支持，直接在设置里改对应的 `*_BASE_URL` 字段即可。
- **网络代理**：当你无法直连 API 服务器时（比如用了机场），通过 `*_PROXY_URL` 让 LingChat 走代理。

两者可以同时使用。

## 配置方式

在「设置 → 高级设置 → API 反代与网络代理」中填写：

- `GLOBAL_PROXY_URL`：全局代理，所有模型默认走这个
- `LLM_PROXY_URL`：主对话模型单独代理（留空则用全局）
- `TRANSLATE_PROXY_URL`：翻译模型单独代理（留空则用全局）
- `VD_PROXY_URL`：视觉模型单独代理（留空则用全局）
- `OLLAMA_PROXY_URL`：Ollama 单独代理（留空则用全局）
- `LMSTUDIO_PROXY_URL`：LM Studio 单独代理（留空则用全局）
- `GEMINI_PROXY_URL`：Gemini 单独代理（留空则用全局）

支持的格式：

- `http://127.0.0.1:7890`
- `http://user:pass@host:port`
- `socks5://127.0.0.1:1080`

::: tip 优先级
单独的 `*_PROXY_URL` 优先于 `GLOBAL_PROXY_URL`。所有字段留空即视为不开代理，等同直连。
:::

## 常见场景

### 场景 1：用机场（Clash / V2Ray 等）

填 `GLOBAL_PROXY_URL=http://127.0.0.1:7890`（端口看你机场客户端的设置），所有模型都会走代理。

### 场景 2：用国内中转商

不需要填 `*_PROXY_URL`，直接把 `CHAT_BASE_URL` 改成中转商给的地址，`CHAT_API_KEY` 改成中转商给的 key。

### 场景 3：自建反代（Cloudflare Worker / Vercel / Nginx）

把 `CHAT_BASE_URL` 改成你的反代地址，API key 保持原版不变。如果反代部署在境外需要走代理访问，再填 `LLM_PROXY_URL`。

### 场景 4：内网网关（LiteLLM / One-API）

`CHAT_BASE_URL=http://192.168.x.x:4000/v1`，不需要代理。

## 排查

- 配置后不生效：确认点了"保存"，部分配置修改后即时生效无需重启
- 持续超时：代理本身不通，先用浏览器确认代理能正常访问目标网站
- 格式错误：确保带协议头（`http://` 或 `socks5://`）
- 使用 `socks5://` 时若报错，说明 httpx 缺少 socks 依赖，可执行 `pip install "httpx[socks]"`
