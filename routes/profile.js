const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const { watchedUniversities } = require("../controllers/profileController");

router.post("/watchedUniversities", authenticateToken, watchedUniversities);

module.exports = router;
