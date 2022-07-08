// Max character length of post body text.
const POST_MAX_LENGTH = 1000;
// Max character length of comment.
const COMMENT_MAX_LENGTH = 500;
// How many posts should be called each time a post.
// retrieving function is called (getting recents, trending posts, etc.).
const NUMBER_OF_POSTS_TO_RETURN_PER_CALL = 5;
// Max/min year of post (year is the year of university student is in).
const MAX_YEAR = 8;
const MIN_YEAR = 1;

module.exports = {
  NUMBER_OF_POSTS_TO_RETURN_PER_CALL,
  COMMENT_MAX_LENGTH,
  POST_MAX_LENGTH,
  MAX_YEAR,
  MIN_YEAR,
};
