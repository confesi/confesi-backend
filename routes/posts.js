const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const {
  create,
  recents,
  trending,
  vote,
} = require("../controllers/postsController");

// Creates a post.
router.post("/create", authenticateToken, create);

// Retrieves newest posts.
router.post("/recents", authenticateToken, recents);

// Retrieves trending posts.
router.post("/trending", authenticateToken, trending);

// Votes on a specified post (-1, 0, or 1).
router.post("/vote", authenticateToken, vote);

module.exports = router;
