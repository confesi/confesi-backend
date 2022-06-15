const mongoose = require("mongoose");

const userSchema = new mongoose.Schema({
    password: {
        type: String,
        required: true,
        minlength: 6,
        // no maxLength here because after hashing password will be very long
    },
    email: {
        type: String,
        required: true,
        minlength: 5,
        maxlength: 255,
        // index: {unique: true}
    },
    username: {
        type: String,
        required: true,
        minlength: 3,
        maxlength: 30,
        // index: {unique: true}
    },
    date: {
        type: Date,
        default: Date.now,
    }
});

module.exports = mongoose.model("User", userSchema);