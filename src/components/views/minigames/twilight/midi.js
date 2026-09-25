// Minimal Standard MIDI File (SMF) reader for user-imported songs.
// Supports format 0/1 files with a PPQ division. Channel 10 (drums) is skipped.
// Voice split: notes are grouped by (track, channel) and sorted by average pitch —
// the top voice provides the melody (highest note at each tick) and inner harmony,
// the bottom voice plays bass, and any remaining voices join the harmony.
export function parseMidiScore(buffer) {
  const view = new DataView(buffer);
  const fail = (message) => {
    throw new Error(message);
  };
  if (view.byteLength < 14 || view.getUint32(0) !== 0x4d546864) fail("不是有效的 MIDI 文件");
  const format = view.getUint16(8),
    trackCount = view.getUint16(10),
    ppq = view.getUint16(12);
  if (format === 2) fail("不支持 Format 2 多曲 MIDI 文件");
  if (ppq & 0x8000 || ppq === 0) fail("不支持 SMPTE 时间码格式的 MIDI 文件");

  const tempos = new Map(),
    meters = new Map(),
    voiceNotes = new Map();
  let lastNoteEnd = 0,
    firstNoteStart = Infinity;
  let offset = 8 + view.getUint32(4);
  for (let trackIndex = 0; trackIndex < trackCount && offset < view.byteLength; trackIndex++) {
    if (view.getUint32(offset) !== 0x4d54726b) fail("MIDI 轨道块损坏");
    let cursor = offset + 8;
    const trackEnd = cursor + view.getUint32(offset + 4);
    offset = trackEnd;
    let tick = 0,
      status = 0;
    const active = new Map();
    const readVarInt = () => {
      let value = 0,
        byte;
      do {
        byte = view.getUint8(cursor++);
        value = (value << 7) | (byte & 0x7f);
      } while (byte & 0x80);
      return value;
    };
    const closeNote = (key, end) => {
      const queue = active.get(key);
      if (!queue?.length) return;
      const [start, velocity] = queue.shift();
      if (end <= start) return;
      const voiceKey = key >> 8; // 声部按 (轨道, 通道) 分组，去掉音高位
      if (!voiceNotes.has(voiceKey)) voiceNotes.set(voiceKey, []);
      voiceNotes.get(voiceKey).push([start, end - start, key & 0xff, velocity]);
      lastNoteEnd = Math.max(lastNoteEnd, end);
      firstNoteStart = Math.min(firstNoteStart, start);
    };
    while (cursor < trackEnd) {
      tick += readVarInt();
      const head = view.getUint8(cursor);
      if (head & 0x80) {
        status = head;
        cursor++;
      }
      if (status === 0xff) {
        const type = view.getUint8(cursor++);
        const length = readVarInt();
        if (type === 0x51 && length === 3)
          tempos.set(tick, (view.getUint8(cursor) << 16) | view.getUint16(cursor + 1));
        else if (type === 0x58 && length >= 4)
          meters.set(tick, [view.getUint8(cursor), 1 << view.getUint8(cursor + 1)]);
        cursor += length;
        status = 0; // meta 事件会中断 running status
        if (type === 0x2f) break;
      } else if (status === 0xf0 || status === 0xf7) {
        cursor += readVarInt();
        status = 0; // sysex 事件同样中断 running status
      } else {
        const kind = status & 0xf0,
          channel = status & 0x0f;
        if (kind === 0x90 || kind === 0x80) {
          const note = view.getUint8(cursor++),
            velocity = view.getUint8(cursor++);
          if (channel !== 9) {
            const key = (trackIndex << 12) | (channel << 8) | note;
            if (kind === 0x90 && velocity > 0) {
              if (!active.has(key)) active.set(key, []);
              active.get(key).push([tick, velocity]);
            } else closeNote(key, tick);
          }
        } else if (kind === 0xc0 || kind === 0xd0) cursor += 1;
        else cursor += 2;
      }
    }
    // 宽容处理未闭合的音符：收到轨道末尾为止
    for (const key of [...active.keys()]) while (active.get(key).length) closeNote(key, tick);
  }
  const voices = [...voiceNotes.values()]
    .filter((notes) => notes.length)
    .map((notes) => ({
      notes,
      average: notes.reduce((sum, note) => sum + note[2], 0) / notes.length,
    }))
    .sort((a, b) => b.average - a.average);
  if (!voices.length) fail("没有找到可用的音符（鼓通道会被忽略）");

  const melody = [],
    harmony = [],
    bass = [];
  const splitTopVoice = (notes) => {
    const groups = new Map();
    for (const note of notes) {
      if (!groups.has(note[0])) groups.set(note[0], []);
      groups.get(note[0]).push(note);
    }
    for (const group of groups.values()) {
      group.sort((a, b) => b[2] - a[2]);
      melody.push(group[0]);
      for (const note of group.slice(1)) harmony.push(note);
    }
  };
  if (voices.length === 1) {
    // 单声部：最高音为旋律；三音以上的同时按下把最低音交给低音，保住小节重音
    const groups = new Map();
    for (const note of voices[0].notes) {
      if (!groups.has(note[0])) groups.set(note[0], []);
      groups.get(note[0]).push(note);
    }
    for (const group of groups.values()) {
      group.sort((a, b) => b[2] - a[2]);
      melody.push(group[0]);
      if (group.length > 2) bass.push(group.at(-1));
      for (const note of group.slice(1, group.length > 2 ? -1 : undefined)) harmony.push(note);
    }
  } else {
    splitTopVoice(voices[0].notes);
    for (const voice of voices.slice(1, -1)) harmony.push(...voice.notes);
    bass.push(...voices.at(-1).notes);
  }
  melody.sort((a, b) => a[0] - b[0]);
  for (const [index, note] of melody.entries()) {
    const following = melody[index + 1];
    if (following) note[1] = Math.min(note[1], following[0] - note[0]);
  }

  const timeline = (events, fallback) => {
    let initialTick = -1;
    for (const tick of events.keys())
      if (tick <= firstNoteStart && tick > initialTick) initialTick = tick;
    const result = [[0, initialTick >= 0 ? events.get(initialTick) : fallback]];
    for (const [tick, value] of [...events.entries()].sort((a, b) => a[0] - b[0]))
      if (tick > firstNoteStart) result.push([tick - firstNoteStart, value]);
    return result;
  };
  const shift = ([tick, length, pitch, velocity]) => [
    tick - firstNoteStart,
    length,
    pitch,
    velocity,
  ];

  return {
    ppq,
    trimmedTicks: firstNoteStart,
    endTick: lastNoteEnd - firstNoteStart,
    tempos: timeline(tempos, 500000),
    meters: timeline(meters, [4, 4]).map(([tick, meter]) => [tick, meter[0], meter[1]]),
    melody: melody.map(shift),
    harmony: harmony.sort((a, b) => a[0] - b[0]).map(shift),
    bass: bass.sort((a, b) => a[0] - b[0]).map(shift),
  };
}
