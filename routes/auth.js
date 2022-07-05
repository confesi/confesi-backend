const router = require("express").Router();
const {
  register,
  login,
  token,
  logout,
  logoutAll,
} = require("../controllers/authController");

router.post("/register", register);

router.post("/login", login);

router.post("/token", token);

// Logs the user out, should this route require authentication?
router.delete("/logout", logout);

// Logs the user out of all devices they are logged in on (takes time because it just cancels the refresh token in DB), should this route require authentication?
router.delete("/logoutall", logoutAll);

module.exports = router;
