use std::time::Duration;

/// How long sessions remain usable after their last use.
pub const UNUSED_SESSION_TTL: Duration =
	Duration::from_secs(3600 * 24 * 60);

/// How long to wait between refreshing session TTLs.
pub const SESSION_MIN_TIME_BETWEEN_REFRESH: Duration =
	Duration::from_secs(3600 * 24);

/// The number of posts to return for each request to a post list.
pub const POSTS_PAGE_SIZE: u16 = 5;

/// The maximum length of a post in UTF-8 bytes.
pub const POST_MAX_SIZE: usize = 1000;

/// The maximum length of a comment in UTF-8 bytes.
pub const COMMENT_MAX_SIZE: usize = 500;

/// The maximum length of a username.
pub const USERNAME_MAX_LENGTH: usize = 32;

/// The expected value of the `Host` header. Checked in order to protect unauthenticated endpoints from DNS rebinding.
pub const HOST: &str = "localhost:3000";

/// The permitted values of the `Origin` header. Will also become `Access-Control-Allow-Origin`.
pub const PERMITTED_ORIGINS: &[&str] = &[
	"https://app.invalid",
	"http://api-docs.localhost:8080",
];

/// The reference point for the time component of vote calculations, in seconds since the Unix epoch.
pub const TRENDING_EPOCH: i64 = 1640995200;  // 2022-01-01T00:00:00Z

pub const TRENDING_DECAY: f64 = 103616.32918473207;  // 45000 ln 10

/// The max number of watched universities a user can have at once.
pub const MAX_WATCHED_UNIVERSITIES: i32 = 3;

/// The max number of results to return when searching for a school by query.
pub const MAX_SCHOOL_RESULTS_PER_QUERY: i32 = 10;
