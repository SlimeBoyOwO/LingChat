const fs = require("fs");
const path = require("path");
const dotenv = require("dotenv");
const { model } = require("../config");

function getModelInfo(req, res) {
  res.json({
    success: true,
    modelType: model.type,
    apiUrl: model.apiUrl,
  });
}

function getEnvConfig(req, res) {
  res.json({
    success: true,
    config: {
      apiKey: model.apiKey,
      apiUrl: model.apiUrl,
      modelType: model.type,
      systemPrompt: model.systemPrompt,
    },
  });
}

function updateEnvConfig(req, res) {
  const { apiKey, apiUrl, modelType, systemPrompt, frontendPort } = req.body;

  if (!apiKey && !apiUrl && !modelType && !systemPrompt && !frontendPort) {
    return res.status(400).json({
      success: false,
      message: "至少需要提供一个要更新的参数",
    });
  }

  try {
    const envPath = path.resolve(__dirname, "../../.env");
    let envConfig = {};

    try {
      const envContent = fs.readFileSync(envPath, "utf8");
      envConfig = dotenv.parse(envContent);
    } catch (err) {
      console.error("读取.env文件失败:", err);
      envConfig = {};
    }

    if (apiKey) envConfig.CHAT_API_KEY = apiKey;
    if (apiUrl) envConfig.CHAT_BASE_URL = apiUrl;
    if (modelType) envConfig.MODEL_TYPE = modelType;
    if (systemPrompt) envConfig.SYSTEM_PROMPT = systemPrompt;
    if (frontendPort) envConfig.FRONTEND_PORT = frontendPort;

    const newEnvContent = Object.entries(envConfig)
      .map(([key, value]) => {
        if (key === "SYSTEM_PROMPT") {
          return `${key}="${value.replace(/"/g, '\\"')}"`;
        }
        return `${key}="${value}"`;
      })
      .join("\n");

    fs.writeFileSync(envPath, newEnvContent);

    return res.json({
      success: true,
      message: "环境变量已更新",
    });
  } catch (error) {
    console.error("更新环境变量失败:", error);
    return res.status(500).json({
      success: false,
      message: "更新环境变量失败: " + error.message,
    });
  }
}

module.exports = {
  getModelInfo,
  getEnvConfig,
  updateEnvConfig,
};
