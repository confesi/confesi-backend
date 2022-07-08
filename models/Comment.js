const mongoose = require("mongoose");
const { COMMENT_MAX_LENGTH } = require("../constants/feed");

// Schema for comments. Comments reside underneath posts.
// They will be threaded in nature (like Reddit).

const commentSchema = new mongoose.Schema({
  // Example: Reference ID
  post_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
    index: true,
  },
  // Example: Reference ID
  parent_ID: {
    type: mongoose.Schema.Types.ObjectId,
    default: null,
    index: true,
  },
  // Example: Reference ID
  author_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
  },
  // Example: Mystery man 9000
  // This could be the username or display name of the user... not sure yet.
  // So far, we think posts are private, comments are public.
  // Must be updated whenever user changes their name?
  author_display_name: {
    type: String,
    required: true,
    maxlength: DISPLAY_NAME_MAX_LENGTH,
  },
  // Example: true
  // Ties into "author_display_name". If true, we could make it "Anonymous", if they
  // want their comment to be public, then it could be their username or display name.
  is_anonymous: {
    type: Boolean,
    default: false,
  },
  // Example: That was the most relatable post ever! Gasp!
  text: {
    type: String,
    maxlength: COMMENT_MAX_LENGTH,
    required: true,
  },
  // Example: 2022-06-20T04:17:43.720+00:00
  posted_at: {
    type: Date,
    default: Date.now,
  },
});

module.exports = mongoose.model("Comment", commentSchema);
