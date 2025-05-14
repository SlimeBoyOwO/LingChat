const WebSocket = require("ws");
const { backend } = require("../config");
const logger = require("../utils/logger");
// handlePythonMessage 和 requestService 现在可能不再用于主要的消息转发流程
// const { handlePythonMessage } = require("./requestService");

let pythonSocket = null;
const pendingRequests = {}; // 如果不再使用 handlePythonMessage 处理特定响应，这个可能也不需要了
let wssInstance = null; // 新增：用于存储从 app.js 传过来的 wss 实例

// 修改 connectToPython 接受 wss 参数
function connectToPython(wss) {
  // 存储 wss 实例，以便在回调函数中使用
  if (!wss) {
    logger.error(
      "WSS instance is required for connectToPython to enable broadcasting."
    );
    // 你可以在这里决定是抛出错误还是允许连接但不广播
  }
  wssInstance = wss;

  logger.debug(`尝试连接到 Python 服务: ws://${backend.addr}:${backend.port}`);
  const ws = new WebSocket(`ws://${backend.addr}:${backend.port}`); // 简化连接头信息，通常不需要手动设置

  ws.on("open", () => {
    logger.debug("已连接到 Python 服务");
    pythonSocket = ws;
  });

  // *** 在这里直接实现广播逻辑 ***
  ws.on("message", (messageBuffer) => {
    const messageString = messageBuffer.toString("utf8");
    // logger.debug("收到 Python 消息:", messageString);                 <=====调试再打开

    // 检查 wssInstance 是否已设置
    if (!wssInstance) {
      logger.error(
        "WSS instance not available, cannot broadcast message from Python."
      );
      return; // 如果没有 wss 实例，无法广播
    }

    try {
      const data = JSON.parse(messageString);

      // --- 开始：复制 server.js 的广播逻辑 ---
      // 添加音频URL路径
      if (data.audioFile) {
        // 确保你的 Express 应用配置了 /audio 路由来提供静态文件
        data.audioUrl = `/audio/${data.audioFile}`;
      }

      // 直接转发广播给所有连接的前端客户端
      wssInstance.clients.forEach((client) => {
        // 确保客户端连接仍然是打开状态
        if (client.readyState === WebSocket.OPEN) {
          client.send(JSON.stringify(data));
        }
      });
      // --- 结束：复制 server.js 的广播逻辑 ---

      // 注意：这里不再调用 handlePythonMessage，因为我们直接进行了广播
      // 如果你仍然需要 handlePythonMessage 处理某些特殊类型的消息（如 auth_response），
      // 你需要在这里添加逻辑判断，决定是广播还是调用 handlePythonMessage。
      // 例如:
      // if (data.type === 'some_special_type') {
      //   handlePythonMessage(messageString, pendingRequests); // 假设 handlePythonMessage 能处理字符串
      // } else {
      //   // 执行上面的广播代码
      // }
    } catch (e) {
      logger.error("处理 Python 响应并广播时出错:", e);
    }
  });

  ws.on("error", (error) => {
    logger.error(`Python 服务连接失败，正在尝试重新连接 ${error.message}`);
    pythonSocket = null;
  });

  ws.on("close", (code, reason) => {
    pythonSocket = null;
    // 重连时也需要 wss 实例
    setTimeout(() => connectToPython(wssInstance), 5000); // 传递存储的 wssInstance
  });

  return ws;
}

function getPythonSocket() {
  return pythonSocket;
}

module.exports = {
  connectToPython,
  getPythonSocket,
  pendingRequests, // 保留导出，以防万一其他地方用到
};
