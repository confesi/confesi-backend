const { ACCESS_TOKEN_LIFETIME } = require("../../config/constants/auth");
const jwt = require("jsonwebtoken");

// Creates an access token that houses the user's unique ID.
function generateAccessToken(userID) {
  return jwt.sign({ userID }, process.env.ACCESS_TOKEN_SECRET, {
    expiresIn: ACCESS_TOKEN_LIFETIME,
  });
}

module.exports = generateAccessToken;
