export interface DecodedVoice {
  buffer: AudioBuffer
}

export async function decodeVoiceForLipSync(dataUrl: string): Promise<DecodedVoice | null> {
  if (!dataUrl || !dataUrl.startsWith('data:audio/')) return null
  const context = new OfflineAudioContext(1, 1, 44100)
  try {
    const encoded = await fetch(dataUrl).then((response) => response.arrayBuffer())
    const buffer = await context.decodeAudioData(encoded.slice(0))
    return { buffer }
  } catch (error) {
    console.warn('[Live2D] Voice decoding failed; lip sync disabled for this line', error)
    return null
  }
}

export function sampleVoiceAmplitude(decoded: DecodedVoice | null, currentTime: number): number {
  if (!decoded || currentTime < 0 || currentTime >= decoded.buffer.duration) return 0
  const buffer = decoded.buffer
  const center = Math.floor(currentTime * buffer.sampleRate)
  const radius = Math.max(1, Math.floor(buffer.sampleRate * 0.012))
  const start = Math.max(0, center - radius)
  const end = Math.min(buffer.length, center + radius)
  if (end <= start) return 0

  let sum = 0
  let samples = 0
  for (let channel = 0; channel < buffer.numberOfChannels; channel += 1) {
    const data = buffer.getChannelData(channel)
    for (let index = start; index < end; index += 4) {
      sum += data[index] * data[index]
      samples += 1
    }
  }
  if (!samples) return 0
  return Math.min(1, Math.sqrt(sum / samples) * 4.5)
}
