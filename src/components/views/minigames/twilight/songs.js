import * as lantern from "./music.js";
import * as neon from "./neon.js";
import * as flandre from "./flandre.js";

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
