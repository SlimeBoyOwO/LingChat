<template>
  <StartList>
    <StartLine v-for="(script, index) in currentPageScripts">
      <StartItem class="menu-subitem"
        :key="script.script_name"
        @click="selectScript(script)"
      >
        <span class="inline-flex items-center gap-2">
          <span>{{ script.script_name }}</span>
          <PluginTag
            v-if="script.source && script.source !== 'game'"
            :source="script.source"
          />
        </span>
      </StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem"
        v-for="n in pageSize - currentPageScripts.length"
        :key="'placeholder-' + n"
        disabled="true"
      >
        {{ '\u00A0' }}
      </StartItem>
    </StartLine>
    <!-- 分页控制 -->
    <StartLine>
      <StartItem class="menu-subitem"
        @click="currentPage--"
        :disabled="currentPage === 1"
      >
        <
      </StartItem>
      <StartItem class="menu-subitem"
        disabled="true"
        style="font-size: 28px"
      >
        {{ currentPage }} / {{ totalPages }}
      </StartItem>
      <StartItem class="menu-subitem"
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
import PluginTag from '@/components/ui/PluginTag.vue'
import { useRouter } from 'vue-router'
import { type ScriptSummary, startScript } from '@/api/services/script-info'
import { useGameStore } from '@/stores/modules/game'

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

const currentPage = ref(1)
const pageSize = 3

const selectScript = async (script: ScriptSummary) => {
  await router.push('/chat')

  gameStore.enterStoryMode(script.script_name)

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
