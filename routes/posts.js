const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const { create, test, retrieve } = require("../controllers/postsController");

router.route("/create", authenticateToken).post(create);

router.route("/test", authenticateToken).post(test);

router.route("/retrieve", authenticateToken).post(retrieve);

module.exports = router;
