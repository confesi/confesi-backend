const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const { users, universities } = require("../controllers/searchController");

// Searches all the current users by username.
router.post("/users", authenticateToken, users);

// Searches all the current universities supported by
// their extended name (ex: University of Victoria; not UVic).
router.post("/universities", authenticateToken, universities);

module.exports = router;
