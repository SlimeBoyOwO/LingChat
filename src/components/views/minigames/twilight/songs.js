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

let midiImportCount = 0;
// 运行时导入本地 MIDI：解析成 score 后复用 score-song 的谱面/合成管线，仅本次会话保留
export function createMidiSong(fileName, buffer) {
  const score = parseMidiScore(buffer);
  const base = createScoreSong(score);
  const song = {
    ...base,
    imported: true,
    id: `midi-import-${++midiImportCount}`,
    title: fileName.replace(/\.[^.]+$/, "") || "导入曲目",
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
