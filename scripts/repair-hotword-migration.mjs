#!/usr/bin/env node
// 一次性修复脚本：热词三表合一后，旧 dev 库里仍记着六条已删除的迁移，
// sea-orm 启动对账会报 "Migration file ... is missing" 并拒绝启动。
//
// 本脚本删掉这些账本记录、丢弃旧热词表，之后正常启动应用即可由新迁移建表。
// 热词未发版，旧表里的数据直接丢弃，不做搬运。
//
// 用法：node scripts/repair-hotword-migration.mjs [数据库路径]
// 默认路径：<仓库根>/data/game_database.db

import { existsSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { DatabaseSync } from "node:sqlite";

const REMOVED_VERSIONS = [
  "m20261004_000001_create_role_hotword",
  "m20261005_000001_create_global_hotword",
  "m20261009_000001_create_hotword_library",
  "m20261010_000001_drop_hotword_lang",
  "m20261012_000001_normalize_hotword_weight",
  "m20261023_000001_library_hotword_unique_per_category",
];

const LEGACY_TABLES = ["library_hotword", "role_hotword", "global_hotword", "hotword_category"];

const dbPath = process.argv[2] ?? path.join(process.cwd(), "data", "game_database.db");
if (!existsSync(dbPath)) {
  console.error(`数据库不存在：${dbPath}`);
  process.exit(1);
}

const db = new DatabaseSync(dbPath);
try {
  const placeholders = REMOVED_VERSIONS.map(() => "?").join(", ");
  db.exec("BEGIN");
  const removed = db
    .prepare(`DELETE FROM seaql_migrations WHERE version IN (${placeholders})`)
    .run(...REMOVED_VERSIONS);
  for (const table of LEGACY_TABLES) {
    db.exec(`DROP TABLE IF EXISTS "${table}"`);
  }
  db.exec("COMMIT");
  console.log(`已修复 ${dbPath}`);
  console.log(`  删除旧迁移记录：${removed.changes} 条`);
  console.log(`  丢弃旧热词表：${LEGACY_TABLES.join(", ")}`);
  console.log("重新启动应用即可，新迁移会建出 hotword 与 hotword_category。");
} catch (error) {
  db.exec("ROLLBACK");
  console.error("修复失败，数据库未改动：", error);
  process.exitCode = 1;
} finally {
  db.close();
}
