// Add in achievments later

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

const userSchema = new mongoose.Schema({
  username: {
    type: String,
    required: true,
    minlength: USERNAME_MIN_LENGTH,
    maxlength: USERNAME_MAX_LENGTH,
    unique: true,
    index: true,
  },
  password: {
    type: String,
    required: true,
    minlength: PASSWORD_MIN_LENGTH,
    // no maxLength here because after hashing the password will be very long
  },
  email: {
    type: String,
    required: true,
    minlength: EMAIL_MIN_LENGTH,
    maxlength: EMAIL_MAX_LENGTH,
    unique: true,
    index: true,
  },
  created_date: {
    type: Date,
    default: Date.now,
  },
  is_admin: {
    type: Boolean,
    default: false,
  },
  bio: {
    type: String,
    maxlength: BIO_MAX_LENGTH,
    default: "",
  },
  times_on_hottest_page: {
    type: Number,
    min: 0,
    default: 0,
  },
  total_dislikes: {
    type: Number,
    min: 0,
    default: 0,
  },
  total_likes: {
    type: Number,
    min: 0,
    default: 0,
  },
  display_name: {
    type: String,
    maxlength: DISPLAY_NAME_MAX_LENGTH,
    default: "Anonymous",
  },
});

module.exports = mongoose.model("User", userSchema);
