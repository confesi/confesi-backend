const router = require("express").Router();
const authenticateToken = require("../lib/auth/authenticateToken");

router.get("/", authenticateToken, (req, res) => {
    const userID = req.user.userID;
    res.send("SUCCESS, userID = " + userID);
}); 

module.exports = router