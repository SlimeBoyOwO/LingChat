/**
 * 前端日志记录器，与后端日志风格一致
 */

// ANSI 颜色代码
const Color = {
  // 文本颜色
  BLACK: "\x1b[30m",
  RED: "\x1b[31m",
  GREEN: "\x1b[32m",
  YELLOW: "\x1b[33m",
  BLUE: "\x1b[34m",
  MAGENTA: "\x1b[35m",
  CYAN: "\x1b[36m",
  WHITE: "\x1b[37m",
  // 高亮
  BRIGHT_BLACK: "\x1b[90m",
  BRIGHT_RED: "\x1b[91m",
  BRIGHT_GREEN: "\x1b[92m",
  BRIGHT_YELLOW: "\x1b[93m",
  BRIGHT_BLUE: "\x1b[94m",
  BRIGHT_MAGENTA: "\x1b[95m",
  BRIGHT_CYAN: "\x1b[96m",
  BRIGHT_WHITE: "\x1b[97m",
  // 样式
  BOLD: "\x1b[1m",
  RESET: "\x1b[0m"
};

// 日志级别
const LogLevel = {
  DEBUG: 0,
  INFO: 1,
  WARNING: 2,
  ERROR: 3
};

// 默认日志级别
let currentLogLevel = process.env.LOG_LEVEL ? 
  LogLevel[process.env.LOG_LEVEL.toUpperCase()] : 
  LogLevel.INFO;

// 日志配置
const levelConfig = {
  [LogLevel.DEBUG]: { color: Color.BRIGHT_BLACK, prefix: "DEBUG" },
  [LogLevel.INFO]: { color: Color.BRIGHT_GREEN, prefix: "INFO" },
  [LogLevel.WARNING]: { color: Color.BRIGHT_YELLOW, prefix: "WARN" },
  [LogLevel.ERROR]: { color: Color.BRIGHT_RED, prefix: "ERROR" }
};

function setLogLevel(level) {
  if (LogLevel[level.toUpperCase()] !== undefined) {
    currentLogLevel = LogLevel[level.toUpperCase()];
  } else {
    console.error(`Invalid log level: ${level}`);
  }
}

function log(level, message) {
  if (level < currentLogLevel) {
    return;
  }

  const config = levelConfig[level];
  const prefix = `[${config.prefix}]`;
  const coloredMessage = `${config.color}${prefix} ${message}${Color.RESET}`;
  console.log(coloredMessage);
}

// 日志方法
function debug(message) {
  log(LogLevel.DEBUG, message);
}

function info(message) {
  log(LogLevel.INFO, message);
}

function warning(message) {
  log(LogLevel.WARNING, message);
}

function error(message) {
  log(LogLevel.ERROR, message);
}

// 服务状态
function serviceStatus(serviceName, isRunning, details, statusType) {
  statusType = statusType || (isRunning ? "success" : "error");
  const statusText = isRunning ? "已运行" : "已停止";
  
  let color;
  if (statusType === "success") {
    color = Color.GREEN;
  } else if (statusType === "error") {
    color = Color.RED;
  } else if (statusType === "warning") {
    color = Color.YELLOW;
  } else {
    color = Color.RESET;
  }

  let message;
  if (details) {
    message = `[${color}${serviceName} ${statusText}${Color.RESET}] ${details}`;
  } else {
    message = `[${color}${serviceName} ${statusText}${Color.RESET}]`;
  }

  // 根据状态确定日志级别
  const level = isRunning ? LogLevel.INFO : LogLevel.WARNING;
  log(level, message);
}

// 对话文本应该使用默认颜色（白色），而不是绿色
function logText(text) {
  console.log(text);
}

module.exports = {
  setLogLevel,
  debug,
  info,
  warning,
  error,
  serviceStatus,
  logText,
  LogLevel
}; 