// U.N. Owen piano reference by DMBN: original melody, inner harmony and bass registers.
import score from "@/assets/minigames/twilight/un-owen-score.json";
import { createScoreSong } from "./score-song.js";

const base = createScoreSong(score);

export const beat = base.beat;
export const bpm = base.bpm;
export const bpmLabel = base.bpmLabel;
export const duration = base.duration;
export const beatAt = base.beatAt;
export const bpmAt = base.bpmAt;
export const beatPosition = base.beatPosition;
export const renderPcm = base.renderPcm;
export const makeChart = base.makeChart;

const sections = [
  { tick: 0, name: "SCARLET INTRO", energy: 0.35 },
  { tick: 38400, name: "U.N. OWEN", energy: 0.65 },
  { tick: 69120, name: "CRYSTAL WINGS", energy: 1 },
  { tick: 138240, name: "MIDNIGHT", energy: 0.65 },
  { tick: 176640, name: "SCARLET STORM", energy: 1 },
  { tick: 240000, name: "FINAL SPELL", energy: 1 },
  { tick: 301440, name: "RITARDANDO", energy: 0.25 },
];
export function sectionAt(time) {
  return (
    [...sections]
      .reverse()
      .find((section) => time >= base.countIn + base.secondsAt(section.tick)) ?? sections[0]
  );
}
