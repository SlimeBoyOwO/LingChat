import { SONGS } from "./songs.js";

self.onmessage = ({ data }) => {
  try {
    const song = SONGS.find((entry) => entry.id === data.songId);
    if (!song) throw new Error("Unknown song");
    const pcm = song.renderPcm(data.sampleRate);
    self.postMessage({ pcm }, [pcm.buffer]);
  } catch (error) {
    self.postMessage({ error: String(error) });
  }
};
