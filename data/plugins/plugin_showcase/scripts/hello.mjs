// 最小脚本示例：读宿主写进工作目录的输入文件，在 out/ 里产出结果。
// 宿主用 Node 权限模型运行它：只能读插件目录和本次工作目录，只能写工作目录。
import fs from "node:fs/promises";

const input = await fs.readFile("input.txt", "utf8").catch(() => "（没有输入文件）");
await fs.mkdir("out", { recursive: true });

const at = new Date().toLocaleString("zh-CN", { hour12: false });
await fs.writeFile("out/greeting.txt", `来自插件的问候：${input.trim()}\n运行时间：${at}\n`);

console.log(JSON.stringify({ ok: true, input: input.trim(), at }));
