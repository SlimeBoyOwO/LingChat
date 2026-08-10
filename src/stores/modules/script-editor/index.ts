import { defineStore } from 'pinia'
import { useEditorState } from './state'
import { useEditorGetters } from './getters'
import { useEditorActions } from './actions'

/**
 * 剧本编辑器 store（setup 风格）。
 *
 * 与 game store 的 option 风格不同：script-editor 的 action 大量互调兄弟方法，
 * option 风格里拆出 actions 会丢 this 类型（已验证失败），故整体转 setup——无 this，
 * 状态是 ref、getter 是 computed、action 是普通函数，源文件如实拆分，类型全自洽。
 *
 * 外部消费方 API 不变：store.schema / store.scriptKey / store.init() 等照常。
 */
export const useScriptEditorStore = defineStore(
  'script-editor',
  () => {
    const s = useEditorState()
    const g = useEditorGetters(s)
    const a = useEditorActions(s, g)
    return { ...s, ...g, ...a }
  },
  {
    // 只持久化 UI 偏好。用白名单式的 exclude 很容易漏 —— 新增 state 字段会
    // 默认被持久化，与「正文绝对不进 localStorage」的意图相反。这里把所有
    // 非偏好字段都显式列出来，新增字段时务必同步。
    // （persist 插件每次 mutation 都全量 JSON.stringify 且无防抖。）
    persist: {
      key: 'lingchat-script-editor-ui',
      exclude: [
        'schema',
        'scripts',
        'loading',
        'detail',
        'globalAssets',
        'chapter',
        'selectedEvent',
        'dirty',
        'saving',
        'lastSavedAt',
        'undoStack',
        'redoStack',
        'report',
        'previewing',
        'previewGeneration',
        'readiness',
        'globalCharacters',
        'assetFiles',
        'bgVersion',
        'globalBgFiles',
        'propsExpanded',
      ],
    },
  },
)
