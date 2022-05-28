const PORT = 3000;
const SALT_ROUNDS = 10;
const ACCESS_TOKEN_LIFETIME = "60s";
const REFRESH_TOKEN_LIFETIME = "1y"; // "30s" for testing short-term refresh token expiry

module.exports = {PORT, SALT_ROUNDS, ACCESS_TOKEN_LIFETIME, REFRESH_TOKEN_LIFETIME};