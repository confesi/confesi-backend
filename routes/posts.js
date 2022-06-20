const router = require("express").Router();
const { ObjectId } = require("mongodb");
const authenticateToken = require("../lib/auth/authenticateToken");
const Post = require("../models/Post");

router.get("/", authenticateToken, (req, res) => {
  const userID = req.user.userID;
  res.status(200).json(userID);
});

router.post("/create", authenticateToken, async (req, res) => {

    // Ensures sending nothing doesn't crash the server
    if (!req.body.body || !req.body.genre) return res.status(400).json({"error": "fields cannot be blank"});

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

// could someone technically get their access token and just send via postman to this address and change the fields they want?
router.post("/retrieve", authenticateToken, async (req, res) => {

    // Ensures sending nothing doesn't crash the server.
    if (!req.body.number_of_posts || req.body.number_of_posts > 50) return res.status(400).json({"error": "fields cannot be blank and must be inside bounds"});

    var last_post_viewed_id;
    if (!req.body.last_post_viewed_id) {
        last_post_viewed_id = "000000000000000000000000"; // simulates starting from the beginning
    } else {
        last_post_viewed_id = req.body.last_post_viewed_id;
    }

    const foundPosts = await Post.find({_id: { $gt: ObjectId(last_post_viewed_id) }}).limit(req.body.number_of_posts);    

    res.status(200).json({"posts": foundPosts});
});

module.exports = router; 