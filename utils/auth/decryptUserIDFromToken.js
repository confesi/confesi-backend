const jwt = require("jsonwebtoken");

async function decryptUserIDFromToken(accessToken) {
  // Decrypts access token, and gets the user's ID from it.
  var userWhoSentPostID;
  jwt.verify(
    accessToken,
    process.env.ACCESS_TOKEN_SECRET,
    async (e, decryptedToken) => {
      if (e) throw "Token tampered with";
      // Returns the userID (_id from user doc in MongoDB) that is associated with this access token.
      userWhoSentPostID = decryptedToken.userID;
    }
  );
  return userWhoSentPostID;
}

module.exports = decryptUserIDFromToken;
