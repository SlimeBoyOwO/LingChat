function checkAdminAuth(req, res, next) {
  const authHeader = req.headers.authorization;

  if (!authHeader) {
    return res.status(401).json({
      success: false,
      message: "未提供认证信息",
    });
  }

  try {
    const token = authHeader.split(" ")[1];
    if (!token) {
      return res.status(401).json({
        success: false,
        message: "认证令牌无效",
      });
    }

    const userDataString = Buffer.from(token, "base64").toString();
    const userData = JSON.parse(userDataString);

    if (
      !userData ||
      !userData.email ||
      userData.email !== "root" ||
      userData.role !== "admin"
    ) {
      return res.status(403).json({
        success: false,
        message: "没有权限访问此资源",
      });
    }

    next();
  } catch (error) {
    console.error("认证失败:", error);
    return res.status(401).json({
      success: false,
      message: "认证失败",
    });
  }
}

module.exports = {
  checkAdminAuth,
};
