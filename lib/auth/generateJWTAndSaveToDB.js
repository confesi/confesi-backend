const jwt = require('jsonwebtoken');
const generateAccessToken = require("../auth/generateAccessToken");
const ObjectID = require('mongodb').ObjectId;
const RefreshToken = require("../../models/RefreshToken");
const { REFRESH_TOKEN_LIFETIME } = require("../../constants/setup");

// Given the user that is created in the DB upon registration, this function
// creates an access and refresh token for it and saves them in the DB. If there is an error, it'll return null
// for both fields which can then be handled to redirect user to login page if creation succeeds, but we can't generate their tokens (rare case?)
async function generateJWTAndSaveToDB(user)  {
    try {
        // Generate jwts
        const accessToken = generateAccessToken(ObjectID(user._id));
        const refreshToken = jwt.sign({userMongoObjectID: ObjectID(user._id)}, process.env.REFRESH_TOKEN_SECRET, { expiresIn: REFRESH_TOKEN_LIFETIME });
        // Save to DB
        const token = new RefreshToken({
            token: refreshToken,
            userID: ObjectID(user._id)
        });
        await token.save();
        return {accessToken: accessToken, refreshToken: refreshToken};
    } catch (e) {
        return  {accessToken: null, refreshToken: null};
    }
}

module.exports = generateJWTAndSaveToDB;