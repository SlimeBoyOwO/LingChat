/**
 * ASR 音频工具：webm/opus Blob → 16kHz mono PCM16 WAV 字节。
 *
 * useAsrInput（录音 → 识别）与 SettingsAsr（测试连接）共用。
 */

/** WAV header 写入（16k mono PCM16，44 字节标准头） */
function writeAscii(view: DataView, offset: number, s: string) {
  for (let i = 0; i < s.length; i++) {
    view.setUint8(offset + i, s.charCodeAt(i));
  }
}

/** f32 PCM 数组 → 16kHz mono PCM16 WAV bytes */
export function pcmToWavPcm16(pcm: number[]): Uint8Array {
  const targetRate = 16000;
  const pcm16 = new Int16Array(pcm.length);
  for (let i = 0; i < pcm.length; i++) {
    const s = Math.max(-1, Math.min(1, pcm[i]));
    pcm16[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
  }
  const header = new ArrayBuffer(44);
  const view = new DataView(header);
  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + pcm16.byteLength, true);
  writeAscii(view, 8, "WAVE");
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true); // fmt chunk size
  view.setUint16(20, 1, true); // PCM
  view.setUint16(22, 1, true); // mono
  view.setUint32(24, targetRate, true);
  view.setUint32(28, targetRate * 2, true);
  view.setUint16(32, 2, true); // block align
  view.setUint16(34, 16, true); // bits per sample
  writeAscii(view, 36, "data");
  view.setUint32(40, pcm16.byteLength, true);
  const out = new Uint8Array(header.byteLength + pcm16.byteLength);
  out.set(new Uint8Array(header), 0);
  out.set(new Uint8Array(pcm16.buffer), header.byteLength);
  return out;
}

/** webm/opus blob → 16kHz mono PCM16 WAV bytes（解码 + 重采样） */
export async function webmToWavPcm16Mono16k(blob: Blob): Promise<Uint8Array> {
  const arrayBuffer = await blob.arrayBuffer();
  const audioCtx = new (
    window.AudioContext ||
    (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext
  )();
  const audioBuffer = await audioCtx.decodeAudioData(arrayBuffer);
  await audioCtx.close();

  const targetRate = 16000;
  const numSamples = Math.ceil(audioBuffer.duration * targetRate);
  const offlineCtx = new OfflineAudioContext(1, numSamples, targetRate);
  const source = offlineCtx.createBufferSource();
  source.buffer = audioBuffer;
  source.connect(offlineCtx.destination);
  source.start();
  const rendered = await offlineCtx.startRendering();
  const channelData = rendered.getChannelData(0);

  const pcm: number[] = new Array(channelData.length);
  for (let i = 0; i < channelData.length; i++) {
    pcm[i] = channelData[i];
  }
  return pcmToWavPcm16(pcm);
}

/**
 * 裁剪首尾静音：找第一个/最后一个能量帧，前后各留 padMs 缓冲。
 * 录音从能量触发开始、VAD 停顿结束，头尾通常带环境声/静音尾巴；
 * 裁剪后发送给 ASR 的只含语音段（更准、更快、更省 token）。
 */
export function trimSilencePcm(
  pcm: number[],
  sampleRate = 16000,
  frameMs = 30,
  threshold = 0.005,
  padMs = 200
): number[] {
  const frame = Math.floor((sampleRate * frameMs) / 1000); // 480 samples @16k
  let startSample = -1;
  let endSample = -1;
  for (let i = 0; i + frame <= pcm.length; i += frame) {
    let sum = 0;
    for (let j = 0; j < frame; j++) {
      sum += pcm[i + j] * pcm[i + j];
    }
    const rms = Math.sqrt(sum / frame);
    if (rms > threshold) {
      if (startSample === -1) startSample = i;
      endSample = i + frame;
    }
  }
  if (startSample === -1) return []; // 全静音
  const pad = Math.floor((sampleRate * padMs) / 1000);
  const start = Math.max(0, startSample - pad);
  const end = Math.min(pcm.length, endSample + pad);
  return pcm.slice(start, end);
}
