const mongoose = require("mongoose");
const {USERNAME_MAX_LENGTH, USERNAME_MIN_LENGTH, PASSWORD_MIN_LENGTH, EMAIL_MAX_LENGTH, EMAIL_MIN_LENGTH} = require("../constants/setup")

const userSchema = new mongoose.Schema({
    password: {
        type: String,
        required: true,
        minlength: PASSWORD_MIN_LENGTH,
        // no maxLength here because after hashing password will be very long
    },
    email: {
        type: String,
        required: true,
        minlength: EMAIL_MIN_LENGTH,
        maxlength: EMAIL_MAX_LENGTH,
    },
    username: {
        type: String,
        required: true,
        minlength: USERNAME_MIN_LENGTH,
        maxlength: USERNAME_MAX_LENGTH,
    },
    date: {
        type: Date,
        default: Date.now,
    }
});

module.exports = mongoose.model("User", userSchema);