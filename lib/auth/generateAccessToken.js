const { ACCESS_TOKEN_LIFETIME } = require("../../constants/setup");
const jwt = require('jsonwebtoken');


function generateAccessToken(userID) {
    return jwt.sign({userID}, process.env.ACCESS_TOKEN_SECRET, { expiresIn: ACCESS_TOKEN_LIFETIME })
}

module.exports = generateAccessToken;