<template>
  <MenuPage>
    <MenuItem title="角色列表">
      <CharacterList>
        <div class="character-list character-grid">
          <CharacterCard
            v-for="character in characters"
            :key="character.id"
            :avatar="character.avatar"
            :name="character.title"
            :info="character.info"
          >
            <template #actions>
              <Button type="select"" @click="selectCharacter(character.id)">选择</Button>
            </template>
          </CharacterCard>
        </div>
      </CharacterList>
    </MenuItem>

    <MenuItem title="刷新人物列表" size="small">
      <Button type="big" @click="refreshCharacters">点我刷新~</Button>
    </MenuItem>

    <MenuItem title="创意工坊" size="small">
      <Button type="big" @click="openCreativeWeb">进入创意工坊</Button>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { MenuPage } from "../../ui";
import { MenuItem } from "../../ui";
import { Button } from "../../base";
import CharacterCard from "../../ui/Menu/CharacterCard.vue";
import CharacterList from "../../ui/Menu/CharacterList.vue";
import {
  characterGetAll,
  characterSelect,
} from "../../../api/services/character";
import type { Character as ApiCharacter } from "../../../types";

interface CharacterCard {
  id: number;
  title: string;
  info: string;
  avatar: string;
}

const characters = ref<CharacterCard[]>([]);
const userId = ref<number>(1);

const fetchCharacters = async (): Promise<CharacterCard[]> => {
  try {
    const list = await characterGetAll();
    return list.map((char: ApiCharacter) => ({
      id: parseInt(char.character_id),
      title: char.title,
      info: char.info || "暂无角色描述",
      avatar: char.avatar_path
        ? `/api/v1/chat/character/character_file/${encodeURIComponent(
            char.avatar_path
          )}`
        : "../pictures/characters/default.png",
    }));
  } catch (error) {
    console.error("获取角色列表失败:", error);
    return [];
  }
};

const loadCharacters = async (): Promise<void> => {
  try {
    const characterData = await fetchCharacters();
    characters.value = characterData;
  } catch (error) {
    console.error("加载角色失败:", error);
  }
};

const updateSelectedStatus = (): void => {
  // eventBus.emit("system:character_updated");
};

const selectCharacter = async (characterId: number): Promise<void> => {
  try {
    await characterSelect({
      user_id: userId.value.toString(),
      character_id: characterId.toString(),
    });
    updateSelectedStatus();
  } catch (error) {
    console.error("切换角色失败:", error);
  }
};

const refreshCharacters = async (): Promise<void> => {
  try {
    const response = await fetch("/api/v1/chat/character/refresh_characters", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    await response.json();
    alert("刷新成功");
    await loadCharacters(); // 重新加载角色列表
  } catch (error) {
    alert("刷新失败");
    console.error("刷新失败:", error);
  }
};

const openCreativeWeb = async (): Promise<void> => {
  try {
    const response = await fetch("/api/v1/chat/character/open_web");
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    await response.json();
  } catch (error) {
    alert("启动失败，请手动去lingchat的discussion网页");
    console.error("打开创意工坊失败:", error);
  }
};

// 初始化加载角色列表
onMounted(() => {
  loadCharacters();
});
</script>

<style scoped>
/*=========角色css部分=========*/
/* 角色选择网格布局 */
.character-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 20px;
  padding: 15px;
  width: 100%;
}

@media (max-width: 768px) {
  .character-grid {
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  }
}
</style>
