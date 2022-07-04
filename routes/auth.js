const router = require("express").Router();
const {
  register,
  login,
  token,
  logout,
  logoutAll,
} = require("../controllers/authController");

router.route("/register").post(register);

router.route("/login").post(login);

router.route("/token").post(token);

router.route("/logout").delete(logout);

router.route("/logoutall").delete(logoutAll);

module.exports = router;
