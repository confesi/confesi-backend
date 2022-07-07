const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const { users, universities } = require("../controllers/searchController");

router.post("/users", authenticateToken, users);

router.post("/universities", authenticateToken, universities);

module.exports = router;
