// Date that we start recording time from
const startDate = new Date("2022-07-08T09:28:16.766Z");

// Calculates a rank for posts based on their total votes and
// time since they've been uploaded. Based on Reddit's trending algorithm.
// LINK TO ARTICLE: https://moz.com/blog/reddit-stumbleupon-delicious-and-hacker-news-algorithms-exposed
function rank(deltaVotes) {
  const currentDate = new Date();
  const deltaSecondTime = Math.floor((currentDate - startDate) / 1000);
  var y;
  if (deltaVotes > 0) {
    y = 1;
  } else if (deltaVotes === 0) {
    y = 0;
  } else if (deltaVotes < 0) {
    y = -1;
  }
  var z;
  if (Math.abs(deltaVotes) >= 1) {
    z = Math.abs(deltaVotes);
  } else {
    z = 1;
  }
  // The "45000" is the number of seconds in 12.5 hours. This
  // helps the posts with high scores decay over time when people
  // continue voting.
  return Math.log10(z) + (y * deltaSecondTime) / 45000;
}

module.exports = rank;
