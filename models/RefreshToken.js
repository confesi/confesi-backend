const mongoose = require("mongoose");
  
const refreshTokenSchema = new mongoose.Schema({
    user_ID: {
        type: mongoose.Schema.Types.ObjectId,
        required: true,
        index: true,
    },
    refresh_token: {
        type: String,
        required: true,
        unique: true, // Maybe not unique? Across different devices?
    },
    created_date: {
        type: Date,
        default: Date.now,
    },
});

module.exports = mongoose.model("RefreshToken", refreshTokenSchema);
