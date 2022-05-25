// what happens when access and/or refresh tokens expire?
// add anti-hacker basic protections

const router = require("express").Router();
const User = require("../models/User");
const RefreshToken = require("../models/RefreshToken");
const registerValidation = require("../validation/register");
const loginValidation = require("../validation/login");
const bcrypt = require("bcrypt");
const { SALT_ROUNDS, ACCESS_TOKEN_LIFETIME, REFRESH_TOKEN_LIFETIME } = require("../constants/setup");
const jwt = require('jsonwebtoken');
const ObjectID = require('mongodb').ObjectId;
const generateAccessToken = require("../lib/auth/generateAccessToken");



router.post("/register", async (req, res) => {

    // Validate
    const { error } = registerValidation(req.body);
    if (error) return res.status(400).send(error.details[0].message);

    // Checking is user already exists (username)
    try {
        const usernameExists = await User.findOne({username: req.body.username});
        if (usernameExists) return res.status(400).send("Username already taken");
    } catch (e) {
        return res.status(500).send("Error querying DB to check if username exists or not");
    }

    // Hashing user's password
    try {
        const hashedPassword = await bcrypt.hash(req.body.password, SALT_ROUNDS);
        // Submitting user to DB
        const user = new User({
            username: req.body.username,
            email: req.body.email,
            password: hashedPassword,
        });
        const savedUser = await user.save();
        res.send(`${savedUser.username} successfully created`);
    } catch (e) {
        return res.status(500).send("Error hashing password or submitting user to DB");
    }
});

router.post("/login", async (req, res) => {

    // Validate
    const { error } = loginValidation(req.body);
    if (error) return res.status(400).send(error.details[0].message);

     // Checking is account exists (username)
     try {
        const user = await User.findOne({username: req.body.username});
        if (!user) return res.status(400).send("Account (username) doesn't exist");

        // Checking if password is correct for that account
        const validPassword = await bcrypt.compare(req.body.password, user.password);
        if (!validPassword) return res.status(400).send("Invalid password");
        // Generate jwts
        const accessToken = generateAccessToken(ObjectID(user._id));
        const refreshToken = jwt.sign({userMongoObjectID: ObjectID(user._id)}, process.env.REFRESH_TOKEN_SECRET, { expiresIn: REFRESH_TOKEN_LIFETIME });
        const foundRefreshToken = await RefreshToken.findOne({userID: ObjectID(user._id)});
        if (!foundRefreshToken) {
            const token = new RefreshToken({
                token: refreshToken,
                userID: ObjectID(user._id)
            });
            await token.save();
        } else {
            await RefreshToken.findOneAndUpdate({userID: ObjectID(user._id)},{token: refreshToken},{new: true});
        }
        // res.status(200).json({ accessToken: accessToken, refreshToken: refreshToken, username: user.username, refreshTokenInDB });
        res.status(200).json({ accessToken: accessToken, refreshToken: refreshToken });
    }
    catch (e) {
        return res.status(500).send("ERROR: " + e);
    }


});

router.post("/token", async (req, res) => {
    const refreshToken = req.body.token;
    if (!refreshToken) return res.status(401).send("No refresh token given");
    jwt.verify(refreshToken, process.env.REFRESH_TOKEN_SECRET, async (e, user) => {
        if (e) return res.status(403).send("Token tampered with");
        // check if given token is in its respective user's token field, if not, return "no access!"
        const foundRefreshToken = await RefreshToken.findOne({userID: ObjectID(user.userMongoObjectID)});
        if (!foundRefreshToken?.token) return res.status(403).send("Refresh token not found in DB");
        if (foundRefreshToken.token !== refreshToken) return res.status(403).send("Refresh token given and one from DB don't match");
        const accessToken = generateAccessToken(ObjectID(user.userMongoObjectID));
        res.json({accessToken});
    });
});

router.delete("/logout", (req, res) => {
    const refreshToken = req.body.token;
    jwt.verify(refreshToken, process.env.REFRESH_TOKEN_SECRET, async (e, user) => {
        if (e) return res.status(403).send("Token tampered with");
        try {
            await RefreshToken.findOneAndDelete({userID: ObjectID(user.userMongoObjectID)});
        }
        catch (error) {
            return res.status(500).send("Could not delete refresh token provided from DB");
        }
        res.status(200).send("Succesfully logged out");
    });
})

module.exports = router;