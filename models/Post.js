const mongoose = require("mongoose");
const {
  POST_MAX_LENGTH,
  MIN_YEAR,
  MAX_YEAR,
} = require("../config/constants/feed");

// Schema for posts. Posts are (obviously) created and posted by the user.
// They can optionally "reply" to another post (like Twitter). In this case,
// they get the populated value of the replied post through a ref.

const postSchema = new mongoose.Schema({
  // Example: Reference ID
  user_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
    index: true,
  },
  // Example: Reference ID
  replying_post_ID: {
    type: mongoose.Schema.Types.ObjectId,
    ref: "Post",
  },
  // Example: 2.321121829440
  rank: {
    type: Number,
    default: 0,
    index: true,
  },
  // Example: 2022-06-20T04:17:43.720+00:00
  created_date: {
    type: Date,
    default: Date.now,
  },
  // Example: UVIC
  university: {
    type: String,
    enum: ["UVIC", "UBC", "SFU"],
    required: true,
  },
  // Example: POLITICS
  genre: {
    type: String,
    enum: [
      "RELATIONSHIPS",
      "POLITICS",
      "CLASSES",
      "GENERAL",
      "OPINIONS",
      "CONFESSIONS",
    ],
    required: true,
  },
  // Example: 3
  year: {
    type: Number,
    required: true,
    min: MIN_YEAR,
    max: MAX_YEAR,
    validate: {
      validator: Number.isInteger,
      message: "year is not an integer value",
    },
  },
  // Example: LAW
  faculty: {
    type: String,
    enum: [
      "LAW",
      "ENGINEERING",
      "FINE_ARTS",
      "COMPUTER_SCIENCE",
      "BUSINESS",
      "EDUCATION",
      "MEDICAL",
      "HUMAN_AND_SOCIAL_DEVELOPMENT",
      "HUMANITIES",
      "SCIENCE",
      "SOCIAL_SCIENCES",
    ],
    required: true,
  },
  // Example: 2
  reports: {
    type: Number,
    min: 0,
    default: 0,
  },
  // Example: This is my super awesome post! Let's do a bunch of gossip, blah, blah! Go Vikes!
  text: {
    type: String,
    maxlength: POST_MAX_LENGTH,
    required: true,
  },
  // Example: 43
  comment_count: {
    type: Number,
    min: 0,
    default: 0,
  },
  // Example: 59
  votes: {
    type: Number,
    validate: {
      validator: Number.isInteger,
      message: "vote is not an integer value",
    },
    default: 0,
  },
});

module.exports = mongoose.model("Post", postSchema);
