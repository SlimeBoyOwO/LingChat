# 热词导入链路修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复批量导入的空词条漏洞，恢复可复跑的解析 fixture，并让“从词库添加”与批量导入遵守同一容量口径。

**Architecture:** 解析器在格式匹配后增加 trim 判空；fixture 按当前严格格式重写，用同一份纯函数守住格式、长度、去重与容量契约；选择器接收宿主容量，按剩余额度禁用超额勾选，确认时再做一次上限兜底。文档只加状态说明与验证命令修正，不重写历史设计。

**Tech Stack:** TypeScript、Vue 3、vue-i18n、Tauri invoke、Node + tsc、Tailwind CSS v4、Prettier

**Spec:** `docs/superpowers/specs/2026-10-20-hotword-batch-import-design.md`（历史设计，格式以 `docs/asr/README.md` 与 `src/utils/hotwordImport.ts` 为准）

## Global Constraints

- 代码与注释用中文；注释中禁止出现 Markdown 语法。
- 最小改动，不做无关重构；本次不改后端。
- 不提交 git，不运行 `pnpm tauri dev`。
- zh-HK 语言包是手写粤语，直接改，不跑生成脚本。
- 前端验收命令是 `pnpm build`，不得加 `tail` / `head` 等过滤。
- 解析器长度口径必须与后端一致：含非 ASCII 按码点 ≤ 15，纯 ASCII 按空白片段 ≤ 7。

## Review Focus

- 形如 `1. (5)` 的空白词条：必须判为异常，不能占容量，也不能让同一输入里的真实词被判超限。
- 形如 `1.甲(6)` 的非法权重：必须报 badWeight，不能静默归一。
- 词库目标分类判重：同词在别的分类不算重复，在目标分类才算。
- 选择器在草稿已满或已选满时：未选项不可再选，全选不可越过剩余额度，确认不能超额。
- fixture 自身失败时：必须以非零退出码结束，不能打印成功。

---

### Task 1: 空词条校验与解析 fixture

**Files:**

- Modify: `src/utils/hotwordImport.ts:111-149`
- Rewrite: `scripts/verify-hotword-import.cjs`

**Interfaces:**

- Consumes: `parseHotwordImport(text, existingRows, capacity, targetCategory?) -> ImportReview`
- Produces: 同样的签名；`details` 中新增一种 `badFormat` 行（`text` 为原始行，`weight` 为空串），该行不进入 `rows`。

- [ ] **Step 1: 重写 fixture，先加入空词条回归断言**

脚本自行用 tsc 编译 `src/utils/hotwordImport.ts` 到系统临时目录，打桩 `@tauri-apps/api/core`，然后断言：

```js
// 正常与全角
rowsOf("1.钦灵(5)\n2.deepseek(2)") // [{text:"钦灵",weight:5},{text:"deepseek",weight:2}]
rowsOf("１．钦灵（５）") // [{text:"钦灵",weight:5}]

// 格式错误与空词条
parse("1.钦灵").invalidCount === 1 && details[0].reason === "badFormat"
parse("1. (5)").rows.length === 0 && parse("1. (5)").invalidCount === 1
parse("1. (5)\n2.真词(4)", [], 1) // rows [{text:"真词",weight:4}]，overCapacityCount === 0

// 非法权重
for (const w of [0, 6, 49, 51]) parse(`1.甲(${w})`).details[0].reason === "badWeight"

// 长度边界
"一".repeat(15) 通过；"一".repeat(16) 拒绝
"one two three four five six seven" 通过；再加 " eight" 拒绝
15 个 emoji 通过；16 个拒绝

// 去重与容量
parse("1.甲(2)\n2.甲(5)\n3.乙(4)") // rows 甲5、乙4，inputDuplicateCount 1
parse("1.甲(5)", [{text:"甲"}]) // mergeCount 1
parse("1.甲(5)", [{text:"甲",category_index:0}], 200, 0) // mergeCount 1
parse("1.甲(5)", [{text:"甲",category_index:0}], 200, 1) // newCount 1
parse("1.甲(4)\n2.乙(4)", [], 1) // rows 1 条，overCapacityCount 1
parse("1.甲(5)", [{text:"甲"}], 0) // 重复词仍可合并，overCapacityCount 0

// 换行与 BOM
rowsOf("﻿1.热词(4)\r\n2.词条(5)") // 2 条
```

- [ ] **Step 2: 运行 fixture，确认空词条断言失败**

Run: `node scripts/verify-hotword-import.cjs`
Expected: FAIL，空词条用例断言 `rows.length === 0` 不成立（当前会返回一条 `{text:"",weight:5}`）。

- [ ] **Step 3: 在解析器中判空**

在 `src/utils/hotwordImport.ts` 取出 `content` 之后、权重校验之前增加：

```ts
if (content === "") {
  invalidCount += 1;
  details.push({
    kind: "invalid",
    line: lineNo,
    label,
    text: line,
    weight: "",
    reason: "badFormat",
  });
  return;
}
```

- [ ] **Step 4: 运行 fixture，确认全部通过**

Run: `node scripts/verify-hotword-import.cjs`
Expected: `hotword import fixture: all assertions passed`，退出码 0。

---

### Task 2: 词库选择器的容量限制

**Files:**

- Modify: `src/components/settings/asr/HotwordLibraryPicker.vue`
- Modify: `src/components/settings/character/HotwordSettings.vue:158-164`
- Modify: `src/locales/zh-CN/settings.ts`、`zh-HK`、`ja`、`en` 的 `settings.asr.hotwordLibrary.picker`

**Interfaces:**

- Consumes: 宿主传入 `capacity: number`（角色/全局均为 `MAX_HOTWORDS_PER_ROLE`）。
- Produces: 选择器在 `picked.size` 达到 `capacity - existing.length` 后禁止继续勾选；`confirm` 在超额时直接返回。

- [ ] **Step 1: 增加 capacity 入参与剩余额度计算**

```ts
const props = defineProps<{
  visible: boolean;
  existing: RoleHotword[];
  capacity: number;
}>();

const remaining = computed(() => Math.max(0, props.capacity - props.existing.length));
const availableSlots = computed(() => Math.max(0, remaining.value - picked.value.size));
const overCapacity = computed(() => picked.value.size > remaining.value);
const canToggleAll = computed(() => {
  if (selectable.value.length === 0) return false;
  if (allSelected.value) return true;
  return selectable.value.length <= availableSlots.value;
});
```

- [ ] **Step 2: 禁止超额勾选与全选**

勾选框：`row.existing || (!picked.has(row.word) && availableSlots === 0)` 时禁用。
`toggle`：新增前若 `availableSlots === 0` 直接返回。
`toggleAll`：`canToggleAll` 为假直接返回；其余逻辑不变。
全选按钮增加 `:disabled="!canToggleAll"` 与禁用态样式。
确认按钮增加 `:disabled="picked.size === 0 || loading || overCapacity"`，`confirm` 内再加 `if (picked.size === 0 || overCapacity) return;`。

- [ ] **Step 3: 显示剩余额度或已达上限**

页脚左侧改为同时显示已选与额度：

```html
<span class="flex flex-wrap items-center gap-x-2 text-sm text-white/60">
  <span>{{ $t("settings.asr.hotwordLibrary.picker.selected", { count: picked.size }) }}</span>
  <span v-if="availableSlots === 0" class="text-amber-300">
    {{ $t("settings.asr.hotwordLibrary.picker.limitReached") }}
  </span>
  <span v-else class="text-white/40">
    {{ $t("settings.asr.hotwordLibrary.picker.remaining", { count: availableSlots }) }}
  </span>
</span>
```

四套 locale 的 `picker` 下新增：

```ts
remaining: "剩余 {count} 条";
limitReached: "已达上限，取消勾选后可继续添加";
// zh-HK：remaining: "淨返 {count} 條"；limitReached: "已達上限，取消揀選後可以繼續加入"
// ja：remaining: "残り {count} 件"；limitReached: "上限に達しました。選択を外すと追加できます"
// en：remaining: "{count} slots left"; limitReached: "Limit reached — deselect to add more"
```

- [ ] **Step 4: 宿主传入容量**

`HotwordSettings.vue` 的 `<HotwordLibraryPicker>` 增加 `:capacity="MAX_HOTWORDS_PER_ROLE"`。

- [ ] **Step 5: 前端构建检查**

Run: `pnpm build`
Expected: 类型检查与生产构建通过，无报错。

---

### Task 3: 文档状态与过时临时文件

**Files:**

- Modify: `docs/superpowers/specs/2026-10-20-hotword-batch-import-design.md:1-5, 539-560`
- Delete: `temp/hotword-lib-check.mjs`

- [ ] **Step 1: 标注设计已被取代**

把顶部状态行改为：

```markdown
- 状态：已被 2026-10-23 的严格格式实现取代；当前格式以 docs/asr/README.md 与 src/utils/hotwordImport.ts 为准，本文保留作历史设计记录
```

- [ ] **Step 2: 修正 fixture 用法与覆盖范围**

把 11.1 的 tsc 命令块替换为：

```bash
node scripts/verify-hotword-import.cjs
```

说明脚本自行编译并打桩，覆盖严格格式、全角、空词条、非法权重、长度边界、去重、容量与 CRLF/BOM；删除旧的 Markdown/自然列表/默认权重描述。

- [ ] **Step 3: 删除过时临时脚本**

Run: `rm temp/hotword-lib-check.mjs`
Expected: 文件不存在；`git status` 不再显示该未跟踪文件。

---

### Task 4: 最终验证

- [ ] **Step 1: 解析 fixture**

Run: `node scripts/verify-hotword-import.cjs`
Expected: `hotword import fixture: all assertions passed`。

- [ ] **Step 2: 前端构建**

Run: `pnpm build`
Expected: 退出码 0。

- [ ] **Step 3: 检查改动范围**

Run: `git status --short` 与 `git diff --check`
Expected: 只有本次计划涉及的文件；无空白错误；无提交。
