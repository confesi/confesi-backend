use actix_web::web;
use actix_web::{get, post, put};
use chrono::{Duration, Timelike, Utc};
use futures::TryStreamExt;
use log::{debug, error, info};
use mongodb::bson::{doc, Bson, DateTime, Document};
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::options::{FindOneOptions, FindOptions};
use mongodb::{Client as MongoClient, Database};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

use crate::api_types::{success, ApiResult, Failure};
use crate::auth::AuthenticatedUser;
use crate::conf;
use crate::masked_oid::{self, MaskedObjectId, MaskedSequentialId, MaskingKey};
use crate::to_unexpected;
use crate::types::{Post, Vote};

#[derive(Serialize)]
pub struct ReplyContext {
	pub id: MaskedObjectId,
}

#[derive(Deserialize, Serialize)]
pub struct Votes {
	pub up: u32,
	pub down: u32,
}

#[derive(Serialize)]
pub struct Detail {
	pub id: MaskedObjectId,
	pub sequential_id: MaskedSequentialId,
	pub reply_context: Option<ReplyContext>,
	pub text: String,
	pub created_at: String,
	pub votes: Votes,
}

#[derive(Deserialize)]
#[serde(tag = "sort", rename_all = "kebab-case")]
pub enum ListQuery {
	Recent { before: Option<MaskedSequentialId> },
	Trending,
}

#[derive(Deserialize)]
pub struct CreateRequest {
	pub text: String,
}

#[derive(Serialize)]
pub struct Created {
	pub id: MaskedObjectId,
}

#[derive(Deserialize)]
#[serde(tag = "sort", rename_all = "kebab-case")]
pub enum HottestQuery {
	PastDate { date: String },
	Yesterday,
}

// Returns the hottest posts created yesterday, or from a specific date.
#[get("/posts/hottest/")]
pub async fn daily_hottest(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	query: web::Query<HottestQuery>,
) -> ApiResult<Box<[Detail]>, ()> {
	// What qualifies a post as "hottest" can change in the future. Currently sorting by a post's "absolute score".
	let sort: Document = doc! {"absolute_score": -1};
	let find_query: Document;
	let today_at_midnight: chrono::DateTime<Utc> = Utc::now()
		.with_hour(0)
		.unwrap()
		.with_minute(0)
		.unwrap()
		.with_second(0)
		.unwrap();
	match &*query {
		// You're asking for a date in the past.
		HottestQuery::PastDate { date } => {
			if let Ok(ms_since_date) = (*date).parse::<i64>() {
				if ms_since_date >= today_at_midnight.timestamp_millis() {
					return Err(Failure::BadRequest(
						"datetime must be from yesterday or older",
					));
				}
				let lower_bound = chrono::DateTime::<Utc>::from_utc(
					match chrono::NaiveDateTime::from_timestamp_opt(ms_since_date / 1000, 0) {
						Some(date) => date
							.with_hour(0)
							.unwrap()
							.with_minute(0)
							.unwrap()
							.with_second(0)
							.unwrap(),
						None => return Err(Failure::BadRequest("invalid datetime passed")),
					},
					Utc,
				);
				let upper_bound = lower_bound + Duration::days(1);
				// Filter by date range, where [`lower_bound`] is midnight of the day you
				// requested, and [`upper_bound`] is 24 hours ahead of [`lower_bound`].
				find_query = doc! {
					"$and": [
						{"created_at": {"$gte": Bson::DateTime(DateTime::from_millis(lower_bound.timestamp_millis()))}},
						{"created_at": {"$lt": Bson::DateTime(DateTime::from_millis(upper_bound.timestamp_millis()))}},
					]
				};
			} else {
				return Err(Failure::BadRequest("invalid datetime passed"));
			}
		}
		// You're asking for yesterday's date specifically.
		HottestQuery::Yesterday => {
			let yesterday_at_midnight: chrono::DateTime<Utc> =
				today_at_midnight - chrono::Duration::days(1);
			// Filter by date range between yesterday at midnight and today at midnight.
			find_query = doc! {
				"$and": [
					{"created_at": {"$gte": Bson::DateTime(DateTime::from_millis(yesterday_at_midnight.timestamp_millis()))}},
					{"created_at": {"$lt": Bson::DateTime(DateTime::from_millis(today_at_midnight.timestamp_millis()))}},
				]
			};
		}
	}

	let posts = db
		.collection::<Post>("posts")
		.find(
			find_query,
			FindOptions::builder()
				.sort(sort)
				.limit(i64::from(conf::HOTTEST_POSTS_PER_PAGE))
				.build(),
		)
		.await
		.map_err(to_unexpected!("Getting posts cursor failed"))?
		.map_ok(|post| {
			Ok(Detail {
				id: masking_key.mask(&post.id),
				sequential_id: masking_key
					.mask_sequential(u64::try_from(post.sequential_id).unwrap()),
				reply_context: None,
				text: post.text,
				created_at: post
					.created_at
					.try_to_rfc3339_string()
					.map_err(to_unexpected!("Formatting post timestamp failed"))?,
				votes: Votes {
					up: u32::try_from(post.votes_up).unwrap(),
					down: u32::try_from(post.votes_down).unwrap(),
				},
			})
		})
		.try_collect::<Vec<Result<Detail, Failure<()>>>>()
		.await
		.map_err(to_unexpected!("Getting posts failed"))?
		.into_iter()
		.collect::<Result<Vec<Detail>, Failure<()>>>()?;

	success(posts.into())
}

#[get("/posts/")]
pub async fn list(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	query: web::Query<ListQuery>,
) -> ApiResult<Box<[Detail]>, ()> {
	let find_query;
	let sort;
	match &*query {
		ListQuery::Recent { before } => {
			find_query = match before {
				None => doc! {},
				Some(before) => {
					let before = masking_key.unmask_sequential(before).map_err(
						|masked_oid::PaddingError| Failure::BadRequest("bad masked sequential id"),
					)?;
					doc! {
						"sequential_id": {"$lt": i64::try_from(before).unwrap()},
					}
				}
			};

			sort = doc! {"sequential_id": -1};
		}
		ListQuery::Trending => {
			find_query = doc! {};
			sort = doc! {"trending_score": -1};
		}
	}
	let posts = db
		.collection::<Post>("posts")
		.find(
			find_query,
			FindOptions::builder()
				.sort(sort)
				.limit(i64::from(conf::POSTS_PAGE_SIZE))
				.build(),
		)
		.await
		.map_err(to_unexpected!("Getting posts cursor failed"))?
		.map_ok(|post| {
			Ok(Detail {
				id: masking_key.mask(&post.id),
				sequential_id: masking_key
					.mask_sequential(u64::try_from(post.sequential_id).unwrap()),
				reply_context: None,
				text: post.text,
				created_at: post
					.created_at
					.try_to_rfc3339_string()
					.map_err(to_unexpected!("Formatting post timestamp failed"))?,
				votes: Votes {
					up: u32::try_from(post.votes_up).unwrap(),
					down: u32::try_from(post.votes_down).unwrap(),
				},
			})
		})
		.try_collect::<Vec<Result<Detail, Failure<()>>>>()
		.await
		.map_err(to_unexpected!("Getting posts failed"))?
		.into_iter()
		.collect::<Result<Vec<Detail>, Failure<()>>>()?;

	success(posts.into())
}

/// Gets the time-based offset of the trending score for the given timestamp.
fn get_trending_score_time(date_time: &DateTime) -> f64 {
	f64::from(u32::try_from(date_time.timestamp_millis() / 1000 - conf::TRENDING_EPOCH).unwrap())
		/ conf::TRENDING_DECAY
}

#[post("/posts/")]
pub async fn create(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	request: web::Json<CreateRequest>,
) -> ApiResult<Created, ()> {
	if request.text.len() > conf::POST_MAX_SIZE {
		return Err(Failure::BadRequest("oversized post text"));
	}
	let mut insert_doc = doc! {
		"owner": &user.id,
		"text": &request.text,
		"votes_up": 0,
		"votes_down": 0,
		"absolute_score": 0,
		"trending_score": get_trending_score_time(&DateTime::now()),  // approximate, but will match `_id` exactly with the next vote
		"created_at": DateTime::now(),
	};
	let mut attempt = 0;
	let insertion = loop {
		attempt += 1;
		if attempt > 100 {
			error!("Too many post creation attempts");
			return Err(Failure::Unexpected);
		}

		let last_sequential_id = db
			.collection::<Post>("posts")
			.aggregate(
				[
					doc! {"$sort": {"sequential_id": -1}},
					doc! {"$limit": 1},
					doc! {"$project": {"_id": false, "sequential_id": true}},
				],
				None,
			)
			.await
			.map_err(to_unexpected!(
				"Getting next post sequential id cursor failed"
			))?
			.try_next()
			.await
			.map_err(to_unexpected!("Getting next post sequential id failed"))?
			.map(|doc| doc.get_i32("sequential_id").unwrap());

		let new_sequential_id = last_sequential_id.unwrap_or(0) + 1;
		insert_doc.insert("sequential_id", new_sequential_id);

		match db
			.collection::<Document>("posts")
			.insert_one(&insert_doc, None)
			.await
		{
			Ok(insertion) => break insertion,
			Err(err) => match err.kind.as_ref() {
				ErrorKind::Write(WriteFailure::WriteError(write_err))
					if write_err.code == 11000 =>
				{
					info!("Retrying post creation: {}", err);
				}
				_ => {
					error!("Creating post failed: {}", err);
					return Err(Failure::Unexpected);
				}
			},
		}
	};

	success(Created {
		id: masking_key.mask(&insertion.inserted_id.as_object_id().unwrap()),
	})
}

#[put("/posts/{post_id}/vote")]
pub async fn vote(
	mongo_client: web::Data<MongoClient>,
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	post_id: web::Path<MaskedObjectId>,
	request: web::Json<i32>, // TODO: enum; see https://github.com/serde-rs/serde/issues/745
) -> ApiResult<Votes, ()> {
	if !(-1..=1).contains(&*request) {
		return Err(Failure::BadRequest("invalid vote"));
	}

	let post_id = masking_key
		.unmask(&post_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

	let mut session = mongo_client
		.start_session(None)
		.await
		.map_err(to_unexpected!("Starting session failed"))?;

	let mut attempt = 0;
	'atomic_vote: loop {
		attempt += 1;
		if attempt > 8 {
			error!("Too many voting attempts");
			return Err(Failure::Unexpected);
		}

		if attempt > 1 {
			session
				.abort_transaction()
				.await
				.map_err(to_unexpected!("Aborting vote transaction failed"))?;
		}

		let existing_vote = db
			.collection::<Vote>("votes")
			.find_one_with_session(
				doc! {
					"post": {"$eq": post_id},
					"user": {"$eq": user.id},
				},
				None,
				&mut session,
			)
			.await
			.map_err(to_unexpected!("Finding existing vote failed"))?
			.map(|v| v.value);

		session
			.start_transaction(None)
			.await
			.map_err(to_unexpected!("Starting transaction failed"))?;

		match existing_vote {
			None => {
				match db
					.collection::<Vote>("votes")
					.insert_one_with_session(
						Vote {
							post: post_id,
							user: user.id,
							value: *request,
						},
						None,
						&mut session,
					)
					.await
				{
					Ok(_) => {}
					Err(err) => {
						debug!("Inserting vote failed: {}", err);
						continue 'atomic_vote;
					}
				}
			}
			Some(existing_vote) => {
				match db
					.collection::<Vote>("votes")
					.update_one_with_session(
						doc! {
							"post": {"$eq": post_id},
							"user": {"$eq": user.id},
							"value": {"$eq": existing_vote},
						},
						doc! {
							"$set": {
								"value": *request,
							},
						},
						None,
						&mut session,
					)
					.await
				{
					Ok(update_result) if update_result.matched_count == 1 => {}
					Ok(_) => {
						debug!("Updating vote failed: no match");
						continue 'atomic_vote;
					}
					Err(err) => {
						debug!("Updating vote failed: {}", err);
						continue 'atomic_vote;
					}
				}
			}
		}

		let votes_up_difference = -i32::from(existing_vote == Some(1)) + i32::from(*request == 1);
		let votes_down_difference =
			-i32::from(existing_vote == Some(-1)) + i32::from(*request == -1);
		let difference = -existing_vote.unwrap_or(0) + *request;

		let trending_score_time = get_trending_score_time(&post_id.timestamp());

		// TODO: Are update pipelines atomic? I haven’t found a straight answer yet.
		let post_update = db
			.collection::<Post>("posts")
			.update_one_with_session(
				doc! {
					"_id": {"$eq": post_id},
				},
				vec![
					doc! {
						"$addFields": {
							"votes_up": {
								"$add": ["$votes_up", {"$literal": votes_up_difference}],
							},
							"votes_down": {
								"$add": ["$votes_down", {"$literal": votes_down_difference}],
							},
							"absolute_score": {
								"$add": ["$absolute_score", {"$literal": difference}],
							},
						},
					},
					doc! {
						"$addFields": {
							"trending_score": {"$add": [
								{"$multiply": [
									{"$cond": [
										{"$lt": ["$absolute_score", 0]},
										-1,
										1,
									]},
									{"$ln":
										{"$add": [1, {"$abs": "$absolute_score"}]}},
								]},
								{"$literal": trending_score_time},
							]},
						},
					},
				],
				None,
				&mut session,
			)
			.await
			.map_err(to_unexpected!("Updating post score failed"))?;

		if post_update.matched_count != 1 {
			error!("Updating post score failed: no such post");
			return Err(Failure::Unexpected);
		}

		if let Err(err) = session.commit_transaction().await {
			debug!("Committing voting transaction failed: {}", err);
			continue 'atomic_vote;
		}

		break;
	}

	let votes = db
		.collection::<Votes>("posts")
		.find_one(
			doc! {
				"_id": {"$eq": post_id},
			},
			FindOneOptions::builder()
				.projection(doc! {
					"_id": false,
					"up": "$votes_up",
					"down": "$votes_down",
				})
				.build(),
		)
		.await
		.map_err(to_unexpected!("Retrieving updated post votes failed"))?
		.ok_or_else(|| {
			error!("Retrieving updated post votes failed: no more post");
			Failure::Unexpected
		})?;

	success(votes)
}
