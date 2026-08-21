<template>
  <StartList>
    <StartLine v-for="(script, index) in currentPageScripts">
      <StartItem
        :key="script.script_name"
        @click="selectScript(script)"
      >
        {{ script.script_name }}
      </StartItem>
      <!-- 声明了跨局记忆变量（persistent_vars）的剧本提供一键重置，回到第一周目 -->
      <button
        v-if="script.has_persistent_vars"
        :key="`${script.script_name}-reset`"
        class="reset-memory-btn"
        type="button"
        :disabled="resettingName !== null"
        @click.stop="resetMemory(script)"
      >
        {{ resetDoneName === script.script_name ? `✓ ${$t('views.menu.resetMemoryDone2')}` : $t('views.menu.resetMemory') }}
      </button>
    </StartLine>

    <StartLine>
      <StartItem
        v-for="n in pageSize - currentPageScripts.length"
        :key="'placeholder-' + n"
        disabled="true"
      >
        {{ '\u00A0' }}
      </StartItem>
    </StartLine>
    <!-- 分页控制 -->
    <StartLine>
      <StartItem
        @click="currentPage--"
        :disabled="currentPage === 1"
      >
        <
      </StartItem>
      <StartItem
        disabled="true"
        style="font-size: 28px"
      >
        {{ currentPage }} / {{ totalPages }}
      </StartItem>
      <StartItem
        @click="currentPage++"
        :disabled="currentPage === totalPages"
      >
        >
      </StartItem>
      <!-- 返回按钮 -->
      <StartItem @click="emit('back')">{{ $t('views.menu.back') }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { StartItem, StartLine, StartList } from '../base'
import { useRouter } from 'vue-router'
import { type ScriptSummary, startScript, resetScriptState } from '@/api/services/script-info'
import { useGameStore } from '@/stores/modules/game'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import { useUIStore } from '@/stores/modules/ui/ui'
import { i18n } from '@/locales'

const emit = defineEmits<{
  (e: 'back'): void
}>()

const props = defineProps({
  scripts: {
    type: Array as () => ScriptSummary[],
    default: [],
  },
})

const router = useRouter()
const gameStore = useGameStore()
const dialogStore = useDialogStore()
const uiStore = useUIStore()

const currentPage = ref(1)
const pageSize = 3
// 记忆重置按钮状态：resettingName 防连点，resetDoneName 做 ✓ 短暂反馈
const resettingName = ref<string | null>(null)
const resetDoneName = ref<string | null>(null)
let resetDoneTimer = 0

const resetMemory = async (script: ScriptSummary) => {
  if (resettingName.value) return
  const confirmed = await dialogStore.confirm(
    i18n.global.t('views.menu.resetMemoryMessage', { name: script.script_name }),
    i18n.global.t('views.menu.resetMemoryTitle'),
  )
  if (!confirmed) return

  resettingName.value = script.script_name
  try {
    const removed = await resetScriptState(script.script_name)
    await dialogStore.alert(
      removed
        ? i18n.global.t('views.menu.resetMemoryDone')
        : i18n.global.t('views.menu.resetMemoryEmpty'),
      i18n.global.t('views.menu.resetMemoryTitle'),
    )
    resetDoneName.value = script.script_name
    clearTimeout(resetDoneTimer)
    resetDoneTimer = window.setTimeout(() => (resetDoneName.value = null), 2000)
  } catch {
    await dialogStore.alert(
      i18n.global.t('views.menu.resetMemoryFailed'),
      i18n.global.t('views.menu.resetMemoryTitle'),
    )
  } finally {
    resettingName.value = null
  }
}

const selectScript = async (script: ScriptSummary) => {
  // 带内容警告的剧本（如恐怖向）先弹确认，取消则不进入
  if (script.content_warning === 'horror') {
    const confirmed = await dialogStore.confirm(
      i18n.global.t('views.contentWarning.horrorMessage'),
      i18n.global.t('views.contentWarning.horrorTitle'),
    )
    if (!confirmed) return

    // 确认后先"卡死 → 花屏"再进入（恐怖演出的一部分）
    await uiStore.beginHorrorEntry()
  }

  await router.push('/chat')

  gameStore.enterStoryMode(script.script_name, script.content_warning)

  await startScript(script.script_name)
}

const totalPages = computed(() => {
  return Math.ceil(props.scripts.length / pageSize)
})

const currentPageScripts = computed(() => {
  const start = (currentPage.value - 1) * pageSize
  const end = start + pageSize
  return props.scripts.slice(start, end)
})
</script>

<style scoped>
/* 跟在大号剧本名按钮后面的记忆重置小字：与菜单同款白字阴影，只是字号小 */
.reset-memory-btn {
  margin-top: 26px;
  margin-left: 10px;
  padding: 4px 6px;
  vertical-align: middle;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.8);
  font-family: 'Maoken_Assorted_Sans', -apple-system, BlinkMacSystemFont, 'Segoe_UI', Roboto,
    'Helvetica_Neue', Arial, sans-serif;
  font-size: clamp(14px, 1.4vw, 24px);
  line-height: 1.2;
  cursor: pointer;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.45);
  transition: color 0.25s ease, transform 0.25s ease, text-shadow 0.25s ease;
}

.reset-memory-btn:hover:not(:disabled) {
  color: #ff6b7a;
  transform: translateY(-2px);
  text-shadow: 0 0 6px rgba(255, 107, 122, 0.5);
}

.reset-memory-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
</style>
