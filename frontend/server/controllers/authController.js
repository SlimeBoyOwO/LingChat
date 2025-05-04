const {
  getPythonSocket,
  pendingRequests,
} = require("../services/pythonService");

async function handleAuthRequest(req, res, action) {
  const { email, password, username, oldPassword, newPassword } = req.body;

  if (!email || !password) {
    return res.status(400).json({
      success: false,
      message: "Email and password are required",
    });
  }

  const pythonSocket = getPythonSocket();
  if (!pythonSocket || pythonSocket.readyState !== WebSocket.OPEN) {
    return res.status(503).json({
      success: false,
      message: "Backend service unavailable",
    });
  }

  const requestId = Date.now().toString();
  const responsePromise = new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(new Error(`${action} request timed out`));
      delete pendingRequests[requestId];
    }, 10000);

    pendingRequests[requestId] = {
      resolve,
      reject,
      timeout,
    };
  });

  const requestData = {
    type: "auth",
    action,
    email,
    password,
    requestId,
  };

  if (action === "register") {
    requestData.username = username;
  } else if (action === "reset_password") {
    requestData.oldPassword = oldPassword;
    requestData.newPassword = newPassword;
  }

  pythonSocket.send(JSON.stringify(requestData));

  try {
    const response = await responsePromise;
    return res.json(response);
  } catch (error) {
    console.error(`${action} error:`, error);
    return res.status(500).json({
      success: false,
      message: `${action} service unavailable`,
    });
  }
}

module.exports = {
  login: (req, res) => handleAuthRequest(req, res, "login"),
  register: (req, res) => handleAuthRequest(req, res, "register"),
  resetPassword: (req, res) => handleAuthRequest(req, res, "reset_password"),
};
