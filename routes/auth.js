// what happens when access and/or refresh tokens expire?
// add anti-hacker basic protections
// create logout all route (logs out on all devices)

const router = require("express").Router();
const User = require("../models/User");
const RefreshToken = require("../models/RefreshToken");
const registerValidation = require("../validation/register");
const loginValidation = require("../validation/login");
const bcrypt = require("bcrypt");
const { SALT_ROUNDS, REFRESH_TOKEN_LIFETIME } = require("../constants/setup");
const jwt = require('jsonwebtoken');
const ObjectID = require('mongodb').ObjectId;
const generateAccessToken = require("../lib/auth/generateAccessToken");



router.post("/register", async (req, res) => {

    // Ensures sending nothing doesn't crash the server
    if (req.body.email == null || req.body.username == null || req.body.password == null) return res.status(400).send("Request must include data");

    // Trim off white space, and set to lowercase
    const username = req.body.username.replace(/ /g,'').toLowerCase();
    const email = req.body.email.replace(/ /g,'').toLowerCase();

    // Make sure username only contains letters, numbers, dashes, and underscores
    if (!username.match("^[a-zA-Z0-9_.-]*$")) return res.status(400).send("Username can only contain letters, numbers, dashes, and underscores");

    // Validate
    const { error } = registerValidation(req.body);
    if (error) return res.status(400).send(error.details[0].message);

    // Checking is user already exists (username & email)
    try {
        const usernameExists = await User.findOne({username: username});
        const emailExists = await User.findOne({email: email});
        if (usernameExists && emailExists) return res.status(400).send("Username and email already taken");
        if (emailExists) return res.status(400).send("Email already taken");
        if (usernameExists) return res.status(400).send("Username already taken");
    } catch (e) {
        return res.status(500).send("Error querying DB to check if username/email exists or not");
    }

    // Hashing user's password
    try {
        const hashedPassword = await bcrypt.hash(req.body.password, SALT_ROUNDS);
        // Submitting user to DB
        const user = new User({
            username: username,
            email: email,
            password: hashedPassword,
        });
        const savedUser = await user.save();
        res.send(`${savedUser.username} successfully created`);
    } catch (e) {
        return res.status(500).send("Error hashing password or submitting user to DB");
    }
});

router.post("/login", async (req, res) => {

    // Ensures sending nothing doesn't crash the server
    if (req.body.usernameOrEmail == null || req.body.password == null) return res.status(400).send("Request must include data");

    // Trim off white space, and set to lowercase
    const usernameOrEmail = req.body.usernameOrEmail.replace(/ /g,'').toLowerCase();

    // Validate
    const { error } = loginValidation(req.body);
    if (error) return res.status(400).send(error.details[0].message);

     // Checking is account exists (username & email)
     try {
        // Checks if it's an email
        var user;
        if (usernameOrEmail.includes("@")) {
            user = await User.findOne({email: usernameOrEmail});
        } else {
        // Checks if it's a username
        user = await User.findOne({username: usernameOrEmail});
        }
        if (!user) return res.status(400).send("Account (username or email) doesn't exist");

        // Checking if password is correct for that account
        const validPassword = await bcrypt.compare(req.body.password, user.password);
        if (!validPassword) return res.status(400).send("Invalid password");
        // Generate jwts
        const accessToken = generateAccessToken(ObjectID(user._id));
        const refreshToken = jwt.sign({userMongoObjectID: ObjectID(user._id)}, process.env.REFRESH_TOKEN_SECRET, { expiresIn: REFRESH_TOKEN_LIFETIME });
        await RefreshToken.findOne({userID: ObjectID(user._id)});
            const token = new RefreshToken({
                token: refreshToken,
                userID: ObjectID(user._id)
            });
            await token.save();
        res.status(200).json({ accessToken: accessToken, refreshToken: refreshToken });
    }
    catch (e) {
        return res.status(500).send("ERROR: " + e);
    }

});

router.post("/token", async (req, res) => {
    console.log("TOKEN ROUTE CALLED");
    // UNCOMMENT THE LINE BELOW TO TEST NEW USERS (and comment everything else to avoid crash)
    // res.status(402).send("temporary testing");
    const refreshToken = req.body.token;
    if (!refreshToken) return res.status(401).send("No refresh token given");
    jwt.verify(refreshToken, process.env.REFRESH_TOKEN_SECRET, async (e, user) => {
        if (e) return res.status(403).send("Token tampered with");
        // check if given token is in its respective user's token field, if not, return "no access!"
        const foundRefreshToken = await RefreshToken.findOne({userID: ObjectID(user.userMongoObjectID), token: refreshToken});
        if (!foundRefreshToken?.token) return res.status(403).send("Refresh token not found in DB");
        if (foundRefreshToken.token !== refreshToken) return res.status(403).send("Refresh token and one from DB don't match");
        const accessToken = generateAccessToken(ObjectID(user.userMongoObjectID));
        res.status(200).json({accessToken});
    });
});

router.delete("/logout", (req, res) => {
    console.log("/LOGOUT route CALLED FROM SERVER!!!!!");
    // UNCOMMENT LINE BELOW TO SIMULATE ERROR (and comment everything else)
    // return res.status(500).send("Could not delete refresh token provided from DB");
    const refreshToken = req.body.token;
    jwt.verify(refreshToken, process.env.REFRESH_TOKEN_SECRET, async (e, user) => {
        if (e) return res.status(403).send("Token tampered with or expired");
        try {
            await RefreshToken.findOneAndDelete({userID: ObjectID(user.userMongoObjectID), token: refreshToken});
        }
        catch (error) {
            return res.status(500).send("Could not delete refresh token provided from DB");
        }
        res.status(200).send("Succesfully logged out");
    });
});

// takes a few minutes as each device needs to call for new access token and discover the refresh token isn't in the db anymore, then it'll log out
router.delete("/logoutall", (req, res) => {
    const refreshToken = req.body.token;
    jwt.verify(refreshToken, process.env.REFRESH_TOKEN_SECRET, async (e, user) => {
        if (e) return res.status(403).send("Token tampered with");
        try {
            // removes all refresh tokens in the database corresponding to the user (on next call from their devices it'll log them out)
            await RefreshToken.remove({userID: ObjectID(user.userMongoObjectID)});
        }
        catch (error) {
            return res.status(500).send("Could not delete refresh token provided from DB");
        }
        res.status(200).send("Succesfully logged out");
    });
});

module.exports = router;