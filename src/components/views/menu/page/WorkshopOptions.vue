<template>
  <StartList>
    <StartLine>
      <StartItem class="menu-subitem" disabled="true">角色编辑器(开发中)</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" @click="router.push('/script-editor')">剧本编辑器</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" @click="router.push('/workshop')">{{ $t('views.menu.cloudWorkshop') }}</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" disabled="true">游戏编辑器(开发中)</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" @click="emit('back')">返回</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import { StartItem, StartLine, StartList } from '../base'

const emit = defineEmits<{
  (e: 'back'): void
}>()

const router = useRouter()
</script>
