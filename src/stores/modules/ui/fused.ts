import { defineStore } from 'pinia'

/** 等待打字机续打的一段台词(合并段追加当前台词,独立段开新台词) */
export interface FusedSegment {
  key: number
  text: string
  motionText: string
  emotion: string
  originalTag: string
  /** 该段 TTS 音频文件(续打时切换播放) */
  audioFile?: string
  /** 触发此回复的用户消息序号(透传给历史回溯) */
  userMessageSeq?: number
  /** 发送者角色 ID */
  roleId: number
  /**
   * merge=true:追加到当前台词(continueFused 归档上一段后拼接);
   * merge=false:独立成句/新台词起点(continueFused 清空静态缓冲再打)。
   */
  merge: boolean
}

interface FusedState {
  /** 语音是否播放完毕(由 MainChat/PetMode 的媒体检测同步到此) */
  audioFinished: boolean
  /** 当前台词累积字数(阈值判断) */
  accumulatedTextLength: number
  /** 最后发送者角色 ID(合并判定第一关) */
  lastSenderRoleId: number
  /**
   * 打断 latch(模拟"媒体未完成"信号):
   * 用户点击 → 置 true → 紧接着下一条段判定为独立成句(不合并),
   * 判定处通过 consumeInterrupted() 消费清除(一次性)。
   * 之后段若满足条件仍可继续合并(点击只是快进,融合不退出)。
   */
  interrupted: boolean
  /**
   * 待打字机续打的段(合并段/独立段都入队,按到达顺序先进先出) */
  pendingSegments: FusedSegment[]
  /**
   * 最近入队段是否为合并段(消费无关信号)。
   * isFinal 并入判定:不只看 pendingCount(异步消费,段可能已被打完而队列空),
   * 也看本标志 —— 上一段以合并方式进入即视为当前台词
   * 仍在融合展示中。
   * pushPending 时按段 merge 属性设置;独立段/首段/新台词起点置 false。
   */
  lastSegmentMerged: boolean
  /**
   * pending 代数:discardPending(角色切换/新回合)或 reset(本轮结束)时自增,
   * 用于作废已排队的段间延迟回调(延迟中角色已切换则丢弃,
   * 不再打出旧角色段)。
   */
  pendingEpoch: number
  /**
   * 已完成段的对话区静态 HTML(白字,直接拼接),跨视图(主聊↔桌宠)共享。
   * 与 staticMotionHtml/curText/curMotion 一起描述"当前台词"的完整展示状态:
   * 渲染状态在 store,主聊/桌宠切换时新组件可从 store 恢复,
   * 回复不中断、内容不丢。
   */
  staticTextHtml: string
  /** 已完成段的动作区静态 HTML(灰字,直接拼接) */
  staticMotionHtml: string
  /** 当前正在打字的段完整文本(打字完成后归档进 staticTextHtml) */
  curText: string
  /** 当前段的动作文本(归档进 staticMotionHtml) */
  curMotion: string
}

let seq = 0

export const useFusedStore = defineStore('fusedDialogue', {
  state: (): FusedState => ({
    audioFinished: true,
    accumulatedTextLength: 0,
    lastSenderRoleId: -1,
    interrupted: false,
    pendingSegments: [],
    lastSegmentMerged: false,
    pendingEpoch: 0,
    staticTextHtml: '',
    staticMotionHtml: '',
    curText: '',
    curMotion: '',
  }),

  getters: {
    /** 待打段数量(供 GameDialog 检查续打) */
    pendingCount(state) {
      return state.pendingSegments.length
    },
  },

  actions: {
    /**
     * 新台词起点:重置累积为当前段长度(角色切换/独立长句/新一轮首段)。
     * 之后新段再判定合并时,以该段为基线累加。
     */
    resetAccumulation(roleId: number, textLength: number) {
      this.lastSenderRoleId = roleId
      this.accumulatedTextLength = textLength
    },

    /** 累积合并段字数(合并判定约束即将显示的总量) */
    accumulate(roleId: number, textLength: number) {
      this.lastSenderRoleId = roleId
      this.accumulatedTextLength += textLength
    },

    /** 用户点击:置打断 latch(紧接着下一条段独立成句) */
    markInterrupted() {
      this.interrupted = true
    },

    /** 消费打断 latch:独立成句判定处调用,清除后后续段恢复合并 */
    consumeInterrupted() {
      this.interrupted = false
    },

    /** 入队待打段(合并段不触发新帧打字,由打字机续打消费) */
    pushPending(segment: Omit<FusedSegment, 'key'>) {
      this.pendingSegments.push({ ...segment, key: ++seq })
      // 最近段标记跟随队尾段:独立段(false)会切断 isFinal 并入链
      this.lastSegmentMerged = segment.merge
    },

    /** 新台词起点(首段/角色切换):最近段不再是合并段,isFinal 不再并入 */
    resetLastSegment() {
      this.lastSegmentMerged = false
    },

    /** 取出队首待打段(打字机当前段完成后调用) */
    shiftPending(): FusedSegment | undefined {
      return this.pendingSegments.shift()
    },

    /**
     * 视图卸载时把段间呼吸延迟中(已 shift 未开打)的段放回队首,
     * 恢复未消费状态——否则该段随旧组件销毁永久丢失(切换断句)。
     * 延迟回调消费后置 null,由新视图的 onMounted 主动续打。
     */
    restoreDeferred(segment: FusedSegment) {
      this.pendingSegments.unshift(segment)
    },

    /**
     * 角色切换/新回合:丢弃 pending 里上一角色的未消费段。
     * 历史已由 appendGameMessage 逐句保存,仅弃展示 ——
     * 否则旧角色剩余段会在新角色台词播放中被续打出来(内容/标题错乱)。
     * 自增 pendingEpoch 作废已排队的段间延迟回调。
     */
    discardPending() {
      this.pendingSegments = []
      this.pendingEpoch++
    },

    /** 本轮结束/模式切换时清理全部融合状态 */
    reset() {
      this.audioFinished = true
      this.accumulatedTextLength = 0
      this.lastSenderRoleId = -1
      this.interrupted = false
      this.pendingSegments = []
      this.lastSegmentMerged = false
      this.pendingEpoch++
      this.staticTextHtml = ''
      this.staticMotionHtml = ''
      this.curText = ''
      this.curMotion = ''
    },
  },
})
