const mongoose = require("mongoose");
const {
  DISPLAY_NAME_MAX_LENGTH,
  BIO_MAX_LENGTH,
} = require("../config/constants/profile");
const {
  EMAIL_MAX_LENGTH,
  USERNAME_MAX_LENGTH,
  USERNAME_MIN_LENGTH,
  PASSWORD_MIN_LENGTH,
  EMAIL_MIN_LENGTH,
} = require("../config/constants/auth");

// Schema for a user (achievments should be added later). This has all the
// user-centric data.

const userSchema = new mongoose.Schema({
  // Example: matthew
  username: {
    type: String,
    required: true,
    minlength: USERNAME_MIN_LENGTH,
    maxlength: USERNAME_MAX_LENGTH,
    unique: true,
    index: true,
  },
  // Example: password123#!@
  password: {
    type: String,
    required: true,
    minlength: PASSWORD_MIN_LENGTH,
    // no maxLength here because after hashing the password will be very long
  },
  // Example: matthew@example.com
  email: {
    type: String,
    required: true,
    minlength: EMAIL_MIN_LENGTH,
    maxlength: EMAIL_MAX_LENGTH,
    unique: true,
    index: true,
  },
  // Example: 2022-06-20T04:17:43.720+00:00
  created_date: {
    type: Date,
    default: Date.now,
  },
  // Example: false
  is_admin: {
    type: Boolean,
    default: false,
  },
  // Example: Hey, I'm matthew. I go to UVic. This is my bio.
  bio: {
    type: String,
    maxlength: BIO_MAX_LENGTH,
    default: "",
  },
  // Example: 13
  times_on_hottest_page: {
    type: Number,
    min: 0,
    default: 0,
  },
  // Example: 12545
  total_dislikes: {
    type: Number,
    min: 0,
    default: 0,
  },
  // Example: 21823
  total_likes: {
    type: Number,
    min: 0,
    default: 0,
  },
  // Example: Mystery man 9000
  display_name: {
    type: String,
    maxlength: DISPLAY_NAME_MAX_LENGTH,
    default: "Anonymous",
  },
});

module.exports = mongoose.model("User", userSchema);
