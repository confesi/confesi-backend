const mongoose = require("mongoose");

// The schema for a JWT refresh token. These are created upon logging in
// or signing up. They are stored in the database. Then, when a user's
// access token expires (or every x minutes automatically) the frontend
// makes a call to the "/token" route and validates a matching refresh token for
// the user requesting a new valid access token exists. If so, the user is given a new
// access token.

const refreshTokenSchema = new mongoose.Schema({
  // Example: Reference ID
  user_ID: {
    type: mongoose.Schema.Types.ObjectId,
    required: true,
    index: true,
  },
  // Example: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VyTW9uZ29PYmplY3RJRCI6IjYyYWZmOWIwOGFkMDc3ZjIxN2M3MTMwNSIsImlhdCI6MTY1NTY5OTg4OCwiZXhwIjoxNjg3MjU3NDg4fQ._qJZPttoMbKvqx3OPJMZznKsSPDnPLNhTyBdHHOAw5g
  refresh_token: {
    type: String,
    required: true,
    unique: true, // Maybe not unique? Across different devices?
  },
  // Example: 2022-06-20T04:17:43.720+00:00
  created_date: {
    type: Date,
    default: Date.now,
  },
});

module.exports = mongoose.model("RefreshToken", refreshTokenSchema);
