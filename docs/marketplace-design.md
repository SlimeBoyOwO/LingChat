# LingChat 创意工坊市场设计（AI 全自动审核）

> 状态：方案已讨论定稿，未实施。
> 目标：基于 GitHub 构建一个由 AI 全自动审核上架的内容/插件市场，作为 LingChat 创意工坊的升级版。

---

## 1. 背景与现状

- **创意工坊现状**：LingChat 已有"创意工坊"，基于 GitHub Discussions（`src-tauri/src/api/workshop.rs`），内容为角色卡/剧本等 Markdown 帖子。**无审核机制**——任何人发帖即上架。
- **插件现状**：Python 工具插件（`src-tauri/src/plugins/`），本地手动安装，**无市场/分发机制**。运行于内嵌 RustPython 沙箱。
- **本次目标**：把"发帖即上架"升级为"PR 提交 → AI 审核 → 合并上架"，并为插件补齐市场、分发与运行时权限控制。

### 关键事实（来自代码探索）

| 事实 | 依据 |
|---|---|
| 创意工坊内容类型：角色卡、剧本（按 Discussion category 区分） | `WorkshopPage.vue` |
| 插件 = `manifest.toml` + Python 脚本，跑在 RustPython（冻结标准库） | `python_backend.rs` |
| 沙箱拦截是"顶层代码执行后"才置空 `sys.modules`——**不是安全边界** | `python_backend.rs` L152-161 |
| `http_get/http_post` 无任何 URL 校验，可访问任意地址（含内网） | `http_host.rs` |
| `call_tool` 无权限过滤，可调用全部已注册工具（含写操作） | `python_backend.rs` L98-125 |
| TTS 语音包已有 `inspect → install` 管线，可泛化为通用包安装器 | `tts/local/package.rs` |

---

## 2. 已定决策摘要

| 决策点 | 结论 |
|---|---|
| 市场承载内容 | 内容 + 代码双轨（角色卡/剧本 + Python 插件） |
| 仓库模型 | 方案 A：集中式注册表（单一市场仓库） |
| 自动化程度 | AI 全自动 gate，审核通过即合并 |
| 提交形态 | 文件夹（Git 天然形态，PR diff 可读） |
| 分发形态 | 单个 zip（CI 打包 + SHA256） |
| 包结构 | 统一包壳：`manifest.toml` + `payload/` |
| 分发载体 | GitHub Releases，每包每版本独立 Release |
| 注册表入口 | 市场仓库 main 分支 `plugins.json`（raw URL） |
| LLM 审查器 | 硅基流动免费模型（OpenAI 兼容），Provider 抽象可切换 |
| 审核原则 | 机器规则一票否决为主，LLM 负责语义判断，fail-closed |
| 信任链 | 基础层：SHA256 + 分支保护 + bot 最小权限（签名二期） |
| 权限模型 | URL 白名单（审核比对 + 运行时强制）；call_tool 读免声明、写必须声明 |
| 大文件 | 不进 git/LFS，走外部 URL + sha256 声明 + bot 转存 Releases |
| 举报/下架 | **二期**（本期不做） |

---

## 3. 架构总览

```
作者 fork 市场仓库 → 添加 registry/<type>-<id>/ → PR
        │
        ▼
GitHub Actions：审核流水线（见 §5）
  ① PR 变更集扫描（大文件/归档，纯规则）
  ② 机器检查（一票否决规则集，纯规则）
  ③ LLM 审查（硅基流动免费模型，结构化 verdict）
        │
        ▼
门禁决策（§6）：拒绝→关 PR / 要求修改→留言 / 通过→bot 合并
        │
        ▼
发布链（§7）：CI 打包 zip → 独立 Release → 更新 plugins.json
        │
        ▼
客户端（§8）：拉 plugins.json → 列表展示 → 下载 → SHA256 校验
          → 解包安装 → 运行时按 manifest 权限强制（URL/工具白名单）
```

---

## 4. 上架物模型

### 4.1 统一包壳（所有可安装物）

```
my-thing-1.2.0.zip
├── manifest.toml        # 统一声明：type/name/version/权限/资源清单
└── payload/             # 类型专属内容（插件脚本、剧本章节、立绘、模型）
```

- `manifest.type` 决定 payload 语义：`plugin`（Python 插件）/ `character`（角色包）/ `script`（剧本包）/ `voice`（语音包）
- 客户端只认一种外壳，新类型只需加 type，不改安装器
- 借鉴：`tts/local/package.rs` 的 `inspect_package` 泛化为通用格式检查

### 4.2 内容类双轨（Markdown 门面）

- **纯文字分享**（角色卡文字版、攻略、贴纸）：只有 Markdown 帖子，无安装物。审核 = 内容合规 + 格式约定校验。
- **可安装物**（角色包、剧本包、插件、语音包）：Markdown 帖子做展示（标题/描述/预览图/标签），附件是结构化包。审核 = 内容合规 + 包结构校验 + （插件才有的）代码安全。
- **历史帖子兼容层**：现有 Discussions 帖子保留原解析逻辑（`## 标签` 段落），能识别出可安装物就显示安装按钮，识别不出保持纯展示。新上架物走市场通道。

### 4.3 manifest 扩展字段

```toml
# 网络白名单：审核比对 + 运行时强制共用
[[network]]
host = "api.tavily.com"          # 精确域名
paths = ["/search", "/extract"]  # 可选：限制路径
https_only = true                # 可选，默认 true

# call_tool 写工具声明（读工具免声明）
[[permissions.tools]]
name = "memory_add_note"

# 大文件声明（>5MB 资源，不进 git）
[[assets]]
name = "yuexi-sbv2.onnx"
url = "https://github.com/author/xxx/releases/download/v1/model.onnx"
sha256 = "9f2c..."
size = 52428800
```

---

## 5. 审核流水线

### 5.0 PR 变更集扫描（任何内容审核之前，纯规则）

- 单文件 > 5MB → 一票否决，留言引导走大文件通道
- 二进制归档（`.zip/.7z/.tar.gz`）出现在 diff → 一票否决
- 单 PR 文件数 > 100 → 一票否决（防拆包绕过）
- magic bytes 扫描：文本文件内容是 ELF/模型头 → 一票否决（防伪装扩展名）

### 5.1 机器检查一票否决规则集

**通用（所有类型）**

| # | 规则 |
|---|---|
| 1 | manifest schema 校验失败、type 非法、id 与目录不符、版本非 semver |
| 2 | 版本不递增（同 id 已有更高版本） |
| 3 | 高置信密钥泄露（gitleaks 扫出 AWS/GitHub/OpenAI 格式） |
| 4 | 可执行二进制（ELF/APK/Mach-O 文件头）出现在 payload |
| 5 | 隐藏文件：`.env`、`.git*`、`__MACOSX`、`.DS_Store` |
| 6 | 大块编码载荷：单文件 base64/hex 连续块 > 1KB |
| 7 | 包内资源完整性：manifest 声明与包内文件不一致 |

**插件专用（Python）**

| # | 规则 |
|---|---|
| 8 | 顶层 `import os/subprocess/shutil/pathlib/ctypes/sysconfig`（顶层执行不受沙箱拦截） |
| 9 | `importlib` / `__import__` / `eval` / `exec` / `compile` / 动态构造 import |
| 10 | `object.__subclasses__`、`__builtins__` 操纵、`__globals__` 访问 |
| 11 | 未声明 URL：静态提取代码 URL，与 manifest `[[network]]` 白名单比对，超出即拒 |
| 12 | host 完全来自变量/用户输入（动态构造，静态不可验证）→ 任意 URL = SSRF 风险，拒 |
| 13 | 内网/localhost/保留段地址，manifest 未显式声明 → 拒 |
| 14 | `call_tool` 调用的工具名不在「manifest `[[permissions.tools]]` ∪ 读工具集」→ 拒 |

**内容专用**

| # | 规则 |
|---|---|
| 15 | Markdown 图片/附件引用外部不可信域名（不在白名单） |

### 5.2 LLM 审查（语义判断，硅基流动免费模型）

- **Provider 抽象层**：只认 OpenAI 兼容接口，默认硅基流动免费模型，可配置切换强模型
- **任务**：
  - 内容合规（违规/成人/版权/垃圾广告）
  - 恶意意图（代码与描述不符、数据外泄）
  - 权限一致性（代码实际行为 ⊆ manifest 声明）
  - 工具承诺行为 vs 代码实际行为（如 description 写"查天气"实际外发数据）
- **结构化输出**（必须 schema 校验，解析失败重试、再失败 fail-closed）：

```json
{
  "verdict": "approve | changes | reject",
  "risk_level": "low | medium | high",
  "findings": [
    { "severity": "error|warn|info", "file": "...", "line": 12,
      "category": "malicious|privacy|policy|quality", "detail": "..." }
  ]
}
```

- **分级**：免费模型初筛 + 机器规则兜底；机器标记"可疑但无法定性"的 PR 才升级强模型/人工（数量很少，成本可控）

---

## 6. 门禁决策

| 条件 | 动作 |
|---|---|
| 机器一票否决命中 | 自动关 PR + 留言（精确到 file:line） |
| 机器通过 + LLM approve | bot 合并（squash），触发发布链 |
| LLM changes | PR 保持打开，留言 findings，作者 push 后重新审核 |
| LLM reject | 自动关 PR + 留言 |
| LLM 解析失败 / API 挂 | **PR 挂起等人工（fail-closed，绝不放行）** |
| 高危类（原生代码、敏感权限） | 强制人工复核（即使 LLM approve） |

- 合并权：bot 全自动合并，main 分支开启 branch protection（禁直接 push，只接受 bot 合并，审核 status check 必须通过）
- bot Token：Actions secret，最小权限（仅该 repo contents+issues 写）

---

## 7. 发布链与分发

### 7.1 标准分发流（每包每版本独立 Release）

```
PR 合并 → CI 触发：
  ① 重新校验 + 打包 zip（registry/<id>/ → <type>-<name>-<version>.zip）
  ② gh release create {type}-{name}-{version} dist/xxx.zip
  ③ 更新 plugins.json：{ id, name, version, type, download_url, sha256, size, permissions, review_report_url }
```

- 下载 URL 稳定可预测：`https://github.com/{owner}/{repo}/releases/download/{name}-{version}/{name}-{version}.zip`
- 独立 Release 的原因：资产不可覆盖（避免滚动更新的 404 窗口）、版本历史可回滚、独立下载计数（商店排序）
- 纯文字帖无安装物，不进 Releases，只活在 registry

### 7.2 大文件通道（>5MB 资源）

```
① 作者在 manifest 声明 [[assets]]（外部 URL + sha256 + size）
② bot 审核时：下载 → 校验 hash 与声明一致 → 格式检查（magic/结构）
③ 通过 → bot 转存上传到市场仓库的 draft release
④ 合并 → draft 转正式 release → 客户端只从市场仓库下载
```

- hash 锁定保证"审过的 == 用户装的"；作者 URL 在转存后作废
- 模型是数据不是代码：只做格式检查 + hash 锁定 + 来源声明，不做二进制内容审查

---

## 8. 客户端改动清单（LingChat Rust 侧）

| # | 改动 | 位置 |
|---|---|---|
| 1 | manifest 新增 `[[network]]` / `[[permissions.tools]]` / `[[assets]]` 字段解析与校验 | `types.rs` + `manifest.rs` |
| 2 | `http_get/http_post` 请求前校验 host ∈ 白名单，不在 → 拒绝请求 | `http_host.rs` |
| 3 | `call_tool` 运行时过滤：仅放行「声明工具 ∪ 读工具集」 | `python_backend.rs` |
| 4 | 拉取 `plugins.json` + 商店列表展示（复用 workshop 前端模式） | `api/` + `WorkshopPage.vue` |
| 5 | 下载 zip + SHA256 校验（reqwest 已有下载能力） | 新增 |
| 6 | `inspect → install` 泛化为通用包安装器 | 借鉴 `tts/local/package.rs` |
| 7 | 插件目录管理：下载 → 解包 → 安装到 `data/plugins/<id>/` → 启用 | `manager.rs` |

> 客户端集成是最大工作量，且必须先于市场仓库存在（没有客户端消费，上架无人安装）。市场仓库（§5-7）相对独立，可先行开发验证。

---

## 9. 成本结构

- **机器检查**：零 LLM 成本，纯规则（gitleaks/正则/magic 扫描）
- **内容类**：几乎全免费（免费模型初筛，少量 token）
- **代码类**：免费模型语义审查；仅机器标记的高风险 PR 才升级强模型/人工
- **GitHub**：公开仓库 Actions/Releases 免费；无 LFS 配额问题（大文件走 Releases 不走 LFS）

---

## 10. 二期清单（明确不做）

- 举报/下架/申诉自动化（含恶意举报防护：分级触发 + 暂停非删除 + 复核翻案）
- 注册表签名（bot 私钥签 plugins.json，客户端内置公钥验签）
- AI 自动复检已上架插件
- GitHub Pages 网页商店
- 客户端"已装清单"吊销机制
- 版本回滚 UI、patch 快通道

---

## 11. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 免费模型限流/下架/输出不稳 | Provider 抽象 + 重试 + fail-closed（挂起等人工） |
| 对抗性绕过（针对弱审核器写恶意代码） | 机器规则宽而硬 + 运行时强制（URL/工具白名单）双保险 |
| 仓库膨胀 | 大文件独立通道，git 只存源码 |
| AI 误判（误杀正常插件） | PR 可 reopen 重审；申诉通道二期 |
| bot Token 泄露 | 最小权限 + Actions secret |
| 沙箱非安全边界（顶层代码可绕过） | 审核即安全边界：规则 8-10 直接封死绕过路径 |
