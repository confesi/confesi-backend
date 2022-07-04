const mongoose = require("mongoose");
  
const savedPostSchema = new mongoose.Schema({
    user_ID: {
        type: mongoose.Schema.Types.ObjectId,
        required: true,
        index: true,
    },
    post_ID: {
        type: mongoose.Schema.Types.ObjectId,
        required: true,
    },
    saved_at_date: {
        type: Date,
        default: Date.now,
    },
});

module.exports = mongoose.model("SavedPost", savedPostSchema);
