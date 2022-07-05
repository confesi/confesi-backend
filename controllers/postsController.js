const { ObjectId } = require("mongodb");
const Post = require("../models/Post");
const {
  NUMBER_OF_POSTS_TO_RETURN_PER_CALL,
} = require("../config/constants/feed");
const getTokenFromAuthHeader = require("../utils/auth/getTokenFromAuthHeader");
const jwt = require("jsonwebtoken");

// CREATE POST //

const create = async (req, res) => {
  // Retrieves expected values from frontend.
  const { text, genre, year, university, faculty, replying_post_ID } = req.body;

  // Ensures sending nothing doesn't crash the server
  if (
    text == null ||
    genre == null ||
    university == null ||
    faculty == null ||
    year == null
  )
    return res.status(400).json({ error: "fields cannot be blank" });

  // Gets the access token from the authorization header (from request).
  const accessToken = getTokenFromAuthHeader(req);

  // Decrypts access token, and gets the user's ID from it.
  var userWhoSentPostID;
  jwt.verify(
    accessToken,
    process.env.ACCESS_TOKEN_SECRET,
    async (e, decryptedToken) => {
      if (e) return res.status(403).send("Token tampered with");
      // Returns the userID (_id from user doc in MongoDB) that is associated with this access token.
      userWhoSentPostID = decryptedToken.userID;
    }
  );

  try {
    // Create post with fields filled all posts will have.
    const post = new Post({
      user_ID: ObjectId(userWhoSentPostID), // Decrypted from accesstoken.
      university,
      genre,
      year,
      faculty,
      text,
    });

    // If the post is replying to another post, add the field with a ref ID to the other post.
    if (replying_post_ID != null && replying_post_ID.length !== 0) {
      post.replying_post_ID = ObjectId(replying_post_ID);
    }

    // Save the post to DB.
    await post.save();
    console.log("Successfully created post.");
  } catch (e) {
    console.log("ERROR CAUGHT: " + e);
  }
};

// RETRIEVE RECENT POSTS //

const recents = async (req, res) => {
  // Retrieves expected values from frontend.
  const { last_post_viewed_ID } = req.body;

  // Validates the needed fields exist
  if (last_post_viewed_ID == null || last_post_viewed_ID.length === 0)
    return res.status(400).json({ error: "fields cannot be blank/empty" });

  // Retrieves posts chronologically (newest first).
  // Finds posts: less than passed ID (more recent), sorts by _id (newest first), populates "replying_post_ID" field, and limits returned posts by CONSTANT.
  try {
    const foundPosts = await Post.find({
      _id: { $lt: ObjectId(last_post_viewed_ID) },
    })
      .sort({ _id: -1 })
      .populate("replying_post_ID")
      .limit(NUMBER_OF_POSTS_TO_RETURN_PER_CALL);
    return res.status(200).json({ foundPosts });
  } catch (error) {
    return res.status(500).json({ error: "unknown error" });
  }
};

// RETRIEVE DAILY HOTTEST POSTS //

const dailyHottest = async (req, res) => {
  return res.status(200).json({ msg: "test" });
};

module.exports = { create, recents, dailyHottest };
