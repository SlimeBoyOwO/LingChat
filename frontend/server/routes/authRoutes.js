const express = require("express");
const router = express.Router();
const authController = require("../controllers/authController");
const { checkAdminAuth } = require("../middlewares/authMiddleware");

router.post("/login", authController.login);
router.post("/register", authController.register);
router.post("/reset-password", authController.resetPassword);

module.exports = router;
