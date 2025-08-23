import { DOM } from "../../../ui/dom";
import { DomUtils } from "../../../utils/dom-utils";
import EventBus from "../../../core/event-bus";
import request from "../../../core/request";

interface Character {
  character_id: number;
  title: string;
  info?: string;
  avatar_path?: string;
}

interface CharacterCard {
  id: number;
  title: string;
  info: string;
  avatar: string;
}

export class CharacterController {
  private characterList: HTMLElement;
  private domUtils: typeof DomUtils;
  private userId: number;

  constructor() {
    this.characterList = document.getElementById(
      "character-list"
    ) as HTMLElement;
    this.domUtils = DomUtils;
    this.userId = 1;
    this.init();
  }

  async init(): Promise<void> {
    try {
      const characters = await this.fetchCharacters();
      this.renderCharacters(characters);
      this.bindEvents();
      this.updateSelectedStatus();
    } catch (error) {
      console.error("Failed to initialize characters:", error);
    }
  }

  updateSelectedStatus(): void {
    EventBus.emit("system:character_updated");
  }

  bindEvents(): void {
    // 图片菜单点击事件
    DOM.menuCharacter?.addEventListener("click", () =>
      this.showCharacterPanel()
    );

    DOM.character.refreshCharactersBtn?.addEventListener("click", () =>
      this.refreshCharacters()
    );

    DOM.character.openWebBtn?.addEventListener("click", () =>
      this.openCreativeWeb()
    );

    document.querySelectorAll(".character-select-btn").forEach((btn) => {
      btn.addEventListener("click", () => {
        const characterId = btn.getAttribute("data-character-id");
        if (characterId) {
          this.selectCharacter(parseInt(characterId));
        }
      });
    });
  }

  showCharacterPanel(): void {
    if (DOM.menuCharacter && DOM.characterPage) {
      this.domUtils.showElements([DOM.menuCharacter, DOM.characterPage]);
      this.domUtils.hideElements(
        this.domUtils.getOtherPanelElements([
          DOM.menuCharacter,
          DOM.characterPage,
        ])
      );
    }
  }

  async openCreativeWeb(): Promise<any> {
    try {
      const response = await fetch("/api/v1/chat/character/open_web");
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const data = await response.json();
      return data;
    } catch (error) {
      alert("启动失败，请手动去lingchat的discussion网页");
      console.error("刷新失败:", error);
      throw error;
    }
  }

  async refreshCharacters(): Promise<any> {
    try {
      const response = await fetch(
        "/api/v1/chat/character/refresh_characters",
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
        }
      );

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const data = await response.json();
      alert("刷新成功");
      return data;
    } catch (error) {
      alert("刷新失败");
      console.error("刷新失败:", error);
      throw error;
    }
  }

  async fetchCharacters(): Promise<CharacterCard[]> {
    try {
      const list = (await request.characterGetAll()) as Character[];
      return list.map((char) => ({
        id: char.character_id,
        title: char.title,
        info: char.info || "暂无角色描述",
        // 使用新的文件获取接口
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
  }

  renderCharacters(characters: CharacterCard[]): void {
    this.characterList.innerHTML = characters
      .map((char) => this.createCharacterCard(char))
      .join("");
  }

  createCharacterCard(character: CharacterCard): string {
    return `
      <div class="character-card">
        <div class="character-avatar-container">
          <img src="${character.avatar}" alt="${character.title}" class="character-avatar">
        </div>
        <div class="character-content">
          <div class="character-title">${character.title}</div>
          <div class="character-description">${character.info}</div>
          <button class="character-select-btn" data-character-id="${character.id}">选择</button>
        </div>
      </div>
    `;
  }

  async selectCharacter(characterId: number): Promise<void> {
    return request
      .characterSelect(this.userId.toString(), characterId.toString())
      .then(() => {
        this.updateSelectedStatus();
      })
      .catch((error) => {
        console.error("切换角色失败:", error);
      });
  }
}
