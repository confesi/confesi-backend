const router = require("express").Router();
const authenticateToken = require("../lib/auth/authenticateToken");
const Post = require("../models/Post");

router.get("/", authenticateToken, (req, res) => {
  const userID = req.user.userID;
  res.status(200).json(userID);
});

router.post("/create", authenticateToken, async (req, res) => {

    // Ensures sending nothing doesn't crash the server
    if (!req.body.body || !req.body.genre) return res.status(400).json({"error": "fields cannot be blank"});

    // 
    try {
        const post = new Post({
            user_ID: "fake user id",
            faculty: "ENGINEERING",
            genre: "CLASSES",
            text: req.body.body
        });
        await post.save();
        console.log("Succesfully created.");
    } catch (e) {
        console.log("ERRROR CAUGHT: " + e);
    }

    console.log("/CREATE req.body: " + req.body.body);
    return res.status(201).json({"response": req.body.body});
});

module.exports = router; 