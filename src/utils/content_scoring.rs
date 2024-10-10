use mongodb::bson::DateTime;

use crate::conf;

/// Gets the time-based offset of the trending score for the given timestamp.
pub fn get_trending_score_time(date_time: &DateTime) -> f64 {
	f64::from(u32::try_from(date_time.timestamp_millis() / 1000 - conf::TRENDING_EPOCH).unwrap())
		/ conf::TRENDING_DECAY
}
