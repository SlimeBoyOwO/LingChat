const express = require("express");
const http = require("http");
const path = require("path");
const WebSocket = require("ws");
const bodyParser = require("body-parser");
const config = require("./config");
const websocketConfig = require("./config/websocket");
const pythonService = require("./services/pythonService");

const app = express();
const server = http.createServer(app);
const wss = websocketConfig(server);

// 中间件
app.use(bodyParser.json());
const projectRoot = path.resolve(__dirname, ".."); // 调整到 frontend 目录
app.use(express.static(path.join(projectRoot, "public")));

// 路由
app.use("/", require("./routes/webRoutes"));
app.use("/api/auth", require("./routes/authRoutes"));
app.use("/api", require("./routes/modelRoutes"));
app.use("/api", require("./routes/envRoutes"));

// WebSocket 处理
wss.on("connection", (ws) => {
  console.log("新的客户端连接");

  ws.on("message", (message) => {
    const msgString = message.toString("utf8");
    const parsedMessage = JSON.parse(msgString); // 尝试解析消息

    // 检查消息类型是否为 "ping"，如果是则跳过 debug 日志
    if (parsedMessage.type !== "ping") {
      console.log("收到客户端消息:", msgString);
    }

    const pythonSocket = pythonService.getPythonSocket();

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

// 连接到 Python 后端
pythonService.connectToPython(wss);

// 启动服务器
server.listen(config.frontend.port, config.frontend.bindAddr, () => {
  console.log(
    `Server running at http://${config.frontend.addr}:${config.frontend.port}`
  );
});
