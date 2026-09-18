import * as lantern from "./music.js";
import * as neon from "./neon.js";
import * as flandre from "./flandre.js";
import { parseMidiScore } from "./midi.js";
import { createScoreSong } from "./score-song.js";

export const SONGS = [
  {
    ...lantern,
    id: "lantern-echo",
    title: "灯下回声",
    difficulty: "入门 3",
    style: "CHIPTUNE",
    neon: false,
  },
  {
    ...neon,
    id: "neon-overdrive",
    title: "霓虹过载",
    difficulty: "专家 9",
    style: "ELECTRO",
    neon: true,
  },
  {
    ...flandre,
    id: "flandre-scarlet",
    title: "U.N.オーエンは彼女なのか？",
    difficulty: "进阶 7",
    style: "TOUHOU / MELODY",
    credit: "东方Project 二次创作 · 原曲 ZUN · 钢琴谱 DMBN / 東方ピアノEasyモード",
    neon: true,
  },
].map((song) => ({ ...song, noteCount: song.makeChart().length }));

function songFromScore(id, title, score) {
  const base = createScoreSong(score);
  const song = {
    ...base,
    imported: true,
    id,
    title,
    style: "MIDI IMPORT",
    neon: false,
  };
  const noteCount = song.makeChart().length;
  const density = noteCount / Math.max(1, song.duration);
  const level = Math.max(1, Math.min(10, Math.round(density * 1.4)));
  song.difficulty = `${level <= 3 ? "入门" : level <= 6 ? "进阶" : "专家"} ${level}`;
  song.noteCount = noteCount;
  return song;
}

// 运行时导入本地 MIDI：解析成 score 后复用 score-song 的谱面/合成管线
export function createMidiSong(fileName, buffer) {
  return songFromScore(
    `midi-import-${Date.now().toString(36)}`,
    fileName.replace(/\.[^.]+$/, "") || "导入曲目",
    parseMidiScore(buffer),
  );
}

// 导入曲目持久化到 localStorage（仅主线程调用；Worker 导入本模块时不会触发）
const IMPORTED_STORAGE_KEY = "twilight-imported-songs";

export function loadImportedSongs() {
  try {
    const saved = JSON.parse(localStorage.getItem(IMPORTED_STORAGE_KEY) ?? "[]");
    if (!Array.isArray(saved)) return 0;
    let count = 0;
    for (const entry of saved) {
      if (!entry?.id || !entry?.title || !entry?.score?.melody) continue;
      if (SONGS.some((song) => song.id === entry.id)) continue; // 重复挂载时幂等
      SONGS.push(songFromScore(entry.id, entry.title, entry.score));
      count++;
    }
    return count;
  } catch {
    return 0;
  }
}

export function saveImportedSongs() {
  try {
    localStorage.setItem(
      IMPORTED_STORAGE_KEY,
      JSON.stringify(
        SONGS.filter((song) => song.imported).map((song) => ({
          id: song.id,
          title: song.title,
          score: song.score,
        })),
      ),
    );
    return true;
  } catch (error) {
    console.warn("导入曲目保存失败:", error);
    return false;
  }
}
