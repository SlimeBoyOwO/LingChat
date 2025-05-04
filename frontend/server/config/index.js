require("dotenv").config();

module.exports = {
  frontend: {
    addr: process.env.FRONTEND_ADDR || "localhost",
    bindAddr: process.env.FRONTEND_BIND_ADDR || "0.0.0.0",
    port: process.env.FRONTEND_PORT || 3000,
  },
  backend: {
    addr: process.env.BACKEND_ADDR || "localhost",
    port: process.env.BACKEND_PORT || 8765,
  },
  model: {
    type: process.env.MODEL_TYPE || "unknown",
    apiUrl: process.env.CHAT_BASE_URL || "",
    apiKey: process.env.CHAT_API_KEY || "",
    systemPrompt: process.env.SYSTEM_PROMPT || "",
  },
};
