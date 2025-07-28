<template>
  <MenuPage>
    <MenuItem title="角色列表">
      <CharacterList>
        <div id="characters"></div>
      </CharacterList>
    </MenuItem>

    <MenuItem title="刷新人物列表">
      <Button type="big" @click="refreshCharacters">点我刷新~</Button>
    </MenuItem>

    <MenuItem title="创意工坊">
      <Button type="big" @click="gotoWorkshop">进入创意工坊</Button>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { h, createApp, ref, onMounted } from "vue";
import { MenuPage } from "../../ui";
import { MenuItem } from "../../ui";
import { Button } from "../../base";
import Character from "../../ui/Menu/CharacterCard.vue";
import CharacterList from "../../ui/Menu/CharacterList.vue";

onMounted(() => {
  interface CharacterInfomation {
    avatar: string;
    name: string;
    info: string;
  }
  function mountCharacter(infomation: CharacterInfomation) {
    const characterNode = h(Character, infomation);
    const app = createApp({
      render: () => characterNode,
    });
    app.mount("#characters");
  }

  // 这里添加人物
  mountCharacter({
    avatar: "",
    name: "灵灵",
    info: "闷骚的孩子",
  });
  // 这个函数多次调用也只有一个人物，燃尽了
});

const refreshCharacters = () => {
  // 当刷新人物列表的按钮点击之后在这里写处理逻辑
  console.log("刷新人物列表");
};
const gotoWorkshop = () => {
  // 当创意工坊的按钮点击之后在这里写处理逻辑
  console.log("进入创意工坊");
};
</script>

<style scoped>
/* --- 角色选择页面新样式 --- */

.section-header {
  display: flex;
  align-items: center;
  border-bottom: 2px solid var(--accent-color);
  padding-bottom: 8px;
  margin-bottom: 15px;
}

.section-header h4,
.section-header h5 {
  margin: 0;
  font-size: 18px;
  color: #333;
  font-weight: 600;
}

.character-card {
  display: flex;
  background-color: #fff;
  border-radius: 12px;
  box-shadow: 0 4px 15px rgba(0, 0, 0, 0.05);
  overflow: hidden;
  transition: all 0.3s ease;
}

.character-card:hover {
  transform: translateY(-5px);
  box-shadow: 0 8px 25px rgba(0, 0, 0, 0.1);
}

.character-avatar-container {
  width: 150px;
  height: 100%;
  flex-shrink: 0;
  padding: 15px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #f5f7fa 0%, #e8ecf3 100%);
}

.character-avatar {
  width: 100%;
  height: auto;
  object-fit: contain;
  border-radius: 8px;
}

.character-content-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 15px;
  position: relative;
}

.character-title {
  font-size: 16px;
  font-weight: 700;
  color: #333;
  margin-top: 0;
  margin-bottom: 8px;
}

.character-description {
  font-size: 13px;
  color: #666;
  line-height: 1.5;
  flex-grow: 1;
  margin-bottom: 15px;
}

.character-select-btn {
  align-self: flex-end; /* 按钮靠右 */
  background-color: #ccc;
  color: #666;
  border: none;
  padding: 8px 15px;
  border-radius: 20px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 13px;
  font-weight: 500;
}
.character-select-btn.selected {
  background-color: var(--accent-color);
  color: white;
}
.character-select-btn:not(.selected):hover {
  background-color: #555;
  color: white;
}
</style>
