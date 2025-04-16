// 获取人物图片元素
const characterImg = document.querySelector(".main-box img");
const characterContainer = document.querySelector(".avatar-container");
const audioPlayerBubble = document.getElementById("audioPlayerBubble");

// 统一的表情配置
const expressionConfig = {
  厌恶: {
    animation: "none",
    bubbleImage: "../pictures/animation/生气.webp",
    bubbleClass: "angry",
    audio: "../audio_effects/厌恶.wav",
    img: "../pictures/qinling/厌恶.png",
  },
  高兴: {
    animation: "happy-bounce",
    bubbleImage: "../pictures/animation/高兴.webp",
    bubbleClass: "happy",
    audio: "../audio_effects/愉快.wav",
    img: "../pictures/qinling/高兴或自信.png",
  },
  担心: {
    animation: "none",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/担心.png",
  },
  生气: {
    animation: "angry-jump",
    bubbleImage: "../pictures/animation/生气2.webp",
    bubbleClass: "angry",
    audio: "../audio_effects/生气.wav",
    img: "../pictures/qinling/生气.png",
  },
  紧张: {
    animation: "none",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/尴尬或紧张.png",
  },
  害怕: {
    animation: "none",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/害怕.png",
  },
  害羞: {
    animation: "none",
    bubbleImage: "../pictures/animation/害羞.webp",
    bubbleClass: "shy",
    audio: "../audio_effects/害羞.wav",
    img: "../pictures/qinling/害羞.png",
  },
  慌张: {
    animation: "none",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/慌张.png",
  },
  认真: {
    animation: "none",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/认真.png",
  },
  无奈: {
    animation: "none",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/无奈.png",
  },
  兴奋: {
    animation: "none",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/兴奋.png",
  },
  疑惑: {
    animation: "none",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/疑惑.png",
  },
};

function changeEmotion(emotion) {
  changeExpression(emotion);
  showBubble(emotion);
}

// 切换表情函数
function changeExpression(emotion) {
  const config = expressionConfig[emotion];

  // 移除所有动画类（从统一配置中获取）
  Object.values(expressionConfig).forEach(({ animation }) => {
    characterContainer.classList.remove(animation);
  });

  // 如果有新图片则更换

  characterImg.src = config.img;

  // 添加新动画
  if (config.animation && config.animation !== "none") {
    characterContainer.classList.remove("normal");
    characterContainer.classList.add(config.animation);

    // 动画结束后处理
    const handler = () => {
      characterContainer.classList.remove(config.animation);
      characterContainer.classList.add("normal");
      characterContainer.removeEventListener("animationend", handler);
    };
    characterContainer.addEventListener("animationend", handler, {
      once: true,
    });
  }
}

function showBubble(emotion) {
  const config = expressionConfig[emotion];
  const bubble = document.querySelector(".bubble");
  const version = Date.now();

  if (config.bubbleImage === "none") return;

  // 设置音频源并播放
  audioPlayerBubble.src = config.audio;
  audioPlayerBubble.load(); // 重新加载音频
  audioPlayerBubble.play();

  // 重置动画
  bubble.classList.remove("show");
  void bubble.offsetWidth;

  // 设置带时间戳的新源
  bubble.style.backgroundImage = `url(${config.bubbleImage}??t=${version}#t=0.1)`;

  // 添加对应的表情类
  bubble.classList.add(config.bubbleClass);

  // 触发显示
  bubble.classList.add("show");

  // 动画结束后隐藏
  bubble.addEventListener(
    "animationend",
    () => {
      bubble.classList.remove("show");
      bubble.classList.remove(config.bubbleClass);
    },
    { once: true }
  );
}
