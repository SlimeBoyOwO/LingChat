const express = require("express");
const router = express.Router();
const envController = require("../controllers/envController");

router.get("/model-info", envController.getModelInfo);

module.exports = router;
