const jwt = require("jsonwebtoken");

// Validates that the access token passed in the authorization header from the frontend is valid
function authenticateToken(req, res, next) {
  const authHeader = req.headers["authorization"];
  // Takes the token value from Authorization header, switching
  // from format: "Bearer <token>" to "<token>".
  const token = authHeader && authHeader.split(" ")[1];
  // Ensures token is not null.
  if (token == null) return res.status(401).send("No access token provided");
  // Verifies the token has not been tampered with and is valid.
  jwt.verify(token, process.env.ACCESS_TOKEN_SECRET, (e, user) => {
    if (e) return res.status(403).send("Not authorized");
    req.user = user;
    next();
  });
}

module.exports = authenticateToken;
