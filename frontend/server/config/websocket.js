const WebSocket = require("ws");

module.exports = (server) => {
  const wss = new WebSocket.Server({ server });
  return wss;
};
