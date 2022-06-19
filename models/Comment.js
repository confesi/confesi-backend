// Use Mongoose "populate()" to deal with threaded comments? Or "Model Tree Structures" from MongoDB?

const mongoose = require("mongoose");
const { COMMENT_MAX_LENGTH } = require("../constants/setup");
  
const commentSchema = new mongoose.Schema({
    post_ID: {
        type: mongoose.Schema.Types.ObjectId,
        required: true,
    },
    parent_ID: {
        type: mongoose.Schema.Types.ObjectId,
        default: null,
    },
    author_ID: {
        type: mongoose.Schema.Types.ObjectId,
        required: true,
    },
    // must be updated whenever user changes their name?
    author_display_name: {
        type: String,
        required: true,
        maxlength: DISPLAY_NAME_MAX_LENGTH,
    },
    is_anonymous: {
        type: Boolean,
        default: false,
    },
    text: {
        type: String,
        maxlength: COMMENT_MAX_LENGTH,
        required: true,
    },
    posted_at: {
        type: Date,
        default: Date.now,
    },
});

module.exports = mongoose.model("Comment", commentSchema);