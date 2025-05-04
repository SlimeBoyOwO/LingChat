const express = require("express");
const router = express.Router();
const path = require("path");

router.get("/", (req, res) => {
  res.sendFile(path.join(__dirname, "../../public/pages/index.html"));
});

router.get("/login", (req, res) => {
  res.sendFile(path.join(__dirname, "../../public/pages/login.html"));
});

router.get("/about", (req, res) => {
  res.sendFile(path.join(__dirname, "../../public/pages/about.html"));
});

module.exports = router;
