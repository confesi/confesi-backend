const User = require("../models/User");
const RefreshToken = require("../models/RefreshToken");
const registerValidation = require("../utils/validation/register");
const bcrypt = require("bcrypt");
const { SALT_ROUNDS } = require("../config/constants/auth");
const jwt = require("jsonwebtoken");
const ObjectID = require("mongodb").ObjectId;
const generateAccessToken = require("../utils/auth/generateAccessToken");
const generateJWTAndSaveToDB = require("../utils/auth/generateJWTAndSaveToDB");

// REGISTER USER //

const register = async (req, res) => {
  // Retrieves expected values from frontend.
  let { username, email, password } = req.body;

  // Validates the needed fields exist
  if (email == null || username == null || password == null)
    return res.status(400).json({ error: "fields cannot be blank" });

  // Trim off white space, and set to lowercase (frontend should do this, but just in case).
  username = username.replace(/ /g, "").toLowerCase();
  email = email.replace(/ /g, "").toLowerCase();

  // Validates the email, username, and password are all in
  // valid formats. Example: email = "myemail" would fail; email = "myemail@email.com" would pass.
  const error = registerValidation(email, username, password);
  if (error) return res.status(400).json({ error: error });

  // Checking is user already exists (username & email).
  try {
    const usernameExists = await User.findOne({ username: username });
    const emailExists = await User.findOne({ email: email });
    if (usernameExists && emailExists)
      return res.status(400).json({ error: "email and username taken" });
    if (emailExists)
      return res.status(400).json({ error: "email already taken" });
    if (usernameExists)
      return res.status(400).json({ error: "username already taken" });
  } catch (e) {
    return res
      .status(500)
      .send("Error querying DB to check if username/email exists or not");
  }

  // Hashing user's password
  try {
    const hashedPassword = await bcrypt.hash(password, SALT_ROUNDS);
    // Creating and submitting user to DB.
    const user = new User({
      username: username,
      email: email,
      password: hashedPassword,
    });
    const createdUser = await user.save();

    // Generating access token and refresh token. Also saves it to database.
    const { accessToken, refreshToken } = await generateJWTAndSaveToDB(
      createdUser
    );
    // Checks to see if saving the refresh token to the database and generating
    // an access token failed. If so, tell the user their account was created, but they have no
    // tokens so they should login.
    // TODO: Fix this "user created but not tokens" with MongoDB "trasactions"?
    if (accessToken == null || refreshToken == null)
      return res
        .status(400)
        .json({ error: "created user, but not tokens in DB" });
    // If at this point, then user was succesfully created and tokens, so return the tokens.
    res
      .status(201)
      .json({ accessToken: accessToken, refreshToken: refreshToken });
  } catch (e) {
    // Internal server error.
    return res.status(500).json({ error: "error creating user" });
  }
};

// LOG USER IN //

const login = async (req, res) => {
  // Deconstruct fields we need from request.
  let { usernameOrEmail, password } = req.body;

  // Ensures we have what we need from request.
  if (usernameOrEmail == null || password == null)
    return res.status(400).json({ error: "fields cannot be blank" });

  // Trim off white space, and set to lowercase (frontend should do, but just in case).
  usernameOrEmail = usernameOrEmail.replace(/ /g, "").toLowerCase();

  // Checking is account exists (username & email)
  try {
    var user;
    // Checks if it's an email login attempt:
    if (usernameOrEmail.includes("@")) {
      user = await User.findOne({ email: usernameOrEmail });
    } else {
      // Checks if it's a username login attempt:
      user = await User.findOne({ username: usernameOrEmail });
    }
    // If no user is found, then the account doesn't exist.
    if (!user) return res.status(400).json({ error: "account doesn't exist" });

    // Checking if password is correct for a found account.
    const validPassword = await bcrypt.compare(password, user.password);

    // If password from database and from request don't match... the
    // user's password is wrong!
    if (!validPassword)
      return res.status(400).json({ error: "password incorrect" });

    // Generating access token and refresh token. Also saves it to database.
    const { accessToken, refreshToken } = await generateJWTAndSaveToDB(user);

    // Checks to see if saving the refresh token to the database and generating
    // an access token failed. If so, tell the user their account was created, but they have no
    // tokens so they should login.
    // TODO: Fix this "user created but not tokens" with MongoDB "trasactions"?
    if (accessToken == null || refreshToken == null)
      return res.status(500).json({ error: "error getting/savings tokens" });

    // If at this point, then user was succesfully created and tokens, so return the tokens.
    res
      .status(200)
      .json({ accessToken: accessToken, refreshToken: refreshToken });
  } catch (e) {
    // Internal server error.
    return res.status(500).json({ error: "error creating user" });
  }
};

// RETURN USER NEW ACCESS TOKEN //

const token = async (req, res) => {
  // Taking the values we need from request.
  const refreshToken = req.body.token;

  // If refreshToken is null, then return error.
  if (refreshToken == null)
    return res.status(401).json({ error: "No refresh token given" });

  // Verify their refresh token is valid.
  jwt.verify(
    refreshToken,
    process.env.REFRESH_TOKEN_SECRET,
    async (e, user) => {
      // If there's an error it means the token has been tampered with, so return an error message.
      if (e) return res.status(403).json({ error: "Token tampered with" });

      // Check if given token is in its respective user's token field, if not, return "no access!"
      const foundRefreshToken = await RefreshToken.findOne({
        user_ID: ObjectID(user.userMongoObjectID),
        refresh_token: refreshToken,
      });

      // If refresh token is not found in databse, then there's no matching token, so return error. They
      // should login again to put another refresh token into the database.
      if (!foundRefreshToken?.refresh_token)
        return res.status(403).send("Refresh token not found in DB");

      // If the refresh token they've passed up doesn't equal the token in the database, return error.
      if (foundRefreshToken.refresh_token !== refreshToken)
        return res
          .status(403)
          .send("Refresh token and one from DB don't match");

      // If they've passed all the checks so far, generate them a new access token.
      const accessToken = generateAccessToken(ObjectID(user.userMongoObjectID));
      res.status(200).json({ accessToken });
    }
  );
};

// LOG USER OUT //

const logout = (req, res) => {
  // Receive what we need from the request.
  const refreshToken = req.body.token;

  // Checks needed fields are present from request.
  if (refreshToken == null)
    return res.status(400).json({ error: "fields cannot be blank" });

  // Verify their refresh token is real. If so, delete the corresponding
  // one from the database.
  jwt.verify(
    refreshToken,
    process.env.REFRESH_TOKEN_SECRET,
    async (e, user) => {
      // If error, this means the token has been tampered with or is expired. They need to log in to logout?
      if (e) return res.status(403).send("Token tampered with or expired");
      // Delete the corresponding refrehs token from the database.
      try {
        await RefreshToken.findOneAndDelete({
          user_ID: ObjectID(user.userMongoObjectID),
          refresh_token: refreshToken,
        });
      } catch (error) {
        // An unknown error occured while trying to delete the corresponding token.
        return res
          .status(500)
          .send("Could not delete refresh token provided from DB");
      }
      // Logout was successful.
      res.status(200).send("Succesfully logged out");
    }
  );
};

// LOG USER OUT OF ALL DEVICES //

// takes a few minutes as each device needs to call for new access token and discover the refresh token isn't in the db anymore, then it'll log out
const logoutAll = (req, res) => {
  // Receive what we need from the request.
  const refreshToken = req.body.token;

  // Checks needed fields are present from request.
  if (refreshToken == null)
    return res.status(400).json({ error: "fields cannot be blank" });

  // Verify their refresh token is real. If so, delete all refresh tokens
  // from this user in the database.
  jwt.verify(
    refreshToken,
    process.env.REFRESH_TOKEN_SECRET,
    async (e, user) => {
      // If error, this means the token has been tampered with or is expired. They need to log in to logout?
      if (e) return res.status(403).json({ error: "Token tampered with" });
      try {
        // Removes all refresh tokens in the database corresponding to the user (on next call from their other devices it'll log them out).
        await RefreshToken.remove({
          user_ID: ObjectID(user.userMongoObjectID),
        });
      } catch (error) {
        // An unknown error occured while trying to delete the corresponding token.
        return res
          .status(500)
          .send("Could not delete refresh token provided from DB");
      }
      // Logout all was successful.
      res.status(200).send("Succesfully logged out");
    }
  );
};

module.exports = { register, login, token, logout, logoutAll };
