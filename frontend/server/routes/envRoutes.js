const express = require("express");
const router = express.Router();
const envController = require("../controllers/envController");
const { checkAdminAuth } = require("../middlewares/authMiddleware");

router.get("/env-config", checkAdminAuth, envController.getEnvConfig);
router.post("/settings/env", checkAdminAuth, envController.updateEnvConfig);

module.exports = router;
