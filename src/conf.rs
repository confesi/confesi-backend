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
