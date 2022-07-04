const router = require("express").Router();
const { ObjectId } = require("mongodb");
const authenticateToken = require("../lib/auth/authenticateToken");
const Post = require("../models/Post");
const { NUMBER_OF_POSTS_TO_RETURN_PER_CALL } = require("../constants/setup");
const getTokenFromAuthHeader = require("../lib/auth/getTokenFromAuthHeader");
const jwt = require("jsonwebtoken");

router.get("/", authenticateToken, (req, res) => {
  const userID = req.user.userID;
  res.status(200).json(userID);
});

router.post("/create", authenticateToken, async (req, res) => {

    // Retrieves expected values from frontend.
    const { body, genre, university, faculty, replying_to_post_ID } = req.body;

    // Ensures sending nothing doesn't crash the server
    if (body == null || genre == null || university == null || faculty == null) return res.status(400).json({"error": "fields cannot be blank"});

    // Gets the access token from the authorization header (from request).
    const accessToken = getTokenFromAuthHeader(req);

    // Decrypts access token, and gets the user's ID from it.
    var userWhoSentPostID;
    jwt.verify(accessToken, process.env.ACCESS_TOKEN_SECRET, async (e, decryptedToken) => {
        if (e) return res.status(403).send("Token tampered with");
        // Returns the userID (_id from user doc in MongoDB) that is associated with this access token.
        userWhoSentPostID = decryptedToken.userID;
    });

    try {
        // Create post with fields filled all posts will have.
        const post = new Post({
            user_ID: ObjectId(userWhoSentPostID), // Decrypted from accesstoken.
            university,
            genre,
            faculty,
            text: body,
        });

        // If the post is replying to another post, add the field with a ref ID to the other post.
        if (replying_to_post_ID != null && replying_to_post_ID.length !== 0) {
            post.replying_to_post = ObjectId(replying_to_post_ID);
        }

        // Save the post to DB.
        await post.save();
        console.log("Successfully created post.");
    } catch (e) {
        console.log("ERRROR CAUGHT: " + e);
    }

});

router.post("/test", authenticateToken, async (req, res) => {

    const posts = await Post.find({}).populate("replying_to_post").limit(2);
    console.log(posts);
    return res.status(200).json({"message": "reached test endpoint"});
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