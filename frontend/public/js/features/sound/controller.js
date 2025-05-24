import { DOM } from "../../ui/dom.js";
import { DomUtils } from "../../utils/dom-utils.js";

export class SoundController {
  constructor() {
    this.processing = false;
    this.domUtils = DomUtils;
    this.init();
  }

  init() {
    this.bindEvents();
    this.AudioSetting();
  }

  bindEvents() {
    if (!DOM.menuSound) return;

    DOM.menuSound.addEventListener("click", () => this.toggleSoundPanel());
  }

  toggleSoundPanel() {
    if (this.processing) return;
    this.processing = true;

    requestAnimationFrame(() => {
      // 显示声音相关元素
      this.domUtils.showElements([
        DOM.menuContent,
        DOM.menuSound,
        DOM.soundPage,
      ]);

      // 隐藏其他面板元素
      this.domUtils.hideElements([
        DOM.menuImage,
        DOM.imagePage,
        DOM.menuSave,
        DOM.history.toggle,
        DOM.history.content,
        DOM.history.clearBtn,
        DOM.menuText,
        DOM.textPage,
        DOM.savePage,
      ]);

      setTimeout(() => {
        this.processing = false;
      }, 300);
    });
  }

  // 可扩展的音频控制方法
  playBubbleSound() {
    if (DOM.bubbleAudio) {
      DOM.bubbleAudio.currentTime = 0;
      DOM.bubbleAudio.play();
    }
  }

  stopAllSounds() {
    [DOM.audioPlayer, DOM.bubbleAudio].forEach((player) => {
      if (player) {
        player.pause();
        player.currentTime = 0;
      }
    });
  }

  // ==========新修改 头==========

  AudioSetting() {
    // 音量修改
    DOM.VolumeAudioPlayer.addEventListener("input", (e) => {
      DOM.audioPlayer.volume = e.target.value / 100;
    });
    DOM.VolumeBubbleAudio.addEventListener("input", (e) => {
      DOM.bubbleAudio.volume = e.target.value / 100;
    });
    DOM.VolumeBackAudioPlayer.addEventListener("input", (e) => {
      DOM.BackAudioPlayer.volume = e.target.value / 100;
    });

    // 音量测试
    DOM.TestAudioPlayer.addEventListener("click", (e) => {
      DOM.audioPlayer.src = "/audio/%E8%A7%92%E8%89%B2%E9%9F%B3%E9%87%8F%E6%B5%8B%E8%AF%95.wav";
      DOM.audioPlayer.play();
    });
    DOM.TestBubbleAudio.addEventListener("click", (e) => {
      DOM.bubbleAudio.src = "/audio_effects/%E7%96%91%E9%97%AE.wav";
      DOM.bubbleAudio.play();
    });

    // 背景音乐
    // 播放 暂停 以及停止逻辑 （由于后端尚未配置，将无法使用加载和上传音乐列表；暂时使用https://files.a2942.top:5904/indexmusic/music/list/Kozoro%20-%20Where%20We%20Belong.mp3代替测试音频）
    DOM.BackAudioPlayer.src = "https://files.a2942.top:5904/indexmusic/music/list/Kozoro%20-%20Where%20We%20Belong.mp3";
    DOM.MusicName.textContent = "Kozoro - Where We Belong";
    DOM.BackAudioPlayer.loop = true; // 循环播放
    DOM.PlayPauseMusic.addEventListener("click", (e) => {
      if (DOM.BackAudioPlayer.paused) {
        DOM.BackAudioPlayer.play();
      } else {
        DOM.BackAudioPlayer.pause();
      }
    });
    // 停止音乐
    DOM.StopMusic.addEventListener("click", (e) => {
      DOM.BackAudioPlayer.pause();
      DOM.BackAudioPlayer.currentTime = 0;
    });

    function updatelist() {
      // 加载音乐列表，name and url 
      fetch("/api/v1/chat/back-music/list")
        .then((response) => response.json())
        .then((data) => {
          data.forEach((item) => {
            const musicItem = document.createElement("div");
            musicItem.className = "Music-List da";

            const musicName = document.createElement("div");
            musicName.textContent = item.name;

            const deleteButtonContainer = document.createElement("div");
            const deleteButton = document.createElement("button");
            deleteButton.className = "button Music-List db";
            deleteButton.textContent = "删除";

            deleteButton.addEventListener("click", () => {
              fetch(`/api/v1/chat/back-music/delete?url=${encodeURIComponent(item.url)}`, {
                method: "DELETE",
              })
                .then((response) => {
                  if (response.ok) {
                    alert("音乐删除成功");
                    DOM.MusicList.innerHTML = ""; // 清空列表
                    updatelist();
                  } else {
                    alert("音乐删除失败");
                  }
                })
                .catch((error) => {
                  console.error("Error:", error);
                  alert("音乐删除失败");
                });
            });

            deleteButtonContainer.appendChild(deleteButton);
            musicItem.appendChild(musicName);
            musicItem.appendChild(deleteButtonContainer);
            DOM.MusicList.appendChild(musicItem);
          });
        })
        .catch((error) => {
          console.error("Error:", error);
        });
    }

    updatelist();

    //处理音乐上传
    DOM.AddMusic.addEventListener("click", (e) => {
      if (DOM.MusicUpload.value) {
        // 判断是否是音乐文件
        if (
          DOM.MusicUpload.value.endsWith(".mp3") ||
          DOM.MusicUpload.value.endsWith(".wav") ||
          DOM.MusicUpload.value.endsWith(".flac") ||
          DOM.MusicUpload.value.endsWith(".webm") ||
          DOM.MusicUpload.value.endsWith(".weba") ||
          DOM.MusicUpload.value.endsWith(".ogg") ||
          DOM.MusicUpload.value.endsWith(".m4a") ||
          DOM.MusicUpload.value.endsWith(".oga")
        ) {
          // /api/v1/chat/back-music/upload?file=***&name=***
          const file = DOM.MusicUpload.files[0];
          const formData = new FormData();
          formData.append("file", file);
          formData.append("name", file.name);
          fetch("/api/v1/chat/back-music/upload", {
            method: "POST",
            body: formData,
          })
            .then((response) => {
              if (response.ok) {
                alert("音乐上传成功");
                DOM.MusicUpload.value = "";
                DOM.MusicList.innerHTML = ""; // 清空列表
                updatelist();
              } else {
                alert("音乐上传失败");
              }
            })
            .catch((error) => {
              console.error("Error:", error);
              alert("音乐上传失败");
            });
        } else {
          alert("请上传正确的音乐文件");
        }
      } else {
        alert("请先选择音乐文件");
      }
    });
  }

  // ==========新修改 尾==========
}
