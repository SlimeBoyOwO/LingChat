<template>
  <MenuPage>
    <MenuItem title="背景选择">
      <Button type="big" @click="">上传自定义背景</Button>
    </MenuItem>
    <MenuItem title="圣光预览">
      <Slider @input="" @change="">弱/强</Slider>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { MenuPage } from "../../ui";
import { MenuItem } from "../../ui";
import { Button } from "../../base";
import { Slider } from "../../base";
</script>

<style scoped>
/* 样式部分 */
</style>
