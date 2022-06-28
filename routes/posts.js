const router = require("express").Router();
const { ObjectId } = require("mongodb");
const authenticateToken = require("../lib/auth/authenticateToken");
const Post = require("../models/Post");
const { NUMBER_OF_POSTS_TO_RETURN_PER_CALL } = require("../constants/setup");

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
            text: req.body.body,
            university: "UVIC"
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

    // Retrieves expected values from frontend.
    const { returnDailyPosts, lastPostViewedID } = req.body;

    // Validates none of them are null;
    if (returnDailyPosts == null || lastPostViewedID == null) return res.status(400).json({"error": "fields cannot be blank and must be inside bounds"});

    // Retreives posts chronologically (newest first)
    foundPosts = await Post.find(lastPostViewedID ? {_id: { $lt: ObjectId(lastPostViewedID) }} : null).sort({_id: -1}).limit(NUMBER_OF_POSTS_TO_RETURN_PER_CALL);
     
    var foundDailyPosts;
    if (returnDailyPosts) {
        foundDailyPosts = await Post.find().limit(3);    
    }

    // var testPosts = [{"genre": "relationships", "faculty": "engineering"}, {"genre": "politics", "faculty": "comp sci"}];

    returnDailyPosts ? res.status(200).json({"posts": foundPosts, "dailyPosts": foundDailyPosts}) : res.status(200).json({"posts": foundPosts});

});

module.exports = router; 