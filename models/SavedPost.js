const mongoose = require("mongoose");

// Schema for when a user saves posts to their profile. Not
// implemented yet. This feature will be like Instagram's saved, except
// it will have the option of being public or private.

const savedPostSchema = new mongoose.Schema({
  // Example: Reference ID
  user_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
    index: true,
  },
  // Example: Reference ID
  post_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
  },
  // Example: 2022-06-20T04:17:43.720+00:00
  saved_at_date: {
    type: Date,
    default: Date.now,
  },
});

module.exports = mongoose.model("SavedPost", savedPostSchema);
