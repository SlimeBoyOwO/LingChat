/**
 * 台词合并(连续短句同角色)共享状态。
 *
 * 跨组件共享「当前展示的台词是否仍打字/音频是否还在播」,供 event-queue.addEvent
 * 在**新 reply 到达时**判定是否合并;armed 标记后,MainChat 在展示完成时自动续打,
 * GameDialog 在消费时走「追加」而非「重置」路径。
 *
 * 喂入方:
 * - isTyping:       GameDialog 同步其打字机 isTyping
 * - isAudioPlaying: MainChat 同步 audio-started/audio-ended
 * 消费方:
 * - event-queue.addEvent 判定并置 armed / armedRoleId
 * - MainChat 完成信号 watch 触发自动续打
 * - GameDialog watch 读取 armed 决定追加
 */
import { reactive } from "vue";

export const dialogueMerge = reactive({
  /** 当前展示的台词是否仍在打字 */
  isTyping: false,
  /** 当前展示的台词是否仍在播放音频 */
  isAudioPlaying: false,
  /** 已武装:当前展示完成后自动推进,且下一条 reply 追加显示(而非重置) */
  armed: false,
  /** 武装时对应的角色,推进前防御校验队头事件是否就是它 */
  armedRoleId: -1,
  /**
   * 本次合并链已累计的台词字符长度(仅对话文本,不含动作段)。
   * 由 GameDialog 维护:全新台词时重置为当前句长度,追加合并时累加。
   * 判定合并时用「已累计长度 + 即将合并的下一句长度 ≤ mergeLineThreshold」——
   * 阈值约束的是合并后的总长度,而非每条句子独立。
   */
  mergedLength: 0,
});
