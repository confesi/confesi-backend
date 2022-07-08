const {
  USERNAME_MAX_LENGTH,
  USERNAME_MIN_LENGTH,
  EMAIL_MAX_LENGTH,
  EMAIL_MIN_LENGTH,
  PASSWORD_MAX_LENGTH,
  PASSWORD_MIN_LENGTH,
} = require("../../config/constants/auth");

// TODO: Add profanity checking. User's should probably not be
// allowed to have profanity in their emails, usernames, or passwords!

// Validates that emails, usernames, and passwords are the correct
// length and are of the correct format.
function registerValidation(email, username, password) {
  // Email validation
  if (!validateEmail(email)) {
    return "invalid email";
  } else if (email.length > EMAIL_MAX_LENGTH) {
    return "email too long";
  } else if (email.length < EMAIL_MIN_LENGTH) {
    return "email too short";
  }
  // Username validation
  if (!username.match("^[a-zA-Z0-9_.-]*$")) {
    return "invalid username format";
  } else if (username.length > USERNAME_MAX_LENGTH) {
    return "username too long";
  } else if (username.length < USERNAME_MIN_LENGTH) {
    return "username too short";
  }
  // Password verification
  if (password.length > PASSWORD_MAX_LENGTH) {
    return "password too long";
  } else if (password.length < PASSWORD_MIN_LENGTH) {
    return "password too short";
  }
  // Passes everything, all good!
  return null;
}

// Regex that basically means: "is the email formatted like an email?"
function validateEmail(email) {
  var re = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return re.test(email);
}

module.exports = registerValidation;
