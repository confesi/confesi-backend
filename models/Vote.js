const mongoose = require("mongoose");

// Schema for votes cast (upvotes/downvotes - think Reddit). This
// is used to ensure users only vote for each post once and get update
// their vote if they want (ex: -1 to 1 to 0, etc.).

// Should documents in this collection expire after 7 days? (then no votes can be cast; just
// the overall number is kept) in order to keep the size under control.

const voteSchema = new mongoose.Schema({
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
    index: true,
  },
  // Example: -1
  value: {
    type: Number,
    default: 0,
    validate: {
      validator: function (value) {
        if (value === -1 || value === 0 || value === 1) {
          return true;
        } else {
          return false;
        }
      },
      message: "value must be -1, 0, or 1",
    },
  },
});

module.exports = mongoose.model("Vote", voteSchema);
