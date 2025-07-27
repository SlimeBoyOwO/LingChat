<template>
  <MenuPage>
    <MenuItem title="角色音量">
      <Slider @input="" @change="">弱/强</Slider>
    </MenuItem>
    <MenuItem title="气泡音量">
      <Slider @input="" @change="">弱/强</Slider>
    </MenuItem>
    <MenuItem title="背景音量">
      <Slider @input="" @change="">弱/强</Slider>
    </MenuItem>
    <MenuItem title="声音测试">
      <Button type="big" @click="">测试角色音量</Button>
      <Button type="big" @click="">测试气泡音量</Button>
    </MenuItem>
    <MenuItem title="背景音乐设置"> </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { MenuPage } from "../../ui";
import { MenuItem } from "../../ui";
import { Slider, Button } from "../../base";
</script>

<style scoped>
/* 样式部分 */
</style>
