/**
 * schema 中文 → i18n 词条映射。
 *
 * Rust 端 schema.rs 返回的 label/hint/placeholder 是中文硬编码（单一真相源在
 * schema，作者工具链离不开它），这里在前端把显示文本词条化：词条存在用词条
 * （4 语言各自翻译），否则回落原文——复用 SettingsAdvanceOther 的 te()?t():原文
 * 先例，未翻译语言自动显示中文。
 *
 * 新增事件/字段时：schema.rs 加定义即可显示，无需动这里；只有想给它配翻译
 * 时才需要补词条 + 在此登记映射。
 */
import { i18n } from '@/locales'

/** schema 里字段的通用形状（取用前端的字段子集） */
export interface SchemaFieldLike {
  key: string
  label: string
  hint?: string
  placeholder?: string
  optionLabels?: string[]
  options?: (string | { value: string; label: string })[]
}

/** 词条存在用词条，否则回落原文 */
const text = (sub: string, fallback: string) =>
  i18n.global.te(`scriptEditor.schema.${sub}`)
    ? i18n.global.t(`scriptEditor.schema.${sub}`)
    : fallback

/** 事件类型 → 词条 key */
const EVENT_KEYS: Record<string, string> = {
  narration: 'event.narration',
  player: 'event.player',
  dialogue: 'event.dialogue',
  ai_dialogue: 'event.aiDialogue',
  free_dialogue: 'event.freeDialogue',
  choices: 'event.choices',
  input: 'event.input',
  set_variable: 'event.setVariable',
  chapter_end: 'event.chapterEnd',
  modify_character: 'event.modifyCharacter',
  background: 'event.background',
  background_effect: 'event.backgroundEffect',
  present_pic: 'event.presentPic',
  music: 'event.music',
  sound: 'event.sound',
  ambient: 'event.ambient',
  unlock_achievement: 'event.unlockAchievement',
}

/** 事件类型 label */
export const eventLabelOf = (spec: { typeKey: string; label: string }) =>
  text(EVENT_KEYS[spec.typeKey] ?? '', spec.label)

/** 分组（category 是中文值）→ 词条 key */
const CATEGORY_KEYS: Record<string, string> = {
  叙事: 'category.narrative',
  AI: 'category.ai',
  交互: 'category.interaction',
  流程: 'category.flow',
  演出: 'category.performance',
  声音: 'category.sound',
  成就: 'category.achievement',
}

/** 分组 label */
export const categoryLabelOf = (cat: string) => text(CATEGORY_KEYS[cat] ?? '', cat)

/**
 * 事件字段 label。歧义字段（text/displayName/prompt/hint/options/imagePath）
 * 用「typeKey.fieldKey」精确匹配，其余按 fieldKey 查通用表。
 */
const FIELD_KEYS: Record<string, string> = {
  'narration.text': 'field.textNarration',
  'narration.displayName': 'field.displayNameNarration',
  'player.text': 'field.textLine',
  'player.displayName': 'field.displayName',
  'dialogue.text': 'field.textLine',
  'dialogue.displayName': 'field.displayName',
  'ai_dialogue.prompt': 'field.aiPrompt',
  'free_dialogue.prompt': 'field.roundPrompt',
  'chapter_end.prompt': 'field.aiJudgePrompt',
  'free_dialogue.hint': 'field.inputHint',
  'input.hint': 'field.inputHint',
  'choices.options': 'field.optionsList',
  'set_variable.options': 'field.varOptions',
  'chapter_end.options': 'field.branchOptions',
  'background.imagePath': 'field.backgroundImage',
  'present_pic.imagePath': 'field.picImage',
  character: 'field.character',
  emotion: 'field.emotion',
  displaySubtitle: 'field.displaySubtitle',
  maxRounds: 'field.maxRounds',
  endLine: 'field.endLine',
  endPrompt: 'field.endPrompt',
  allowFree: 'field.allowFree',
  endType: 'field.endType',
  nextChapter: 'field.nextChapter',
  nextLegacy: 'field.nextLegacy',
  action: 'field.action',
  clothes: 'field.clothes',
  perceive: 'field.perceive',
  transition: 'field.transition',
  effect: 'field.effect',
  scale: 'field.scale',
  musicPath: 'field.musicPath',
  playbackSpeed: 'field.playbackSpeed',
  soundPath: 'field.soundPath',
  ambientPath: 'field.ambientPath',
  volume: 'field.volume',
  loop: 'field.loop',
  stop: 'field.stop',
  fade: 'field.fade',
  achievementId: 'field.achievementId',
  title: 'field.achievementTitle',
  description: 'field.achievementDesc',
  condition: 'field.condition',
  duration: 'field.duration',
}

const fieldKeyOf = (fieldKey: string, typeKey?: string) =>
  (typeKey ? FIELD_KEYS[`${typeKey}.${fieldKey}`] : undefined) ?? FIELD_KEYS[fieldKey]

/** 事件字段 label */
export const fieldLabelOf = (field: SchemaFieldLike, typeKey?: string) =>
  text(fieldKeyOf(field.key, typeKey) ?? '', field.label)

/** 字段 hint */
const HINT_KEYS: Record<string, string> = {
  character: 'hint.character',
  emotion: 'hint.emotion',
  'narration.text': 'hint.textNarration',
  'ai_dialogue.prompt': 'hint.aiPrompt',
  maxRounds: 'hint.maxRounds',
  endLine: 'hint.endLine',
  'choices.options': 'hint.optionsList',
  allowFree: 'hint.allowFree',
  'input.hint': 'hint.inputHint',
  'set_variable.options': 'hint.varOptions',
  endType: 'hint.endType',
  nextChapter: 'hint.nextChapter',
  'chapter_end.options': 'hint.branchOptions',
  'chapter_end.prompt': 'hint.aiJudgePrompt',
  nextLegacy: 'hint.nextLegacy',
  clothes: 'hint.clothes',
  perceive: 'hint.perceive',
  effect: 'hint.effect',
  playbackSpeed: 'hint.playbackSpeed',
  ambientPath: 'hint.ambientPath',
  volume: 'hint.volume',
  stop: 'hint.stop',
  achievementId: 'hint.achievementId',
  title: 'hint.achievementTitle',
  description: 'hint.achievementDesc',
  condition: 'hint.condition',
  duration: 'hint.duration',
}

const hintKeyOf = (fieldKey: string, typeKey?: string) =>
  (typeKey ? HINT_KEYS[`${typeKey}.${fieldKey}`] : undefined) ?? HINT_KEYS[fieldKey]

export const fieldHintOf = (field: SchemaFieldLike, typeKey?: string) =>
  field.hint ? text(hintKeyOf(field.key, typeKey) ?? '', field.hint) : ''

/** 字段 placeholder */
const PLACEHOLDER_KEYS: Record<string, string> = {
  'narration.displayName': 'placeholder.narrationDisplayName',
  'player.displayName': 'placeholder.playerDisplayName',
  'free_dialogue.hint': 'placeholder.freeDialogueHint',
  endLine: 'placeholder.endLine',
  'input.hint': 'placeholder.inputHint',
  achievementId: 'placeholder.achievementId',
  title: 'placeholder.achievementTitle',
  duration: 'placeholder.duration',
  recommandStart: 'placeholder.recommandStart',
}

const placeholderKeyOf = (fieldKey: string, typeKey?: string) =>
  (typeKey ? PLACEHOLDER_KEYS[`${typeKey}.${fieldKey}`] : undefined) ?? PLACEHOLDER_KEYS[fieldKey]

export const fieldPlaceholderOf = (field: SchemaFieldLike, typeKey?: string) =>
  field.placeholder ? text(placeholderKeyOf(field.key, typeKey) ?? '', field.placeholder) : ''

/**
 * select 选项 label：优先词条表（action 的 option_labels / end_type 的三个选项），
 * 否则回落 Rust 的 optionLabels/原文。
 */
const OPTION_KEYS: Record<string, string[]> = {
  'modify_character.action': ['option.showCharacter', 'option.hideCharacter'],
  'chapter_end.end_type': [
    'option.endTypeLinear',
    'option.endTypeBranching',
    'option.endTypeAiJudged',
  ],
}

export const optionLabelOf = (
  field: SchemaFieldLike,
  typeKey: string | undefined,
  optionValue: string,
  idx: number,
) => {
  const keys = typeKey ? OPTION_KEYS[`${typeKey}.${field.key}`] : undefined
  if (keys?.[idx]) return text(keys[idx], optionValue)
  return field.optionLabels?.[idx] ?? optionValue
}

/** 剧本设置（story_config）字段 label/hint */
const STORY_FIELD_KEYS: Record<string, string> = {
  script_name: 'field.scriptName',
  description: 'field.storyDescription',
  recommand_start: 'field.recommandStart',
  intro_chapter: 'field.introChapter',
}
const STORY_HINT_KEYS: Record<string, string> = {
  script_name: 'hint.scriptName',
  recommand_start: 'hint.recommandStart',
}

export const storyFieldLabelOf = (field: SchemaFieldLike) =>
  text(STORY_FIELD_KEYS[field.key] ?? '', field.label)

export const storyFieldHintOf = (field: SchemaFieldLike) =>
  field.hint ? text(STORY_HINT_KEYS[field.key] ?? '', field.hint) : ''

/** 解锁条件类型 label */
const UNLOCK_TYPE_KEYS: Record<string, string> = {
  chat_count: 'unlock.chatCount',
  time_range: 'unlock.timeRange',
  adventure_completed: 'unlock.adventureCompleted',
  achievement_unlocked: 'unlock.achievementUnlocked',
}

export const unlockTypeLabelOf = (spec: { typeKey: string; label: string }) =>
  text(UNLOCK_TYPE_KEYS[spec.typeKey] ?? '', spec.label)

/** 解锁条件字段 label / placeholder（无 placeholder 的字段回落 hint 映射） */
const UNLOCK_FIELD_KEYS: Record<string, string> = {
  threshold: 'field.threshold',
  start_hour: 'field.startHour',
  end_hour: 'field.endHour',
  adventure_folder: 'field.adventureFolder',
  achievement_id: 'field.unlockAchievementId',
}
const UNLOCK_HINT_KEYS: Record<string, string> = {
  start_hour: 'hint.startHour',
  adventure_folder: 'hint.adventureFolder',
}

export const unlockFieldLabelOf = (field: SchemaFieldLike) =>
  text(UNLOCK_FIELD_KEYS[field.key] ?? '', field.label)

export const unlockFieldPlaceholderOf = (field: SchemaFieldLike) => {
  if (field.placeholder) return text(UNLOCK_FIELD_KEYS[field.key] ?? '', field.placeholder)
  if (field.hint) return text(UNLOCK_HINT_KEYS[field.key] ?? '', field.hint)
  return ''
}

/** 情绪选项：EMOTION_CONFIG_EMO 的 key 即中文，映射到 settings 已有词条 */
const EMOTION_SLUGS: Record<string, string> = {
  兴奋: 'excited',
  厌恶: 'disgusted',
  哭泣: 'crying',
  害怕: 'scared',
  害羞: 'shy',
  平静: 'calm',
  心动: 'heartFlutter',
  惊讶: 'surprised',
  慌张: 'flustered',
  担心: 'worried',
  无奈: 'helpless',
  生气: 'angry',
  疑惑: 'confused',
  紧张: 'nervous',
  自信: 'confident',
  认真: 'serious',
  调皮: 'playful',
  难为情: 'embarrassed',
  高兴: 'happy',
  正常: 'normal',
}

export const emotionLabelOf = (emotion: string) => {
  const slug = EMOTION_SLUGS[emotion]
  return slug
    ? i18n.global.te(`settings.characterCreate.emotions.${slug}`)
      ? i18n.global.t(`settings.characterCreate.emotions.${slug}`)
      : emotion
    : emotion
}

/** 粒子特效选项（编辑器「背景特效」下拉，前端自有中文） */
const PARTICLE_KEYS: Record<string, string> = {
  None: 'particle.none',
  StarField: 'particle.starField',
  Rain: 'particle.rain',
  Sakura: 'particle.sakura',
  Snow: 'particle.snow',
  Fireworks: 'particle.fireworks',
}

export const particleLabelOf = (value: string, label: string) =>
  text(PARTICLE_KEYS[value] ?? '', label)
