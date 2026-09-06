import * as lantern from "./music.js";
import * as neon from "./neon.js";

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
].map((song) => ({ ...song, noteCount: song.makeChart().length }));
