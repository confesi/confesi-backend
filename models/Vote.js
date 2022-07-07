const mongoose = require("mongoose");

// Should documents in this collection expire after 7 days? (then no votes can be cast; just
// the overall number is kept) in order to keep the size under control

const voteSchema = new mongoose.Schema({
  user_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
    index: true,
  },
  post_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
    index: true,
  },
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
