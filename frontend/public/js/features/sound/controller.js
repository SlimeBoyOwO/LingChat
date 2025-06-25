import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";
import request from "../../core/request.js";

export class SoundController {
  constructor() {
    this.processing = false;
    this.domUtils = DomUtils;
    this.init();
  }

  init() {
    this.bindEvents();
    this.setupAudioControls();
  }

  bindEvents() {
    if (!DOM.menuSound) return;

    DOM.menuSound.addEventListener("click", () => this.showSoundPanel());
  }

  showSoundPanel() {
    this.domUtils.showElements([DOM.menuSound, DOM.soundPage]);
    this.domUtils.hideElements(
      this.domUtils.getOtherPanelElements([DOM.menuSound, DOM.soundPage])
    );
  }

  // 音频播放控制
  playBubbleSound() {
    if (DOM.bubbleAudio) {
      DOM.bubbleAudio.currentTime = 0;
      DOM.bubbleAudio.play();
    }
  }

  stopAllSounds() {
    [DOM.audioPlayer, DOM.bubbleAudio, DOM.BackAudioPlayer].forEach(
      (player) => {
        if (player) {
          player.pause();
          player.currentTime = 0;
        }
      }
    );
  }

  // ========== 音频控制设置 ===========
  setupAudioControls() {
    // 音量控制
    this.setupVolumeControls();

    // 测试按钮
    this.setupTestButtons();

    // 背景音乐控制
    this.setupBackgroundMusic();

    // 初始化音乐列表
    this.loadMusicList();

    // 上传事件监听
    this.setupUploadHandler();
  }

  setupVolumeControls() {
    DOM.VolumeAudioPlayer.addEventListener("input", (e) => {
      DOM.audioPlayer.volume = e.target.value / 100;
    });

    DOM.VolumeBubbleAudio.addEventListener("input", (e) => {
      DOM.bubbleAudio.volume = e.target.value / 100;
    });

    DOM.VolumeBackAudioPlayer.addEventListener("input", (e) => {
      DOM.BackAudioPlayer.volume = e.target.value / 100;
    });
  }

  setupTestButtons() {
    DOM.TestAudioPlayer.addEventListener("click", () => {
      this.playTestSound(DOM.audioPlayer, "/audio/角色音量测试.wav");
    });

    DOM.TestBubbleAudio.addEventListener("click", () => {
      this.playTestSound(DOM.bubbleAudio, "/audio_effects/疑问.wav");
    });
  }

  playTestSound(player, url) {
    if (!player) return;

    player.src = url;
    player.play().catch((e) => {
      console.error("播放测试音效失败:", e);
    });
  }

  setupBackgroundMusic() {
    // 初始化背景音乐播放器
    DOM.BackAudioPlayer.loop = true;

    // 播放/暂停按钮
    DOM.PlayPauseMusic.addEventListener("click", () => {
      if (DOM.BackAudioPlayer.paused) {
        this.playCurrentMusic();
      } else {
        DOM.BackAudioPlayer.pause();
      }
      this.updatePlayButtonIcon();
    });

    // 停止按钮
    DOM.StopMusic.addEventListener("click", () => {
      DOM.BackAudioPlayer.pause();
      DOM.BackAudioPlayer.currentTime = 0;
      this.updatePlayButtonIcon();
    });
  }

  playCurrentMusic() {
    if (!DOM.BackAudioPlayer.src) {
      // 如果没有选中音乐，尝试播放列表第一首
      const firstItem = DOM.MusicList.querySelector(".Music-List");
      if (firstItem) {
        this.playMusicFromItem(firstItem);
      }
      return;
    }

    DOM.BackAudioPlayer.play().catch((e) => {
      console.error("播放音乐失败:", e);
    });
  }

  updatePlayButtonIcon() {
    // 这里可以添加更新播放按钮图标的逻辑
    const icon = DOM.PlayPauseMusic.querySelector("i");
    if (icon) {
      icon.className = DOM.BackAudioPlayer.paused ? "icon-play" : "icon-pause";
    }
  }

  loadMusicList() {
    return request.backmusicList()
    .then(list => {
      this.renderMusicList(list);
    })
    .catch(error => {
      console.error("加载音乐列表错误:", error);
    })
  }

  renderMusicList(musicList) {
    DOM.MusicList.innerHTML = "";

    musicList.forEach((item) => {
      const musicItem = document.createElement("div");
      musicItem.className = "Music-List da";
      musicItem.dataset.url = item.url;

      // 音乐名称（可点击播放）
      const musicName = document.createElement("div");
      musicName.textContent = item.name;
      musicName.className = "music-name";
      musicName.addEventListener("click", () =>
        this.playMusicFromItem(musicItem)
      );

      // 删除按钮
      const deleteBtn = document.createElement("button");
      deleteBtn.className = "button Music-List db";
      deleteBtn.textContent = "删除";
      deleteBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        this.deleteMusicItem(item.url);
      });

      musicItem.appendChild(musicName);
      musicItem.appendChild(deleteBtn);
      DOM.MusicList.appendChild(musicItem);
    });
  }

  async playMusicFromItem(item) {
    const url = item.dataset.url;
    if (!url) return;

    try {
      DOM.BackAudioPlayer.src = url;
      DOM.BackAudioPlayer.play().catch((e) => {
        console.error("播放失败:", e);
      });

      // 更新当前播放显示
      DOM.MusicName.textContent = item.querySelector(".music-name").textContent;
      this.updatePlayButtonIcon();
    } catch (error) {
      console.error("播放音乐错误:", error);
    }
  }

  async deleteMusicItem(url) {
    if (!confirm("确定要删除这首音乐吗？")) return;
    return request.backmusicDelete(url)
    .then(() => {
      this.loadMusicList(); // 刷新列表
    })
    .catch(error => {
      console.error("删除音乐错误:", error);
      alert("删除音乐失败");
    })
  }

  setupUploadHandler() {
    DOM.AddMusic.addEventListener("click", async () => {
      const fileInput = DOM.MusicUpload;
      if (!fileInput.files || fileInput.files.length === 0) {
        alert("请先选择音乐文件");
        return;
      }

      const file = fileInput.files[0];
      const fileName = file.name;
      const fileExt = fileName.slice(fileName.lastIndexOf(".")).toLowerCase();

      // 验证文件类型
      const allowedExts = [
        ".mp3",
        ".wav",
        ".flac",
        ".webm",
        ".weba",
        ".ogg",
        ".m4a",
        ".oga",
      ];
      if (!allowedExts.includes(fileExt)) {
        alert("请上传支持的音频格式: " + allowedExts.join(", "));
        return;
      }
      
      const formData = new FormData();
      formData.append("file", file);
      return request.backmusicUpload(formData)
      .then(() => {
        alert("音乐上传成功");
        fileInput.value = ""; // 清空文件选择
        this.loadMusicList(); // 刷新列表
      })
      .catch(error => {
        console.error("上传音乐错误:", error);
        alert("音乐上传失败");
      })
    });
  }
}
