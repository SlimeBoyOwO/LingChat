import type { GameLine } from '@/api/services/history'
import type { GameMessage } from '@/stores/modules/game/state'

export const convertToGameMessages = (lines: GameLine[]): GameMessage[] => {
  // 先过滤掉 SYSTEM 类型的消息（虽然后端已经过滤了，但再加一层保障）
  const filteredLines = lines.filter((line) => line.attribute !== 'SYSTEM')

  return filteredLines.map((line, index, array) => {
    const filteredContent = line.content.replace(/\{.*?\}/g, '').trim()

    const isLastMessage = index === array.length - 1
    const nextLine = isLastMessage ? null : array[index + 1]

    console.log('line.attribute', line.attribute)
    let isFinal = false
    if (line.attribute === 'assistant') {
      if (isLastMessage || nextLine?.attribute === 'user') {
        isFinal = true
      }
    }

    return {
      type: line.attribute === 'USER' ? 'message' : 'reply',
      displayName: line.display_name || '',
      content: filteredContent,
      emotion: line.predicted_emotion || undefined,
      audioFile: line.audio_file || undefined,
      isFinal: isFinal,
      motionText: line.action_content || undefined,
      originalTag: line.original_emotion || undefined,
      timestamp: Date.now(),
    }
  })
}
