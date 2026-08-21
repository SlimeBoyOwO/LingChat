<template>
  <StartList>
    <StartLine v-for="(script, index) in currentPageScripts">
      <StartItem
        :key="script.script_name"
        @click="selectScript(script)"
      >
        {{ script.script_name }}
      </StartItem>
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
import { type ScriptSummary, startScript } from '@/api/services/script-info'
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
