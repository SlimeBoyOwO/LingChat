#!/usr/bin/env node
/**
 * 生成 docs/database 下的两张 ER 图：
 *   er-crowfoot.svg  鸦脚表示法（物理模型，含全部列、外键、索引与迁移来源）
 *   er-chen.svg      Chen 表示法（概念模型，实体 / 关系 / 属性）
 *
 * 迁移或实体变更后重新运行：node docs/database/gen-er.mjs
 * 数据来源：crates/ling-chat-main/src/db/entities 与 src/migration。
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const OUT_DIR = path.dirname(fileURLToPath(import.meta.url));

// ---------------------------------------------------------------- 字体与配色

// 属性值用双引号包裹，字体名内部改用单引号，避免破坏 XML
const FONT = `'Microsoft YaHei', 'PingFang SC', 'Noto Sans CJK SC', 'Segoe UI', sans-serif`;
const MONO = `'Cascadia Mono', 'JetBrains Mono', Consolas, 'Courier New', monospace`;

const CLUSTERS = {
  core: { name: "核心对话", color: "#2563eb", light: "#eff6ff" },
  identity: { name: "身份与认知", color: "#0d9488", light: "#ecfdf5" },
  hotword: { name: "ASR 热词", color: "#b45309", light: "#fffbeb" },
  agent: { name: "AI 助手", color: "#7c3aed", light: "#f5f3ff" },
  misc: { name: "独立表", color: "#475569", light: "#f8fafc" },
};

// ---------------------------------------------------------------- schema 数据

/** 列定义：n 名称，t 类型，pk 主键，fk 外键，nn 非空，def 默认值，m 后续迁移新增，u 唯一，chk 检查约束 */
const C = (n, t, o = {}) => ({ n, t, ...o });

const schema = {
  role: {
    label: "角色",
    cluster: "core",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("script_key", "VARCHAR(255)"),
      C("script_role_key", "VARCHAR(255)"),
      C("name", "VARCHAR(255)", { nn: true }),
      C("role_type", "VARCHAR(32)", { nn: true, def: "'npc'" }),
      C("resource_folder", "VARCHAR(512)"),
      C("identity_id", "TEXT", { m: "m20261114", u: true }),
      C("identity_relations_seeded", "INTEGER", { m: "m20261114", nn: true, def: "0" }),
    ],
    cons: [
      "UQ uq_script_role (script_key, script_role_key)",
      "UQ uq_role_identity (identity_id)",
      "IX ix_role_name (name)",
      "role_type: main / npc / system / user / player",
    ],
    mig: "建表 m20240101 · 加列 m20261114",
  },
  save: {
    label: "存档",
    cluster: "core",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("title", "VARCHAR(255)", { nn: true }),
      C("status", "TEXT", { nn: true, def: "'{}'" }),
      C("create_date", "DATETIME", { nn: true }),
      C("update_date", "DATETIME", { nn: true }),
      C("running_script_id", "INTEGER", { fk: { t: "running_script", c: "id", logical: true } }),
      C("last_message_id", "INTEGER", { fk: { t: "line", c: "id", logical: true } }),
      C("main_role_id", "INTEGER", { fk: { t: "role", c: "id" } }),
      C("player_role_id", "INTEGER", { m: "m20261114", fk: { t: "role", c: "id", logical: true } }),
    ],
    cons: ["IX ix_save_player_role (player_role_id)", "player_role_id 为应用层引用，无 SQL 外键"],
    mig: "建表 m20240101 · 加列 m20261114",
  },
  running_script: {
    label: "剧本运行态",
    cluster: "core",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("script_folder", "VARCHAR(512)", { nn: true }),
      C("variable_info", "TEXT", { nn: true, def: "'{}'" }),
      C("current_chapter", "VARCHAR(255)", { nn: true }),
      C("event_sequence", "INTEGER", { nn: true }),
      C("save_id", "INTEGER", { nn: true, fk: { t: "save", c: "id" } }),
    ],
    cons: ["IX ix_running_script_folder (script_folder)"],
    mig: "建表 m20240101",
  },
  line: {
    label: "台词",
    cluster: "core",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("content", "TEXT", { nn: true }),
      C("original_emotion", "VARCHAR(64)"),
      C("predicted_emotion", "VARCHAR(64)"),
      C("tts_content", "TEXT"),
      C("action_content", "TEXT"),
      C("audio_file", "VARCHAR(512)"),
      C("thinking", "TEXT", { m: "m20260729" }),
      C("attribute", "VARCHAR(32)", { nn: true }),
      C("sender_role_id", "INTEGER", { fk: { t: "role", c: "id" } }),
      C("display_name", "VARCHAR(255)"),
      C("save_id", "INTEGER", { nn: true, fk: { t: "save", c: "id" } }),
      C("tool_call", "TEXT", { m: "m20260727" }),
      C("round_id", "TEXT", { m: "m20261020" }),
      C("parent_line_id", "INTEGER", { fk: { t: "line", c: "id" } }),
    ],
    cons: ["FK save_id / sender_role_id / parent_line_id"],
    mig: "建表 m20240101 · 加列 0727 0729 1020 · 去列 m20261114",
  },
  line_perception: {
    label: "台词可见性",
    cluster: "core",
    cols: [
      C("line_id", "INTEGER", { pk: true, nn: true, fk: { t: "line", c: "id" } }),
      C("role_id", "INTEGER", { pk: true, nn: true, fk: { t: "role", c: "id" } }),
    ],
    cons: ["PK (line_id, role_id)", "FK line_id / role_id"],
    mig: "建表 m20240101",
  },
  memory_bank: {
    label: "持久记忆",
    cluster: "core",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("info", "TEXT", { nn: true, def: "'{}'" }),
      C("save_id", "INTEGER", { nn: true, fk: { t: "save", c: "id" } }),
      C("role_id", "INTEGER", { fk: { t: "role", c: "id" } }),
    ],
    cons: [],
    mig: "建表 m20240101",
  },
  adventure_unlock: {
    label: "冒险解锁",
    cluster: "misc",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("adventure_folder", "VARCHAR(512)", { nn: true, u: true }),
      C("character_folder", "VARCHAR(512)", { nn: true }),
      C("unlocked_at", "DATETIME"),
      C("completed_at", "DATETIME"),
    ],
    cons: ["UQ adventure_folder", "IX ix_adventure_character (character_folder)"],
    mig: "建表 m20240101 · compat 去 user_id",
  },
  skill_agent_conversation: {
    label: "助手会话",
    cluster: "agent",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("title", "VARCHAR(255)"),
      C("script_key", "VARCHAR(255)"),
      C("created_at", "DATETIME", { nn: true }),
      C("updated_at", "DATETIME", { nn: true }),
    ],
    cons: [],
    mig: "建表 m20260803",
  },
  skill_agent_message: {
    label: "助手消息",
    cluster: "agent",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("conversation_id", "INTEGER", {
        nn: true,
        fk: { t: "skill_agent_conversation", c: "id", logical: true },
      }),
      C("role", "VARCHAR(32)", { nn: true }),
      C("content", "TEXT"),
      C("reasoning", "TEXT", { m: "m20260807" }),
      C("prompt_tokens", "INTEGER", { m: "m20260814" }),
      C("completion_tokens", "INTEGER", { m: "m20260814" }),
      C("cached_tokens", "INTEGER", { m: "m20260815" }),
      C("tool_calls", "TEXT"),
      C("tool_call_id", "VARCHAR(255)"),
      C("created_at", "DATETIME", { nn: true }),
    ],
    cons: ["IX ix_skill_agent_message_conversation (conversation_id)"],
    mig: "建表 m20260803 · 加列 0807 0814 0815",
  },
  role_hotword: {
    label: "角色热词",
    cluster: "hotword",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("role_id", "INTEGER", { nn: true, fk: { t: "role", c: "id", logical: true } }),
      C("text", "TEXT", { nn: true }),
      C("weight", "INTEGER", { nn: true, def: "4", chk: "1..50" }),
    ],
    cons: ["UQ (role_id, text)", "CHECK weight 1..50"],
    mig: "建表 m20261004 · 重建 1010 · 归一 1012",
  },
  global_hotword: {
    label: "全局热词",
    cluster: "hotword",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("text", "TEXT", { nn: true, u: true }),
      C("weight", "INTEGER", { nn: true, def: "4", chk: "1..50" }),
    ],
    cons: ["UQ text", "CHECK weight 1..50"],
    mig: "建表 m20261005 · 重建 1010 · 归一 1012",
  },
  hotword_category: {
    label: "词库分类",
    cluster: "hotword",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("name", "TEXT", { nn: true, u: true }),
    ],
    cons: ["UQ name"],
    mig: "建表 m20261009",
  },
  library_hotword: {
    label: "词库热词",
    cluster: "hotword",
    cols: [
      C("id", "INTEGER", { pk: true, ai: true, nn: true }),
      C("category_id", "INTEGER", { fk: { t: "hotword_category", c: "id", logical: true } }),
      C("text", "TEXT", { nn: true }),
      C("weight", "INTEGER", { nn: true, def: "4", chk: "1..50" }),
    ],
    cons: ["UQ (category_id, text) WHERE 非空", "UQ (text) WHERE 未分类", "CHECK weight 1..50"],
    mig: "建表 m20261009 · 重建 1010 · 归一 1012 · 唯一键 1023",
  },
  role_relation: {
    label: "角色认知",
    cluster: "identity",
    cols: [
      C("role_id", "INTEGER", { pk: true, nn: true, fk: { t: "role", c: "id" } }),
      C("target_role_id", "INTEGER", { pk: true, nn: true, fk: { t: "role", c: "id" } }),
      C("text", "TEXT", { nn: true, def: "''" }),
      C("initial_affection", "INTEGER"),
      C("updated_at", "TEXT", { nn: true }),
    ],
    cons: [
      "PK (role_id, target_role_id)",
      "IX ix_role_relation_target (target_role_id)",
      "FK role_id / target_role_id → role",
    ],
    mig: "建表 m20261114（承接 role_identity_relation）",
  },
};

// ---------------------------------------------------------------- 通用 SVG

const esc = (s) =>
  String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

function textEl(x, y, s, o = {}) {
  const {
    size = 12,
    fill = "#0f172a",
    anchor = "start",
    weight = 400,
    mono = false,
    opacity = 1,
    rotate = null,
    halo = false,
  } = o;
  const tr = rotate === null ? "" : ` transform="rotate(${rotate} ${x} ${y})"`;
  const hl = halo
    ? ` stroke="#ffffff" stroke-width="3.5" paint-order="stroke" stroke-linejoin="round"`
    : "";
  return `<text x="${x}" y="${y}" font-size="${size}" fill="${fill}" text-anchor="${anchor}" font-weight="${weight}" font-family="${mono ? MONO : FONT}" opacity="${opacity}"${hl}${tr}>${esc(s)}</text>`;
}

function roundedPath(pts, r = 10) {
  const p = [];
  for (const q of pts) {
    if (
      !p.length ||
      Math.abs(p[p.length - 1].x - q.x) > 0.01 ||
      Math.abs(p[p.length - 1].y - q.y) > 0.01
    )
      p.push(q);
  }
  if (p.length < 2) return "";
  let d = `M ${p[0].x} ${p[0].y}`;
  for (let i = 1; i < p.length - 1; i++) {
    const a = p[i - 1];
    const b = p[i];
    const c = p[i + 1];
    const v1 = { x: a.x - b.x, y: a.y - b.y };
    const v2 = { x: c.x - b.x, y: c.y - b.y };
    const l1 = Math.hypot(v1.x, v1.y) || 1;
    const l2 = Math.hypot(v2.x, v2.y) || 1;
    const rr = Math.min(r, l1 / 2, l2 / 2);
    const s1 = { x: b.x + (v1.x / l1) * rr, y: b.y + (v1.y / l1) * rr };
    const s2 = { x: b.x + (v2.x / l2) * rr, y: b.y + (v2.y / l2) * rr };
    d += ` L ${s1.x.toFixed(1)} ${s1.y.toFixed(1)} Q ${b.x} ${b.y} ${s2.x.toFixed(1)} ${s2.y.toFixed(1)}`;
  }
  const last = p[p.length - 1];
  return `${d} L ${last.x} ${last.y}`;
}

/** 端点符号。angle 为“离开实体”的方向角（度） */
function endMarker(x, y, angle, kind, color, solid) {
  const g = [];
  const dash = solid ? "" : ` stroke-dasharray="3 2"`;
  const prong = (x1, y1, x2, y2) =>
    `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${color}" stroke-width="1.6"${dash}/>`;
  if (kind === "one") {
    g.push(prong(11, -6, 11, 6));
  } else if (kind === "zero-one") {
    g.push(`<circle cx="8" cy="0" r="3.5" fill="none" stroke="${color}" stroke-width="1.6"/>`);
    g.push(prong(24, -6, 24, 6));
  } else if (kind === "many" || kind === "zero-many") {
    const base = kind === "zero-many" ? 9 : 0;
    if (kind === "zero-many")
      g.push(`<circle cx="5" cy="0" r="3.5" fill="none" stroke="${color}" stroke-width="1.6"/>`);
    g.push(prong(base, 0, base + 13, -6));
    g.push(prong(base, 0, base + 13, 0));
    g.push(prong(base, 0, base + 13, 6));
  }
  return `<g transform="translate(${x} ${y}) rotate(${angle})" stroke-linecap="round">${g.join("")}</g>`;
}

// ---------------------------------------------------------------- 鸦脚图

const CW = 440; // 表框宽
const CHH = 34; // 表头高
const CRH = 22; // 行高

/** 页脚：分隔线 + 约束行 + 迁移行 */
function consH(t) {
  const s = schema[t];
  return (s.cons?.length ?? 0) * 13 + 27;
}

function boxH(t) {
  return CHH + schema[t].cols.length * CRH + consH(t);
}

const layout = {
  role: { x: 90, y: 200 },
  line: { x: 90, y: 520 },
  line_perception: { x: 90, y: 960 },
  save: { x: 670, y: 200 },
  running_script: { x: 670, y: 500 },
  adventure_unlock: { x: 670, y: 730 },
  memory_bank: { x: 1250, y: 200 },
  role_relation: { x: 1250, y: 390 },
  skill_agent_conversation: { x: 1250, y: 650 },
  skill_agent_message: { x: 1250, y: 840 },
  role_hotword: { x: 1830, y: 200 },
  global_hotword: { x: 1830, y: 420 },
  hotword_category: { x: 1830, y: 620 },
  library_hotword: { x: 1830, y: 780 },
};

const bottom = (t) => layout[t].y + boxH(t);
const cyOf = (t) => layout[t].y + boxH(t) / 2;
const rowY = (t, col) => {
  const i = schema[t].cols.findIndex((c) => c.n === col);
  return layout[t].y + CHH + i * CRH + CRH / 2;
};

/** 边：pts 折线；a/b 两端符号；label 可选 {s,x,y,rot} */
const edges = [
  {
    pts: [
      { x: 530, y: rowY("role", "id") },
      { x: 600, y: rowY("role", "id") },
      { x: 600, y: rowY("save", "main_role_id") },
      { x: 670, y: rowY("save", "main_role_id") },
    ],
    a: "one",
    b: "many",
    label: { s: "主角", x: 600, y: rowY("role", "id") - 8 },
  },
  {
    pts: [
      { x: 670, y: rowY("save", "player_role_id") },
      { x: 620, y: rowY("save", "player_role_id") },
      { x: 620, y: rowY("role", "id") + 14 },
      { x: 530, y: rowY("role", "id") + 14 },
    ],
    a: "zero-one",
    b: "one",
    logical: true,
    label: { s: "玩家主角", x: 645, y: 340, rot: -90 },
  },
  {
    pts: [
      { x: 90, y: rowY("line", "sender_role_id") },
      { x: 52, y: rowY("line", "sender_role_id") },
      { x: 52, y: 470 },
      { x: 90, y: 470 },
    ],
    a: "many",
    b: "one",
    label: { s: "发送", x: 52, y: 660, rot: -90 },
  },
  {
    pts: [
      { x: 530, y: rowY("line", "save_id") },
      { x: 640, y: rowY("line", "save_id") },
      { x: 640, y: 495 },
      { x: 760, y: 495 },
      { x: 760, y: bottom("save") },
    ],
    a: "many",
    b: "one",
    label: { s: "所属", x: 640, y: 745, rot: -90 },
  },
  {
    pts: [
      { x: 340, y: bottom("line") },
      { x: 340, y: 940 },
      { x: 440, y: 940 },
      { x: 440, y: bottom("line") },
    ],
    a: "one",
    b: "many",
    label: { s: "父级", x: 390, y: 952 },
  },
  {
    pts: [
      { x: 310, y: 960 },
      { x: 310, y: bottom("line") },
    ],
    a: "many",
    b: "one",
  },
  {
    pts: [
      { x: 90, y: rowY("line_perception", "role_id") },
      { x: 34, y: rowY("line_perception", "role_id") },
      { x: 34, y: 455 },
      { x: 90, y: 455 },
    ],
    a: "many",
    b: "one",
    label: { s: "感知", x: 34, y: 800, rot: -90 },
  },
  {
    pts: [
      { x: 1250, y: rowY("memory_bank", "save_id") },
      { x: 1110, y: rowY("memory_bank", "save_id") },
    ],
    a: "many",
    b: "one",
    label: { s: "存档记忆", x: 1180, y: rowY("memory_bank", "save_id") - 8 },
  },
  {
    pts: [
      { x: 1350, y: 200 },
      { x: 1350, y: 174 },
      { x: 400, y: 174 },
      { x: 400, y: 200 },
    ],
    a: "many",
    b: "one",
    label: { s: "角色记忆", x: 900, y: 166 },
  },
  {
    pts: [
      { x: 800, y: bottom("save") },
      { x: 800, y: 500 },
    ],
    a: "one",
    b: "zero-one",
    label: { s: "运行 1:1", x: 812, y: 495 },
  },
  // role_relation 两侧都指向 role，画一条代表「认知」：主语行与目标行同表
  {
    pts: [
      { x: 1250, y: rowY("role_relation", "role_id") },
      { x: 1180, y: rowY("role_relation", "role_id") },
      { x: 1180, y: 718 },
      { x: 560, y: 718 },
      { x: 560, y: 480 },
      { x: 530, y: 480 },
    ],
    a: "many",
    b: "one",
    logical: true,
    label: { s: "认知", x: 860, y: 710 },
  },
  {
    pts: [
      { x: 200, y: 200 },
      { x: 200, y: 184 },
      { x: 1810, y: 184 },
      { x: 1810, y: 267 },
      { x: 1830, y: 267 },
    ],
    a: "one",
    b: "zero-many",
    logical: true,
    label: { s: "角色热词", x: 1794, y: 226, rot: -90 },
  },
  {
    pts: [
      { x: 2050, y: bottom("hotword_category") },
      { x: 2050, y: 780 },
    ],
    a: "one",
    b: "zero-many",
    logical: true,
  },
  {
    pts: [
      { x: 1350, y: bottom("skill_agent_conversation") },
      { x: 1350, y: 840 },
    ],
    a: "one",
    b: "zero-many",
    logical: true,
  },
];

function renderCrowfoot() {
  const parts = [];
  const W = 2380;
  const H = 1460;

  parts.push(`<rect width="${W}" height="${H}" fill="#ffffff"/>`);
  parts.push(
    textEl(60, 62, "LingChat 数据库 ER 图 · 鸦脚表示法", {
      size: 28,
      weight: 700,
      fill: "#0f172a",
    }),
  );
  parts.push(
    textEl(
      60,
      92,
      "物理模型：14 张表，覆盖 m20240101 ~ m20261114 全部迁移；点线为无外键约束的应用层引用",
      { size: 13, fill: "#475569" },
    ),
  );

  // 图例
  const lx = 60;
  const ly = 118;
  parts.push(
    `<rect x="${lx}" y="${ly}" width="1300" height="46" rx="8" fill="#f8fafc" stroke="#e2e8f0"/>`,
  );
  const legend = [
    ["实线", "SQLite FOREIGN KEY", "#2563eb", false],
    ["虚线", "应用层引用", "#7c3aed", true],
  ];
  let cx = lx + 16;
  for (const [n, d, col, dashed] of legend) {
    parts.push(
      `<line x1="${cx}" y1="${ly + 23}" x2="${cx + 40}" y2="${ly + 23}" stroke="${col}" stroke-width="2"${dashed ? ` stroke-dasharray="7 5"` : ""}/>`,
    );
    parts.push(textEl(cx + 48, ly + 20, n, { size: 12, weight: 600, fill: "#0f172a" }));
    parts.push(textEl(cx + 48, ly + 35, d, { size: 10, fill: "#64748b" }));
    cx += 190;
  }
  // 端点符号说明
  const sym = (kind, x) => {
    parts.push(
      `<line x1="${x}" y1="${ly + 23}" x2="${x + 34}" y2="${ly + 23}" stroke="#64748b" stroke-width="1.6"/>`,
    );
    parts.push(endMarker(x + 34, ly + 23, 0, kind, "#64748b", true));
  };
  sym("one", cx);
  parts.push(textEl(cx + 44, ly + 27, "1", { size: 11, fill: "#334155" }));
  sym("many", cx + 80);
  parts.push(textEl(cx + 124, ly + 27, "N", { size: 11, fill: "#334155" }));
  sym("zero-one", cx + 160);
  parts.push(textEl(cx + 210, ly + 27, "0..1", { size: 11, fill: "#334155" }));
  sym("zero-many", cx + 250);
  parts.push(textEl(cx + 300, ly + 27, "0..N", { size: 11, fill: "#334155" }));

  parts.push(`<circle cx="${lx + 1180}" cy="${ly + 20}" r="3" fill="#f59e0b"/>`);
  parts.push(textEl(lx + 1190, ly + 24, "后续迁移新增列", { size: 11, fill: "#334155" }));
  parts.push(
    textEl(1390, ly + 20, "类型后缀 ? = 可空 · PK/FK 实心 = 物理外键，空心 = 应用层引用", {
      size: 10.5,
      fill: "#64748b",
    }),
  );

  // 先画连线，再画表框，让表框盖住线头
  const lines = [];
  for (const e of edges) {
    const col = e.logical ? "#7c3aed" : "#2563eb";
    const dashed = !!e.logical;
    const d = roundedPath(e.pts, 9);
    lines.push(
      `<path d="${d}" fill="none" stroke="#ffffff" stroke-width="6" stroke-linecap="round"/>`,
    );
    lines.push(
      `<path d="${d}" fill="none" stroke="${col}" stroke-width="1.8" stroke-linecap="round"${dashed ? ` stroke-dasharray="7 5"` : ""}/>`,
    );
  }
  parts.push(...lines);

  // 端点符号与标签
  for (const e of edges) {
    const col = e.logical ? "#7c3aed" : "#2563eb";
    const p0 = e.pts[0];
    const p1 = e.pts[1];
    const a0 = (Math.atan2(p1.y - p0.y, p1.x - p0.x) * 180) / Math.PI;
    const q0 = e.pts[e.pts.length - 1];
    const q1 = e.pts[e.pts.length - 2];
    const a1 = (Math.atan2(q1.y - q0.y, q1.x - q0.x) * 180) / Math.PI;
    parts.push(endMarker(p0.x, p0.y, a0, e.a, col, !e.logical));
    parts.push(endMarker(q0.x, q0.y, a1, e.b, col, !e.logical));
    if (e.label) {
      const rot = e.label.rot ?? 0;
      parts.push(
        textEl(e.label.x, e.label.y, e.label.s, {
          size: 11,
          fill: "#334155",
          anchor: "middle",
          weight: 500,
          halo: true,
          rotate: rot === 0 ? null : rot,
        }),
      );
    }
  }

  // 表框
  for (const t of Object.keys(layout)) {
    const s = schema[t];
    const cl = CLUSTERS[s.cluster];
    const L = layout[t];
    const h = boxH(t);
    const g = [];
    g.push(
      `<rect x="${L.x}" y="${L.y}" width="${CW}" height="${h}" rx="8" fill="#ffffff" stroke="${cl.color}" stroke-width="1.4"/>`,
    );
    g.push(
      `<path d="M ${L.x} ${L.y + 8} A 8 8 0 0 1 ${L.x + 8} ${L.y} H ${L.x + CW - 8} A 8 8 0 0 1 ${L.x + CW} ${L.y + 8} V ${L.y + CHH} H ${L.x} Z" fill="${cl.light}"/>`,
    );
    g.push(
      `<line x1="${L.x}" y1="${L.y + CHH}" x2="${L.x + CW}" y2="${L.y + CHH}" stroke="${cl.color}" stroke-width="1.2"/>`,
    );
    g.push(textEl(L.x + 12, L.y + 22, t, { size: 14, weight: 700, mono: true, fill: cl.color }));
    g.push(textEl(L.x + CW - 12, L.y + 22, s.label, { size: 12, fill: cl.color, anchor: "end" }));

    s.cols.forEach((c, i) => {
      const ry = L.y + CHH + i * CRH + CRH / 2;
      if (i % 2 === 1)
        g.push(
          `<rect x="${L.x + 1}" y="${ry - CRH / 2}" width="${CW - 2}" height="${CRH}" fill="#f8fafc"/>`,
        );
      if (c.m) g.push(`<circle cx="${L.x + 8}" cy="${ry}" r="2.6" fill="#f59e0b"/>`);
      let bx = L.x + 16;
      if (c.pk) {
        g.push(`<rect x="${bx}" y="${ry - 7}" width="18" height="13" rx="3" fill="${cl.color}"/>`);
        g.push(
          textEl(bx + 9, ry + 3.5, "PK", {
            size: 8.5,
            fill: "#ffffff",
            anchor: "middle",
            weight: 700,
          }),
        );
        bx += 22;
      }
      if (c.fk) {
        const logical = c.fk.logical;
        g.push(
          `<rect x="${bx}" y="${ry - 7}" width="18" height="13" rx="3" fill="${logical ? "none" : cl.color}" stroke="${cl.color}" stroke-width="1"/>`,
        );
        g.push(
          textEl(bx + 9, ry + 3.5, "FK", {
            size: 8.5,
            fill: logical ? cl.color : "#ffffff",
            anchor: "middle",
            weight: 700,
          }),
        );
        bx += 22;
      }
      if (c.u) {
        g.push(
          `<rect x="${bx}" y="${ry - 7}" width="16" height="13" rx="3" fill="none" stroke="#94a3b8" stroke-width="1"/>`,
        );
        g.push(
          textEl(bx + 8, ry + 3.5, "U", {
            size: 8.5,
            fill: "#64748b",
            anchor: "middle",
            weight: 700,
          }),
        );
        bx += 20;
      }
      g.push(textEl(bx, ry + 4, c.n, { size: 12, mono: true, fill: "#0f172a" }));
      const right = `${c.t}${c.n === "id" || !c.nn ? "?" : ""}${c.def ? ` = ${c.def}` : ""}`;
      g.push(
        textEl(L.x + CW - 10, ry + 4, right, {
          size: 9.5,
          mono: true,
          fill: c.nn ? "#64748b" : "#94a3b8",
          anchor: "end",
        }),
      );
    });

    // 页脚：约束 + 迁移
    const footerTop = L.y + CHH + s.cols.length * CRH;
    g.push(
      `<line x1="${L.x + 8}" y1="${footerTop}" x2="${L.x + CW - 8}" y2="${footerTop}" stroke="#e2e8f0"/>`,
    );
    let ty = footerTop + 19;
    for (const line of s.cons) {
      g.push(textEl(L.x + 12, ty, line, { size: 9.5, fill: "#64748b" }));
      ty += 13;
    }
    g.push(textEl(L.x + 12, ty + 2, s.mig, { size: 9.5, fill: "#94a3b8" }));

    parts.push(`<g>${g.join("")}</g>`);
  }

  parts.push(
    textEl(
      60,
      H - 26,
      "生成：docs/database/gen-er.mjs · 实体：crates/ling-chat-main/src/db/entities · 迁移：crates/ling-chat-main/src/migration",
      { size: 10.5, fill: "#94a3b8" },
    ),
  );

  return { svg: wrap(parts, W, H), W, H };
}

// ---------------------------------------------------------------- Chen 图

const ATTR_RY = 17;
const ATTR_GAP = 10;

function attrRx(name) {
  return Math.max(44, (name.length * 6.4 + 32) / 2);
}

/** 实体/引用框尺寸 */
function entitySize(t) {
  const s = schema[t];
  return { w: Math.max(190, t.length * 7.4 + s.label.length * 12 + 44), h: 66 };
}

const chen = {
  panels: [
    {
      id: "core",
      title: "核心对话",
      sub: "角色、存档、台词与剧本运行态",
      x: 60,
      y: 120,
      w: 2440,
      h: 1560,
      ents: [
        { t: "role", x: 640, y: 330, side: "l" },
        { t: "save", x: 1290, y: 330, side: "u" },
        { t: "line", x: 640, y: 980, side: "r" },
        { t: "running_script", x: 1840, y: 330, side: "r" },
        { t: "memory_bank", x: 1290, y: 830, side: "r" },
        { t: "adventure_unlock", x: 1840, y: 830, side: "r" },
      ],
      refs: [],
      diamonds: [
        {
          x: 965,
          y: 330,
          label: "主角",
          sub: "save.main_role_id",
          links: [
            { to: "role", card: "1" },
            { to: "save", card: "N" },
          ],
        },
        {
          x: 1290,
          y: 580,
          label: "存档记忆",
          sub: "memory_bank.save_id",
          links: [
            { to: "save", card: "1" },
            { to: "memory_bank", card: "N" },
          ],
        },
        {
          x: 965,
          y: 580,
          label: "角色记忆",
          sub: "memory_bank.role_id",
          links: [
            { to: "role", card: "1" },
            { to: "memory_bank", card: "N" },
          ],
        },
        {
          x: 1565,
          y: 330,
          label: "运行",
          sub: "running_script.save_id",
          links: [
            { to: "save", card: "1" },
            { to: "running_script", card: "1" },
          ],
        },
        {
          x: 640,
          y: 655,
          label: "发送",
          sub: "line.sender_role_id",
          links: [
            { to: "role", card: "1" },
            { to: "line", card: "N" },
          ],
        },
        {
          x: 360,
          y: 720,
          label: "感知",
          sub: "表 line_perception",
          links: [
            { to: "role", card: "M" },
            { to: "line", card: "N" },
          ],
          note: "M:N",
        },
        {
          x: 420,
          y: 1100,
          label: "父级",
          sub: "line.parent_line_id",
          links: [
            { to: "line", card: "1", pt: { x: 545, y: 1012 } },
            { to: "line", card: "N", pt: { x: 640, y: 1013 } },
          ],
        },
      ],
    },
    {
      id: "identity",
      title: "玩家身份与认知",
      sub: "身份卡（文件）投影为 role（role_type = player）；认知统一存 role_relation",
      x: 2540,
      y: 120,
      w: 1400,
      h: 1560,
      ents: [],
      refs: [
        { id: "save", x: 430, y: 300, note: "见核心图" },
        { id: "role", x: 430, y: 700, note: "见核心图" },
      ],
      diamonds: [
        {
          x: 830,
          y: 500,
          label: "玩家主角",
          sub: "save.player_role_id",
          links: [
            { to: "ref:save", card: "1" },
            { to: "ref:role", card: "N" },
          ],
        },
        {
          x: 900,
          y: 1060,
          label: "认知",
          sub: "表 role_relation",
          links: [
            { to: "ref:role", card: "M", pt: { x: 430, y: 690 } },
            { to: "ref:role", card: "N", pt: { x: 430, y: 712 } },
          ],
          note: "M:N",
          attrs: ["text", "initial_affection", "updated_at"],
          attrSide: "r",
        },
      ],
    },
    {
      id: "hotword",
      title: "ASR 热词",
      sub: "角色级 / 全局 / 词库三套热词",
      x: 60,
      y: 1720,
      w: 1500,
      h: 1180,
      ents: [
        { t: "hotword_category", x: 380, y: 230, side: "l" },
        { t: "library_hotword", x: 880, y: 230, side: "r" },
        { t: "role_hotword", x: 880, y: 620, side: "r" },
        { t: "global_hotword", x: 380, y: 940, side: "r" },
      ],
      refs: [{ id: "role", x: 380, y: 620, note: "见核心图" }],
      diamonds: [
        {
          x: 630,
          y: 230,
          label: "归类",
          sub: "library_hotword.category_id",
          links: [
            { to: "hotword_category", card: "1" },
            { to: "library_hotword", card: "N" },
          ],
        },
        {
          x: 630,
          y: 620,
          label: "拥有",
          sub: "role_hotword.role_id",
          links: [
            { to: "ref:role", card: "1" },
            { to: "role_hotword", card: "N" },
          ],
        },
      ],
    },
    {
      id: "agent",
      title: "AI 助手",
      sub: "剧本编辑器的助手会话",
      x: 1600,
      y: 1720,
      w: 1500,
      h: 1180,
      ents: [
        { t: "skill_agent_conversation", x: 420, y: 300, side: "l" },
        { t: "skill_agent_message", x: 970, y: 300, side: "r" },
      ],
      refs: [],
      diamonds: [
        {
          x: 695,
          y: 300,
          label: "会话",
          sub: "skill_agent_message.conversation_id",
          links: [
            { to: "skill_agent_conversation", card: "1" },
            { to: "skill_agent_message", card: "N" },
          ],
        },
      ],
    },
  ],
};

function diamondSize(label, sub) {
  const lines = [label, sub].filter(Boolean);
  const w = Math.max(132, Math.max(...lines.map((s) => s.length * (s === label ? 15 : 6.6))) + 40);
  const h = lines.length > 1 ? 62 : 54;
  return { w, h };
}

function renderChen() {
  const parts = [];
  const W = 4000;
  const H = 2960;
  parts.push(`<rect width="${W}" height="${H}" fill="#ffffff"/>`);
  parts.push(textEl(60, 62, "LingChat 数据库 ER 图 · Chen 表示法", { size: 28, weight: 700 }));
  parts.push(
    textEl(
      60,
      92,
      "概念模型：矩形=实体，菱形=联系（标注物理表名），椭圆=属性，下划线=主键；虚线框为跨分区的实体引用",
      { size: 13, fill: "#475569" },
    ),
  );

  const shapes = []; // 供重叠检查
  const conns = [];
  const labels = [];
  const drawn = [];

  for (const panel of chen.panels) {
    parts.push(
      `<rect x="${panel.x}" y="${panel.y}" width="${panel.w}" height="${panel.h}" rx="14" fill="#fbfdff" stroke="#e2e8f0"/>`,
    );
    parts.push(
      textEl(panel.x + 22, panel.y + 34, panel.title, { size: 18, weight: 700, fill: "#0f172a" }),
    );
    parts.push(textEl(panel.x + 22, panel.y + 56, panel.sub, { size: 11.5, fill: "#94a3b8" }));

    const entities = new Map();
    for (const e of panel.ents) {
      const s = schema[e.t];
      const size = entitySize(e.t);
      const cx = panel.x + e.x;
      const cy = panel.y + e.y;
      const box = { x: cx - size.w / 2, y: cy - size.h / 2, w: size.w, h: size.h };
      entities.set(e.t, { ...e, cx, cy, box, cl: CLUSTERS[s.cluster], label: s.label, table: e.t });
      shapes.push({ name: `实体 ${e.t}`, ...box });
    }
    const refs = new Map();
    for (const r of panel.refs ?? []) {
      const w = 170;
      const h = 56;
      const cx = panel.x + r.x;
      const cy = panel.y + r.y;
      const box = { x: cx - w / 2, y: cy - h / 2, w, h };
      refs.set(r.id, { ...r, cx, cy, box });
      shapes.push({ name: `引用 ${r.id}`, ...box });
    }
    const diamonds = new Map();
    for (const [i, d] of panel.diamonds.entries()) {
      const size = diamondSize(d.label, d.sub);
      const cx = panel.x + d.x;
      const cy = panel.y + d.y;
      diamonds.set(i, { ...d, cx, cy, ...size });
      shapes.push({
        name: `联系 ${d.label}`,
        x: cx - size.w / 2,
        y: cy - size.h / 2,
        w: size.w,
        h: size.h,
      });
    }

    // 属性椭圆
    const attrBoxes = [];
    for (const e of entities.values()) {
      const s = schema[e.table];
      const items = s.cols.map((c) => ({ name: c.n, pk: !!c.pk, fk: !!c.fk, m: !!c.m }));
      const step = ATTR_RY * 2 + ATTR_GAP;
      if (e.side === "l" || e.side === "r") {
        const n = items.length;
        const startY = e.cy - ((n - 1) * step) / 2;
        items.forEach((it, i) => {
          const rx = attrRx(it.name);
          const cx = e.side === "l" ? e.box.x - 34 - rx : e.box.x + e.box.w + 34 + rx;
          const cy = startY + i * step;
          const b = { x: cx - rx, y: cy - ATTR_RY, w: rx * 2, h: ATTR_RY * 2 };
          attrBoxes.push({ ...it, ...b, cx, cy, rx, owner: e });
          shapes.push({ name: `属性 ${e.table}.${it.name}`, ...b });
          conns.push({ x1: e.cx, y1: e.cy, x2: cx, y2: cy, color: e.cl.color });
        });
      } else {
        const sign = e.side === "u" ? -1 : 1;
        const perRow = 4;
        const rows = [];
        for (let i = 0; i < items.length; i += perRow) rows.push(items.slice(i, i + perRow));
        const rowStep = ATTR_RY * 2 + 16;
        rows.forEach((row, ri) => {
          const widths = row.map((it) => attrRx(it.name) * 2);
          const gap = 16;
          const total = widths.reduce((a, b) => a + b, 0) + gap * (row.length - 1);
          let x = e.cx - total / 2;
          const cy = e.cy + sign * (e.box.h / 2 + 40 + ri * rowStep);
          row.forEach((it, i) => {
            const rx = attrRx(it.name);
            const cx = x + rx;
            const b = { x: cx - rx, y: cy - ATTR_RY, w: rx * 2, h: ATTR_RY * 2 };
            attrBoxes.push({ ...it, ...b, cx, cy, rx, owner: e });
            shapes.push({ name: `属性 ${e.table}.${it.name}`, ...b });
            conns.push({ x1: e.cx, y1: e.cy, x2: cx, y2: cy, color: e.cl.color });
            x += rx * 2 + gap;
          });
        });
      }
    }

    // 联系连线
    for (const d of diamonds.values()) {
      for (const link of d.links) {
        let target;
        let tbox = null;
        if (link.pt) {
          target = { cx: panel.x + link.pt.x, cy: panel.y + link.pt.y };
        } else if (link.to.startsWith("ref:")) {
          const r = refs.get(link.to.slice(4));
          target = { cx: r.cx, cy: r.cy };
          tbox = r.box;
        } else {
          const e = entities.get(link.to);
          target = { cx: e.cx, cy: e.cy };
          tbox = e.box;
        }
        conns.push({
          x1: d.cx,
          y1: d.cy,
          x2: target.cx,
          y2: target.cy,
          color: "#334155",
          width: 1.7,
        });
        // 基数标注落在菱形侧，并避开目标实体框
        const dx = target.cx - d.cx;
        const dy = target.cy - d.cy;
        const dist = Math.hypot(dx, dy) || 1;
        const ux = dx / dist;
        const uy = dy / dist;
        let at = dist * 0.55;
        if (tbox) {
          // 目标框沿连线方向的半宽/半高，保证标注落在框外
          const tx = ux === 0 ? Infinity : tbox.w / 2 / Math.abs(ux);
          const ty = uy === 0 ? Infinity : tbox.h / 2 / Math.abs(uy);
          at = Math.min(at, dist - Math.min(tx, ty) - 16);
        }
        labels.push({ x: d.cx + ux * at, y: d.cy + uy * at - 7, s: link.card });
      }
      // 联系上的属性
      if (d.attrs?.length) {
        const side = d.attrSide ?? "d";
        const sign = side === "u" ? -1 : 1;
        const rowStep = ATTR_RY * 2 + 16;
        const perRow = 3;
        const rows = [];
        for (let i = 0; i < d.attrs.length; i += perRow) rows.push(d.attrs.slice(i, i + perRow));
        rows.forEach((row, ri) => {
          const widths = row.map((n) => attrRx(n) * 2);
          const gap = 16;
          const total = widths.reduce((a, b) => a + b, 0) + gap * (row.length - 1);
          let x = d.cx - total / 2;
          const cy = d.cy + sign * (d.h / 2 + 32 + ri * rowStep);
          row.forEach((n) => {
            const rx = attrRx(n);
            const cx = x + rx;
            const b = { x: cx - rx, y: cy - ATTR_RY, w: rx * 2, h: ATTR_RY * 2 };
            shapes.push({ name: `联系属性 ${d.label}.${n}`, ...b });
            conns.push({ x1: d.cx, y1: d.cy, x2: cx, y2: cy, color: "#334155" });
            attrBoxes.push({ name: n, pk: false, ...b, cx, cy, rx });
            x += rx * 2 + gap;
          });
        });
      }
    }

    drawn.push({ panel, entities, refs, diamonds, attrBoxes });
  }

  // 连线（先画，形状后画覆盖线头）
  for (const c of conns) {
    const d = `M ${c.x1} ${c.y1} L ${c.x2} ${c.y2}`;
    parts.push(`<path d="${d}" stroke="#ffffff" stroke-width="${(c.width ?? 1.3) + 4}"/>`);
    parts.push(
      `<path d="${d}" stroke="${c.color}" stroke-width="${c.width ?? 1.3}" opacity="0.85"/>`,
    );
  }

  // 形状
  for (const { panel, entities, refs, diamonds, attrBoxes } of drawn) {
    for (const e of entities.values()) {
      const cl = e.cl;
      parts.push(
        `<rect x="${e.box.x}" y="${e.box.y}" width="${e.box.w}" height="${e.box.h}" rx="7" fill="${cl.light}" stroke="${cl.color}" stroke-width="1.8"/>`,
      );
      parts.push(
        textEl(e.cx, e.cy - 4, e.table, {
          size: 15,
          weight: 700,
          mono: true,
          anchor: "middle",
          fill: cl.color,
        }),
      );
      parts.push(
        textEl(e.cx, e.cy + 16, schema[e.table].label, {
          size: 11.5,
          anchor: "middle",
          fill: "#475569",
        }),
      );
    }
    for (const r of refs.values()) {
      parts.push(
        `<rect x="${r.box.x}" y="${r.box.y}" width="${r.box.w}" height="${r.box.h}" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.4" stroke-dasharray="6 4"/>`,
      );
      parts.push(
        textEl(r.cx, r.cy - 2, r.id, {
          size: 13,
          weight: 600,
          mono: true,
          anchor: "middle",
          fill: "#475569",
        }),
      );
      parts.push(
        textEl(r.cx, r.cy + 16, schema[r.id].label + "（" + r.note + "）", {
          size: 10,
          anchor: "middle",
          fill: "#94a3b8",
        }),
      );
    }
    for (const d of diamonds.values()) {
      const pts = `${d.cx},${d.cy - d.h / 2} ${d.cx + d.w / 2},${d.cy} ${d.cx},${d.cy + d.h / 2} ${d.cx - d.w / 2},${d.cy}`;
      parts.push(`<polygon points="${pts}" fill="#ffffff" stroke="#334155" stroke-width="1.6"/>`);
      parts.push(
        textEl(d.cx, d.cy - 1, d.label, {
          size: 14,
          weight: 600,
          anchor: "middle",
          fill: "#1e293b",
        }),
      );
      if (d.sub)
        parts.push(
          textEl(d.cx, d.cy + 15, d.sub, {
            size: 8.6,
            mono: true,
            anchor: "middle",
            fill: "#94a3b8",
          }),
        );
      if (d.note)
        parts.push(
          textEl(d.cx + d.w / 2 + 6, d.cy - d.h / 2 - 2, d.note, { size: 9.5, fill: "#b45309" }),
        );
    }
    for (const a of attrBoxes) {
      parts.push(
        `<ellipse cx="${a.cx}" cy="${a.cy}" rx="${a.rx}" ry="${ATTR_RY}" fill="#ffffff" stroke="#94a3b8" stroke-width="1.1"/>`,
      );
      parts.push(
        textEl(a.cx, a.cy + 4, a.name, { size: 11, mono: true, anchor: "middle", fill: "#0f172a" }),
      );
      if (a.pk)
        parts.push(
          `<line x1="${a.cx - a.rx * 0.62}" y1="${a.cy + 7}" x2="${a.cx + a.rx * 0.62}" y2="${a.cy + 7}" stroke="#0f172a" stroke-width="1"/>`,
        );
      if (a.m) parts.push(`<circle cx="${a.x + 7}" cy="${a.y + 6}" r="2.4" fill="#f59e0b"/>`);
    }
    if (panel.id === "identity") {
      parts.push(
        textEl(
          panel.x + 22,
          panel.y + panel.h - 24,
          "身份卡文件仍是 prompt / 头像真源；relations.yml 已废弃，认知只存 role_relation",
          { size: 10.5, fill: "#94a3b8" },
        ),
      );
    }
    if (panel.id === "hotword") {
      parts.push(
        textEl(
          panel.x + 22,
          panel.y + panel.h - 24,
          "global_hotword 无归属对象，独立生效；role_hotword / library_hotword 的 role_id、category_id 均为应用层引用",
          { size: 10.5, fill: "#94a3b8" },
        ),
      );
    }
  }

  // 基数
  for (const l of labels) {
    parts.push(
      textEl(l.x, l.y, l.s, {
        size: 11,
        weight: 700,
        fill: "#1d4ed8",
        anchor: "middle",
        halo: true,
      }),
    );
  }

  // 图例（放在面板上方的空白带）
  const ly = 30;
  parts.push(
    `<rect x="3180" y="6" width="760" height="104" rx="10" fill="#f8fafc" stroke="#e2e8f0"/>`,
  );
  const legendRows = [
    ["rect", "实体（表）", "#2563eb"],
    ["diamond", "联系（菱形内为物理表名）", "#334155"],
    ["ellipse", "属性（下划线为主键）", "#94a3b8"],
    ["dashed", "跨分区引用的实体", "#94a3b8"],
  ];
  legendRows.forEach(([kind, label, color], i) => {
    const y = ly + i * 24;
    if (kind === "rect")
      parts.push(
        `<rect x="3200" y="${y - 12}" width="34" height="20" rx="3" fill="#eff6ff" stroke="${color}"/>`,
      );
    if (kind === "diamond")
      parts.push(
        `<polygon points="3217,${y - 13} 3236,${y - 2} 3217,${y + 9} 3198,${y - 2}" fill="#fff" stroke="${color}"/>`,
      );
    if (kind === "ellipse")
      parts.push(
        `<ellipse cx="3217" cy="${y - 2}" rx="20" ry="11" fill="#fff" stroke="${color}"/>`,
      );
    if (kind === "dashed")
      parts.push(
        `<rect x="3200" y="${y - 12}" width="34" height="20" rx="3" fill="#fff" stroke="${color}" stroke-dasharray="5 3"/>`,
      );
    parts.push(textEl(3250, y + 3, label, { size: 10.5, fill: "#334155" }));
  });

  parts.push(
    textEl(
      60,
      H - 26,
      "生成：docs/database/gen-er.mjs · Chen 图按功能分区，完整字段与约束见 er-crowfoot.svg",
      { size: 10.5, fill: "#94a3b8" },
    ),
  );

  return { svg: wrap(parts, W, H), W, H, shapes };
}

function wrap(parts, W, H) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<title>LingChat 数据库 ER 图</title>
<rect width="100%" height="100%" fill="#ffffff"/>
${parts.join("\n")}
</svg>`;
}

// ---------------------------------------------------------------- 执行与检查

const cf = renderCrowfoot();
fs.writeFileSync(path.join(OUT_DIR, "er-crowfoot.svg"), cf.svg);

const ch = renderChen();
fs.writeFileSync(path.join(OUT_DIR, "er-chen.svg"), ch.svg);

// 几何自检：表框重叠 / Chen 形状重叠
const overlaps = [];
const tbl = Object.keys(layout).map((t) => ({ name: t, ...layout[t], w: CW, h: boxH(t) }));
for (let i = 0; i < tbl.length; i++)
  for (let j = i + 1; j < tbl.length; j++) {
    const a = tbl[i];
    const b = tbl[j];
    if (a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h)
      overlaps.push(`[crowfoot] ${a.name} × ${b.name}`);
  }
// 鸦脚连线不应穿过无关表框
const near = (p, r) =>
  p.x >= r.x - 2 && p.x <= r.x + r.w + 2 && p.y >= r.y - 2 && p.y <= r.y + r.h + 2;
const inRect = (p, r) => p.x > r.x && p.x < r.x + r.w && p.y > r.y && p.y < r.y + r.h;
const segCross = (a, b, c, d) => {
  const o = (p, q, r) => Math.sign((q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x));
  return o(a, b, c) !== o(a, b, d) && o(c, d, a) !== o(c, d, b);
};
for (const e of edges) {
  const owners = tbl.filter((t) => near(e.pts[0], t) || near(e.pts[e.pts.length - 1], t));
  for (let i = 0; i < e.pts.length - 1; i++) {
    const a = e.pts[i];
    const b = e.pts[i + 1];
    for (const t of tbl) {
      if (owners.includes(t)) continue;
      const corners = [
        [
          { x: t.x, y: t.y },
          { x: t.x + t.w, y: t.y },
        ],
        [
          { x: t.x + t.w, y: t.y },
          { x: t.x + t.w, y: t.y + t.h },
        ],
        [
          { x: t.x + t.w, y: t.y + t.h },
          { x: t.x, y: t.y + t.h },
        ],
        [
          { x: t.x, y: t.y + t.h },
          { x: t.x, y: t.y },
        ],
      ];
      if (inRect(a, t) || inRect(b, t) || corners.some(([c, d]) => segCross(a, b, c, d))) {
        overlaps.push(`[crowfoot] 连线 ${e.label?.s ?? ""} 穿过 ${t.name}`);
        break;
      }
    }
  }
}

// 表内文字不得互相压盖（按字符数估算宽度）
for (const t of Object.keys(schema)) {
  const L = layout[t];
  for (const c of schema[t].cols) {
    let x = L.x + 16;
    if (c.pk) x += 22;
    if (c.fk) x += 22;
    if (c.u) x += 20;
    const nameW = c.n.length * 7.2;
    const typeStr = `${c.t}${c.n === "id" || !c.nn ? "?" : ""}${c.def ? ` = ${c.def}` : ""}`;
    const typeW = typeStr.length * 5.7;
    if (x + nameW + 8 > L.x + CW - 10 - typeW) overlaps.push(`[crowfoot] 列文字挤压 ${t}.${c.n}`);
  }
}

// Chen 形状必须落在某个分区内
for (const s of ch.shapes)
  if (
    !chen.panels.some(
      (p) => s.x >= p.x && s.y >= p.y && s.x + s.w <= p.x + p.w && s.y + s.h <= p.y + p.h,
    )
  )
    overlaps.push(`[chen] 超出分区 ${s.name}`);

const sh = ch.shapes;
for (let i = 0; i < sh.length; i++)
  for (let j = i + 1; j < sh.length; j++) {
    const a = sh[i];
    const b = sh[j];
    if (a.x < b.x + b.w - 1 && b.x < a.x + a.w - 1 && a.y < b.y + b.h - 1 && b.y < a.y + a.h - 1)
      overlaps.push(`[chen] ${a.name} × ${b.name}`);
  }

console.log(`er-crowfoot.svg  ${cf.W}×${cf.H}`);
console.log(`er-chen.svg      ${ch.W}×${ch.H}`);
if (overlaps.length) {
  console.log(`\n重叠 ${overlaps.length} 处：`);
  for (const o of overlaps.slice(0, 60)) console.log("  " + o);
} else {
  console.log("几何检查：无形状重叠");
}
