const router = require("express").Router();
const authenticateToken = require("../lib/auth/authenticateToken");

router.get("/", authenticateToken, (req, res) => {
    const userID = req.user.userID;
    res.status(200).json(userID);
}); 

module.exports = router