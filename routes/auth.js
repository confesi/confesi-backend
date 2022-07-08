const router = require("express").Router();
const {
  register,
  login,
  token,
  logout,
  logoutAll,
} = require("../controllers/authController");

// Creates a user account.
router.post("/register", register);

// Logs the user in.
router.post("/login", login);

// Fetches user's a new access token based on their sent
// refresh token.
router.post("/token", token);

// Logs the user out. Should this route require authentication?
router.delete("/logout", logout);

// Logs the user out of all devices they are
// logged in on (takes time because it just cancels
// the refresh token in DB). Should this
// route require authentication?
router.delete("/logoutall", logoutAll);

module.exports = router;
