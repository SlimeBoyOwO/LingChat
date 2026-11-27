# 插件示例

这个插件用三条最小示例演示插件平台能做什么，也可以直接当模板抄。

1. **脚本**：在插件目录里运行自己的 Node 脚本，读输入文件、写输出文件、拿回 stdout。
2. **网页**：起一个本地网页服务，让宿主把它放进应用内的沙箱窗口。
3. **技能**：插件自带一份技能，聊天时让导演 Agent 读取它，读到的内容通过旁白报出来。

## 目录结构

```
manifest.toml                    插件声明：id、能力开关、配置项、工具入口
main.py                          工具入口：run() 按 tool_name 分发
hooks.py                         导演后台任务结束后的旁白
scripts/hello.mjs                脚本示例：读 input.txt，写 out/greeting.txt
scripts/view.mjs                 网页示例：零依赖本地页面
skills/secret-note/SKILL.md      技能说明，导演靠它知道该读哪个文件
skills/secret-note/references/note.md  参考文件，里面是那张神秘纸条
```

## 怎么试

本机需要 Node 20+ 且 `node` 在 PATH 中。在 设置 → 插件 里启用「插件示例」：

- 启用和应用启动时会自动跑一次 `scripts/hello.mjs`，往台词表写一条「启用时脚本示例运行通过」。
  不想自动跑就在插件设置里关掉 `auto_demo`。
- 在主聊天里说「把插件示例全部跑一遍」，或者直接点名工具：

| 工具              | 做什么               |
| ----------------- | -------------------- |
| `showcase_script` | 只跑脚本示例         |
| `showcase_view`   | 只打开示范网页       |
| `showcase_skill`  | 只投导演读纸条的任务 |
| `showcase_all`    | 三条一起跑           |

模型没主动挑工具时，直接说「调用 showcase_all」。

预期：

- 脚本工具返回退出码 0，输出文件里有 `input.txt` 和 `out/greeting.txt`；
- 出现标题为「插件示例 · 视图窗口」的窗口，页面显示示范内容，按住可以拖动那个小方块堆；关掉窗口后对应 Node 进程消失；
- 导演完成后旁白写「导演读到了神秘的话：「祝你今天开心」」。这需要配好上帝 Agent 的模型；没配的话脚本和网页仍可单独验证。

现场在 `data/plugin_runs/plugin_showcase/`（每次脚本执行一个工作目录）和 `data/log/`。

## 照着写自己的插件

1. 复制整个目录，比如叫 `my_plugin`，改 `manifest.toml` 里的 `id` 和 `name`。
2. 只保留你需要的能力开关：`script_run`（跑脚本）、`plugin_view`（开窗口）、`director_task`（投导演任务）。
   没声明的能力，插件调用对应函数会拿到明确的拒绝。
3. 在 `[[tools]]` 里声明工具，`script` 指向处理它的 Python 文件。工具默认只对默认角色组开放。
4. Python 侧只依赖 `plugin_host`，没有文件系统、子进程和网络；要跑 Node 就通过 `script_run`，
   要出界面就 `script_serve` + `open_plugin_view`。
5. 想带技能，把 `skills/<名字>/SKILL.md` 放进插件目录，投导演任务时在 `skills` 里声明名字即可。

更完整的接口说明见 `docs/plugin-dev-guide.md` 与 `docs/plugin_host.pyi`。

## 边界

- 视图窗口只接受回环 `http://127.0.0.1` / `localhost` / `[::1]`，不注入 LingChat 前端与 Tauri IPC。
- `script_run` 走 Node 权限模型：只读插件目录与本次工作目录、只写工作目录、禁网、禁子进程。
- `script_serve` 跑的是插件自带的可信页面脚本，不受上述权限限制，但仍只认插件目录内的脚本。
