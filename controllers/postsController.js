const { ObjectId } = require("mongodb");
const Post = require("../models/Post");
const {
  NUMBER_OF_POSTS_TO_RETURN_PER_CALL,
} = require("../config/constants/feed");
const getTokenFromAuthHeader = require("../utils/auth/getTokenFromAuthHeader");
const jwt = require("jsonwebtoken");
const Vote = require("../models/Vote");
const decryptUserIDFromToken = require("../utils/auth/decryptUserIDFromToken");

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
  const userWhoSentPostID = await decryptUserIDFromToken(accessToken);

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
  // Finds posts: less than passed ID (more recent), sorts by _id (newest first), populates "replying_post_ID"
  // field, and limits returned posts by a CONSTANT.
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

// VOTE ON A POST //

const vote = async (req, res) => {
  // Retrieves expected values from frontend.
  const { post_ID, newVoteValue } = req.body;

  // Validates the needed fields exist
  if (
    post_ID == null ||
    (newVoteValue != -1 && newVoteValue != 0 && newVoteValue != 1)
  ) {
    return res
      .status(400)
      .json({ error: "fields cannot be blank/empty/invalid" });
  }

  // Gets the access token from the authorization header (from request).
  const accessToken = getTokenFromAuthHeader(req);

  // Decrypts access token, and gets the user's ID from it.
  const user_ID = await decryptUserIDFromToken(accessToken);

  // Searches if the vote for this post already exists by this user
  const foundVote = await Vote.findOne({
    $and: [{ user_ID: user_ID }, { post_ID: post_ID }],
  });

  if (!foundVote) {
    console.log(`no vote found, now voting: ${newVoteValue}`);
    const newVote = new Vote({
      value: newVoteValue,
      post_ID,
      user_ID,
    });
    await newVote.save();
  } else {
    const result = await Vote.findOneAndUpdate(
      { post_ID, user_ID },
      [
        {
          $set: {
            value: { $ifNull: ["$value", 0] },
          },
        },
        {
          $set: {
            value: newVoteValue,
          },
        },
      ],
      {
        projection: { value: 1 },
        upsert: true,
      }
    );

    // Old vote value found from update query
    const oldVoteValue = result.value;

    // Set upvote/downvote by amount to 0 (to be changed below)
    var changeVoteAmount = 0;

    // If the vote changes, update the "changeVoteAmount"
    // "newVoteValue" is passed in request
    if (newVoteValue !== oldVoteValue) {
      if (oldVoteValue === -1 && newVoteValue === 0) {
        // add 1
        changeVoteAmount = 1;
      } else if (oldVoteValue === -1 && newVoteValue === 1) {
        // add 2
        changeVoteAmount = 2;
      } else if (oldVoteValue === 0 && newVoteValue === -1) {
        // subtract 1
        changeVoteAmount = -1;
      } else if (oldVoteValue === 0 && newVoteValue === 1) {
        // add 1
        changeVoteAmount = 1;
      } else if (oldVoteValue === 1 && newVoteValue === -1) {
        // subtract 2
        changeVoteAmount = -2;
      } else if (oldVoteValue === 1 && newVoteValue === 0) {
        // subtract 1
        changeVoteAmount = -1;
      }
      // Update vote count of post, return post with updated vote count
      const updatedVotePost = await Post.findOneAndUpdate(
        {
          _id: ObjectId(post_ID),
        },
        { $inc: { votes: changeVoteAmount } },
        { new: true }
      );

      // Rank function to update rank on post after post has been atomicaly voted on
      updatedVotePost.rank = updatedVotePost.votes * 2 + 1;

      // Save post with updated rank (and from earlier, new updated vote count)
      const updatedRankPost = await updatedVotePost.save();

      // Return new updated post
      res.status(200).json({ post: updatedRankPost });
    } else {
      // Post already has vote that user tried to cast
      res.status(400).json({ msg: "new vote same as old vote" });
    }
  }
};

module.exports = { create, recents, vote };
