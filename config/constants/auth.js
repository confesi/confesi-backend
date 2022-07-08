// Number of times salting is applied to password hashing.
const SALT_ROUNDS = 10;
// How long the access token lasts for.
const ACCESS_TOKEN_LIFETIME = "30m";
// How long the refresh token lasts for.
const REFRESH_TOKEN_LIFETIME = "1y";
const PASSWORD_MIN_LENGTH = 8;
const PASSWORD_MAX_LENGTH = 100;
const EMAIL_MIN_LENGTH = 3;
const EMAIL_MAX_LENGTH = 255;
const USERNAME_MIN_LENGTH = 3;
const USERNAME_MAX_LENGTH = 30;

module.exports = {
  SALT_ROUNDS,
  ACCESS_TOKEN_LIFETIME,
  REFRESH_TOKEN_LIFETIME,
  PASSWORD_MIN_LENGTH,
  PASSWORD_MAX_LENGTH,
  EMAIL_MIN_LENGTH,
  EMAIL_MAX_LENGTH,
  USERNAME_MIN_LENGTH,
  USERNAME_MAX_LENGTH,
};
