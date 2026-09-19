import { SONGS } from "./songs.js";
import { createScoreSong } from "./score-song.js";

self.onmessage = ({ data }) => {
  try {
    // 运行时导入的 MIDI 曲目不在内置清单里，由主线程随消息附上 score 数据
    const song = data.score
      ? createScoreSong(data.score)
      : SONGS.find((entry) => entry.id === data.songId);
    if (!song) throw new Error("Unknown song");
    const pcm = song.renderPcm(data.sampleRate);
    self.postMessage({ pcm }, [pcm.buffer]);
  } catch (error) {
    self.postMessage({ error: String(error) });
  }
};
