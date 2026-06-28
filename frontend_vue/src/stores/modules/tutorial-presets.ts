/**
 * 新手教程预设注册表
 *
 * 集中管理所有教程预设，方便扩展。
 * 调用 registerTutorialPreset() 即可注册新的教程流程。
 */
import type { TutorialPreset } from '../../types/tutorial'

/**
 * 已注册的教程预设（key = presetId）
 */
export const REGISTERED_PRESETS: Record<string, TutorialPreset> = {}

/**
 * 注册一个教程预设
 */
export function registerTutorialPreset(preset: TutorialPreset): void {
  REGISTERED_PRESETS[preset.id] = preset
}

// ============================================================
// 首期新手引导预设：「onboarding」
//
// 流程设计：
//   每个配置步骤都是"提示 → 点击下一步 → 面板打开/教程隐藏 → 用户操作 → 关闭面板 → 继续"
//
// 关键字段：
//   - nextAction：点击"下一步"时触发的操作（不是进入步骤时）
//   - waitForField：隐藏教程后，等待此 store 字段变为 false 才推进到下一步
// ============================================================

registerTutorialPreset({
  id: 'onboarding',
  name: '新手引导',
  steps: [
    {
      id: 'welcome',
      title: '欢迎来到 LingChat',
      content: '欢迎来到 LingChat！来快速了解如何开始吧。',
      tooltipPlacement: 'center',
      skippable: true,
      allowBack: false,
    },
    {
      id: 'llm-config-info',
      title: '配置 AI 大模型',
      content:
        '首先，我们需要配置 AI 大模型。\n\n点击"下一步"打开大模型管理面板，\n填写你的 API Key 和模型信息。',
      tooltipPlacement: 'bottom',
      nextAction: { type: 'openLlmConfig' },
      waitForField: 'showLlmConfig',
      skippable: true,
      allowBack: true,
    },
    {
      id: 'character-info',
      title: '选择聊天角色',
      content: '接下来，选择一个你喜欢的角色来开始对话。\n\n点击"下一步"打开角色设置面板。',
      tooltipPlacement: 'bottom',
      nextAction: { type: 'switchSettingsTab', payload: { tab: 'character' } },
      waitForField: 'showSettings',
      skippable: true,
      allowBack: true,
    },
    {
      id: 'chat',
      title: '开始对话',
      content: '一切就绪！点击"开始游戏"，\n即可进入聊天界面开始与 AI 角色对话。',
      tooltipPlacement: 'center',
      action: { type: 'closeSettings' },
      skippable: true,
      allowBack: true,
    },
    {
      id: 'complete',
      title: '引导完成',
      content:
        '你已经掌握了基本操作！\n现在尽情探索 LingChat 的更多功能吧。\n\n提示：你可以随时在设置中调整各种选项。',
      tooltipPlacement: 'center',
      skippable: false,
      allowBack: true,
      autoAdvanceMs: 5000,
    },
  ],
})
