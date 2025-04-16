const inputMessage = document.getElementById("inputMessage");
const responseMessage = document.getElementById("inputMessage"); // 有意使用同一个元素
const sendButton = document.getElementById("sendButton");
const statusDiv = document.getElementById("status");
const audioPlayer = document.getElementById("audioPlayer");
const audioStatus = document.getElementById("audioStatus");
const avatarTitle = document.getElementById("character");
const avatarSubTitle = document.getElementById("character-sub");
const avatarEmotion = document.getElementById("character-emotion");

// 获取人物图片元素
const characterImg = document.querySelector(".main-box img");
const characterContainer = document.querySelector(".avatar-container");
const audioPlayerBubble = document.getElementById("audioPlayerBubble");

const writer = new TypeWriter(inputMessage);

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
    audio: "../audio_effects/喜悦.wav",
    img: "../pictures/qinling/高兴或自信.png",
  },
  担心: {
    animation: "none",
    bubbleImage: "../pictures/animation/流泪.webp",
    bubbleClass: "none",
    audio: "../audio_effects/伤心.wav",
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
    bubbleImage: "../pictures/animation/紧张.webp",
    bubbleClass: "none",
    audio: "../audio_effects/尴尬.wav",
    img: "../pictures/qinling/尴尬或紧张.png",
  },
  害怕: {
    animation: "none",
    bubbleImage: "../pictures/animation/惊讶.webp",
    bubbleClass: "none",
    audio: "../audio_effects/震惊.wav",
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
    bubbleImage: "../pictures/animation/慌乱.webp",
    bubbleClass: "none",
    audio: "../audio_effects/震惊.wav",
    img: "../pictures/qinling/慌张.png",
  },
  认真: {
    animation: "serious-think",
    bubbleImage: "none",
    bubbleClass: "none",
    audio: "none",
    img: "../pictures/qinling/认真.png",
  },
  无奈: {
    animation: "none",
    bubbleImage: "../pictures/animation/叹气.webp",
    bubbleClass: "none",
    audio: "../audio_effects/叹气.wav",
    img: "../pictures/qinling/无奈.png",
  },
  兴奋: {
    animation: "none",
    bubbleImage: "../pictures/animation/聊天.webp",
    bubbleClass: "none",
    audio: "../audio_effects/聊天.wav",
    img: "../pictures/qinling/兴奋.png",
  },
  疑惑: {
    animation: "none",
    bubbleImage: "../pictures/animation/疑问.webp",
    bubbleClass: "none",
    audio: "../audio_effects/疑问.wav",
    img: "../pictures/qinling/疑惑.png",
  },
  AI思考: {
    animation: "none",
    bubbleImage: "../pictures/animation/AI思考.webp",
    bubbleClass: "none",
    audio: "../audio_effects/无语.wav",
    img: "none",
  },
};

// 获取当前域名，如果为空则使用 localhost
const host = window.location.hostname || 'localhost';
let protocol;
if (window.location.protocol === 'http:') {
    protocol = 'ws';
} else if (window.location.protocol === 'https:') {
    protocol = 'wss';
} else {
    // 若为 file:// 协议，默认使用 ws
    protocol = 'ws';
}
// 创建 WebSocket 连接
const socket = new WebSocket(`${protocol}://${host}:3000/ws`);    

// const socket = new WebSocket("wss://frp-oil.com:58025//ws");

// 添加消息队列和状态变量
let messageQueue = [];
let isProcessing = false;
let currentMessagePart = null;
let isWaitingForResponse = false; // 新增：等待AI回复标志
let lastEmotion = "正常";

socket.addEventListener("open", (event) => {
  statusDiv.textContent = "已连接到服务器";
  sendButton.disabled = false;
});

socket.addEventListener("message", (event) => {
  const data = JSON.parse(event.data);

  if (data.type === "reply") {
    if (data.isMultiPart) {
      // 如果是多部分消息，添加到队列
      messageQueue.push(data);

      // 如果是第一部分，开始处理
      if (data.partIndex === 0 && !isProcessing) {
        processNextMessage();
      }
    } else {
      // 单条消息直接显示
      displayMessage(data);
    }
  }
});

let textSpeed = localStorage.getItem("textSpeed") || "medium"; // 默认中等速度

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
  if (
    config.animation &&
    config.animation !== "none" &&
    lastEmotion !== emotion
  ) {
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

  if (config.bubbleImage === "none" || lastEmotion === emotion) return;

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
  setTimeout(() => {
    bubble.classList.remove("show");
    bubble.classList.remove(config.bubbleClass);
  }, 2000); // 2秒后执行

  //bubble.addEventListener(
  //  "animationend",
  //  () => {
  //    bubble.classList.remove("show");
  //    bubble.classList.remove(config.bubbleClass);
  //  },
  //  { once: true }
  //);
}

// 启用输入框
function enableInput() {
  inputMessage.disabled = false;
  inputMessage.placeholder = "输入消息...";
  avatarTitle.innerText = "可爱的你";
  avatarSubTitle.innerText = "狼狼大学";
  writer.stop();
}

// 禁用输入框并显示思考状态
function showThinking() {
  inputMessage.disabled = true;
  inputMessage.placeholder = "灵灵正在思考...";
  responseMessage.value = "灵灵正在思考...";
  showBubble("AI思考");
}

// 处理下一条消息
function processNextMessage() {
  if (messageQueue.length === 0) {
    isProcessing = false;
    return;
  }

  isProcessing = true;
  inputMessage.placeholder = "";
  currentMessagePart = messageQueue.shift();
  avatarTitle.innerText = "钦灵";
  avatarSubTitle.innerText = "Slime Studio";
  displayMessage(currentMessagePart);

  // 设置键盘事件监听器
  document.addEventListener("keydown", handleKeyPress);
}

// 显示消息
function displayMessage(data) {
  const resMessage =
    data.message + (data.motionText ? ` （${data.motionText}）` : "");

  writer.start(resMessage, textSpeed);

  const emotion = data.emotion;
  avatarEmotion.innerText = data.originalTag;
  avatarTitle.innerText = "钦灵";
  avatarSubTitle.innerText = "Slime Studio";
  changeEmotion(emotion);
  lastEmotion = emotion;
  addToHistory(null, resMessage, false);

  // 处理音频播放
  if (data.audioFile) {
    audioStatus.textContent = "准备播放音频...";
    const audioUrl = `/audio/${data.audioFile}`;

    audioPlayer.src = audioUrl;
    audioPlayer.load();

    const playPromise = audioPlayer.play();

    playPromise
      .then(() => {
        audioStatus.textContent = "正在播放音频...";
      })
      .catch((e) => {
        console.error("播放失败:", e);
        audioStatus.textContent = `播放失败: ${e.message}`;
      });

    audioPlayer.onended = () => {
      audioStatus.textContent = "";
    };
  } else {
    // 如果是最后一条消息，显示不同提示
    if (currentMessagePart.partIndex === currentMessagePart.totalParts - 1) {
      audioStatus.textContent = "按Enter或发送按钮清空对话...";
    } else {
      audioStatus.textContent = "按Enter或发送按钮继续...";
    }
  }
}

// 处理按键事件
function handleKeyPress(event) {
  if (event.key === "Enter") {
    event.preventDefault(); // 防止表单提交
    // 防止按钮文本被选中
    window.getSelection().removeAllRanges();
    handleContinue();
  }
}

// 处理继续/清空逻辑
function handleContinue() {
  // 移除当前监听器
  document.removeEventListener("keydown", handleKeyPress);

  // 检查是否是最后一条消息
  if (
    currentMessagePart &&
    currentMessagePart.partIndex === currentMessagePart.totalParts - 1
  ) {
    // 清空对话框
    responseMessage.value = "";
    audioStatus.textContent = "";
    currentMessagePart = null;
    messageQueue = [];
    isProcessing = false;
    avatarEmotion.innerText = "";
    lastEmotion = "正常";
    addToHistory(null, null, true);
    enableInput(); // 重新启用输入框
    isWaitingForResponse = false; // AI已回复
  } else {
    // 处理下一条消息
    processNextMessage();
  }
}

// 发送消息或继续对话
function sendOrContinue() {
  const message = inputMessage.value.trim();

  if (isProcessing) {
    // 如果正在处理多部分消息，则执行继续逻辑
    handleContinue();
  } else if (message) {
    // 否则发送新消息
    addToHistory(message, null, false);
    showThinking(); // 显示思考状态
    isWaitingForResponse = true;
    messageQueue = [];
    avatarTitle.innerText = "钦灵";
    avatarSubTitle.innerText = "Slime Studio";
    isProcessing = false;

    socket.send(
      JSON.stringify({
        type: "message",
        content: message,
      })
    );

    inputMessage.value = "";
  } else {
    alert("请输入消息");
  }
}

// 暴露设置速度的函数
function setTextSpeed(speed) {
  textSpeed = speed;
  localStorage.setItem("textSpeed", speed);
}

// 发送按钮点击事件
sendButton.addEventListener("click", (e) => {
  e.preventDefault(); // 防止按钮默认行为
  sendOrContinue();
});

// 输入框Enter键事件
inputMessage.addEventListener("keypress", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    sendOrContinue();
  }
});

socket.addEventListener("error", (event) => {
  statusDiv.textContent = "连接出错";
  enableInput(); // 出错时也启用输入框
});

socket.addEventListener("close", (event) => {
  statusDiv.textContent = "连接已断开";
  sendButton.disabled = true;
  enableInput(); // 断开时也启用输入框
});
