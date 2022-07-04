const jwt = require("jsonwebtoken");

async function getUserIDFromAccessToken(res, accessToken) {
    jwt.verify(accessToken, process.env.ACCESS_TOKEN_SECRET, async (e, decryptedToken) => {
        if (e) return res.status(403).send("Token tampered with");
        // Returns the userID (_id from user doc in MongoDB) that is associated with this access token
        return "hello";
    });
}

module.exports = getUserIDFromAccessToken;