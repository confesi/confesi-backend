const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const {
  create,
  recents,
  dailyHottest,
} = require("../controllers/postsController");

// Creates a post
router.post("/create", authenticateToken, create);

// Retrieves the newest posts
router.post("/recents", authenticateToken, recents);

// Returns the daily hottest posts
router.post("/dailyHottest", authenticateToken, dailyHottest);

module.exports = router;
