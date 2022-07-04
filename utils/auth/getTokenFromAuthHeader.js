function getTokenFromAuthHeader(req) {
  const authHeader = req.headers["authorization"];
  const token = authHeader && authHeader.split(" ")[1];
  if (token == null) throw "No access token provided.";
  return token;
}

module.exports = getTokenFromAuthHeader;
