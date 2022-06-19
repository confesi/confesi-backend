const mongoose = require("mongoose");
const { POST_MAX_LENGTH } = require("../constants/setup");
  
const postSchema = new mongoose.Schema({
    user_ID: {
        type: mongoose.Schema.Types.ObjectId,
        required: true,
    },
    created_date: {
        type: Date,
        default: Date.now,
    },
    genre: {
        type: String,
        enum: ["RELATIONSHIPS", "POLITICS", "CLASSES", "GENERAL"],
        required: true,
    },
    faculty: {
        type: String,
        enum: ["ENGINEERING", "ARTS", "COMPUTER_SCIENCE", "BUSINESS"],
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

module.exports = mongoose.model("Post", postSchema);
