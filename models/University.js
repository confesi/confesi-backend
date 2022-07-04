const mongoose = require("mongoose");
const {USERNAME_MAX_LENGTH, USERNAME_MIN_LENGTH, PASSWORD_MIN_LENGTH, EMAIL_MAX_LENGTH, EMAIL_MIN_LENGTH, DISPLAY_NAME_MAX_LENGTH, BIO_MAX_LENGTH} = require("../constants/setup");
  
const universitySchema = new mongoose.Schema({
    name: {
        type: String,
        required: true,
        unique: true,
        index: true,
    },
    school_code: {
        type: String,
        required: true,
        unique: true,
        index: true,
    },
    email_suffix: {
        type: String,
        required: true,
    },
    location: {
        type: {type: String},
        coordinates: [Number]
    },
});

module.exports = mongoose.model("University", universitySchema);
