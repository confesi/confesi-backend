const mongoose = require("mongoose");
const {
  POST_MAX_LENGTH,
  MIN_YEAR,
  MAX_YEAR,
} = require("../config/constants/feed");
const { post } = require("../routes/posts");

const postSchema = new mongoose.Schema({
  user_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
    index: true,
  },
  replying_post_ID: {
    type: mongoose.Schema.Types.ObjectId,
    ref: "Post",
  },
  rank: {
    type: Number,
    default: 0,
    index: true,
  },
  created_date: {
    type: Date,
    default: Date.now,
  },
  university: {
    type: String,
    enum: ["UVIC", "UBC", "SFU"],
    required: true,
  },
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
  reports: {
    type: Number,
    min: 0,
    default: 0,
  },
  text: {
    type: String,
    maxlength: POST_MAX_LENGTH,
    required: true,
  },
  comment_count: {
    type: Number,
    min: 0,
    default: 0,
  },
  like_count: {
    type: Number,
    min: 0,
    default: 0,
  },
  dislike_count: {
    type: Number,
    min: 0,
    default: 0,
  },
});

// postSchema.pre("save", function (next) {
//   console.log("SAVE METHOD CALLED");
//   this.rank = this.university + this.genre;
//   next();
// });

// postSchema.pre("updateOne", function (next) {
//   // pseudo-code with dummied-down algorithm
//   post.likes = posts.likes + 1;
//   post.rank = (post.likes + post.dislikes) / post.created_date;
//   next();
// });

module.exports = mongoose.model("Post", postSchema);
