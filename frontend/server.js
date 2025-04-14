const express = require("express");
const http = require("http");
const WebSocket = require("ws");
const path = require("path");

const app = express();
const server = http.createServer(app);
const wss = new WebSocket.Server({ server });

const PORT = 3000;

// 静态文件服务
app.use(express.static("public"));

// 为每个页面添加路由
app.get("/", (req, res) => {
  res.sendFile(path.join(__dirname, "public/pages/index.html"));
});

app.get("/about", (req, res) => {
  res.sendFile(path.join(__dirname, "public/pages/about.html"));
});

// 连接到 Python WebSocket 服务
let pythonSocket = null;

function connectToPython() {
  // 添加协议头
  const ws = new WebSocket("ws://python-backend:8765", {
    headers: {
      Origin: "http://node-frontend:3000",
      Connection: "Upgrade",
      Upgrade: "websocket",
    },
  });

  // 添加握手验证
  ws.onopen = () => {
    console.log("WebSocket 握手成功");
    ws.send(
      JSON.stringify({
        type: "handshake",
        protocol: "websocket",
        version: "13", // WebSocket 协议版本
      })
    );
  };

  ws.on("open", () => {
    console.log("已连接到 Python 服务");
    pythonSocket = ws;
  });

  ws.on("message", (message) => {
    try {
      const data = JSON.parse(message);

      // 添加音频URL路径
      if (data.audioFile) {
        data.audioUrl = `/audio/${data.audioFile}`;
      }

      // 直接转发，不修改顺序
      wss.clients.forEach((client) => {
        if (client.readyState === WebSocket.OPEN) {
          client.send(JSON.stringify(data));
        }
      });
    } catch (e) {
      console.error("处理Python响应出错:", e);
    }
  });

  ws.on("error", (error) => {
    console.error("Python 服务连接错误:", error);
    pythonSocket = null;
  });

  ws.on("close", () => {
    console.log("Python 服务连接断开，5秒后重试...");
    pythonSocket = null;
    setTimeout(connectToPython, 5000);
  });

  return ws;
}

// 初始连接
connectToPython();

// WebSocket 连接处理
wss.on("connection", (ws) => {
  console.log("新的客户端连接");

  ws.on("message", (message) => {
    console.log("收到客户端消息:", message);

    // 将消息转发到 Python 服务
    if (pythonSocket && pythonSocket.readyState === WebSocket.OPEN) {
      pythonSocket.send(message);
    } else {
      ws.send(
        JSON.stringify({
          type: "error",
          message: "暂时无法连接到后端服务",
        })
      );
    }
  });

  ws.on("close", () => {
    console.log("客户端断开连接");
  });
});

// 启动服务器
server.listen(PORT, () => {
  console.log(`服务器运行在 http://0.0.0.0:${PORT}`);
});
