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
    title: "真っ黒フランドール S",
    difficulty: "极限 12",
    style: "TOUHOU / MIDI",
    neon: true,
  },
].map((song) => ({ ...song, noteCount: song.makeChart().length }));
