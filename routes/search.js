const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const { users, universities } = require("../controllers/searchController");

router.route("/users", authenticateToken).post(users);

router.route("/universities", authenticateToken).post(universities);

module.exports = router;
