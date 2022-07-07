const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const { create, recents, vote } = require("../controllers/postsController");

// Creates a post
router.post("/create", authenticateToken, create);

// Retrieves the newest posts
router.post("/recents", authenticateToken, recents);

// Votes on a specified post (-1 or 1)
router.post("/vote", authenticateToken, vote);

module.exports = router;
