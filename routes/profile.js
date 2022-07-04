const router = require("express").Router();
const authenticateToken = require("../middlewares/authenticateToken");
const { watchedUniversities } = require("../controllers/profileController");

router
  .route("/watchedUniversities", authenticateToken)
  .post(watchedUniversities);

module.exports = router;
