const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const { create, recents, like } = require("../controllers/postsController");

// Creates a post
router.post("/create", authenticateToken, create);

// Retrieves the newest posts
router.post("/recents", authenticateToken, recents);

// likes a specified post
router.post("/like", authenticateToken, like);

module.exports = router;
