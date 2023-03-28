use std::collections::{VecDeque, HashMap};
use rand::Rng;

use actix_web::{
	get,
	post,
	delete,
	put,
  web,
};
use futures::TryStreamExt;
use log::{error, debug};
use mongodb::{Database, bson::{doc, Document, Bson, DateTime, self}, options::{TransactionOptions, FindOptions, FindOneOptions}, Client as MongoClient};
use serde::{Deserialize, Serialize};

use crate::{masked_oid::{MaskingKey, MaskedObjectId, self}, api_types::{ApiResult, Failure, success}, to_unexpected, auth::AuthenticatedUser, services::posts::{Created, Votes}, conf, types::{Comment, Vote, Post}, utils::content_scoring::get_trending_score_time};

#[derive(Serialize, Clone)]
pub struct CommentDetail {
	pub id: MaskedObjectId,
	pub parent_comments: Vec<MaskedObjectId>,
	pub parent_post: MaskedObjectId,
	pub text: String,
	pub replies: i32,
	pub children: Vec<CommentDetail>,
	pub votes: Votes,
}

#[derive(Deserialize)]
pub struct CreateRequest {
	pub text: String,
	pub parent_post: MaskedObjectId,
	pub parent_comments: Vec<MaskedObjectId>,
}

#[put("/comments/{comment_id}/vote")]
pub async fn vote_on_comment(
	mongo_client: web::Data<MongoClient>,
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	comment_id: web::Path<MaskedObjectId>,
	request: web::Json<i32>,  // TODO: enum; see https://github.com/serde-rs/serde/issues/745
) -> ApiResult<Votes, ()> {
	if !(-1..=1).contains(&*request) {
		return Err(Failure::BadRequest("invalid vote"));
	}

	let comment_id = masking_key.unmask(&comment_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

	let mut session = mongo_client.start_session(None)
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
			session.abort_transaction()
				.await
				.map_err(to_unexpected!("Aborting vote transaction failed"))?;
		}

		let existing_vote = db.collection::<Vote>("comment_votes").find_one_with_session(
			doc! {
				"content": {"$eq": comment_id},
				"user": {"$eq": user.id},
			},
			None,
			&mut session
		)
			.await
			.map_err(to_unexpected!("Finding existing vote failed"))?
			.map(|v| v.value);

		session.start_transaction(None)
			.await
			.map_err(to_unexpected!("Starting transaction failed"))?;

		match existing_vote {
			None => {
				match
					db.collection::<Vote>("comment_votes").insert_one_with_session(
						Vote {
							content: comment_id,
							user: user.id,
							value: *request,
						},
						None,
						&mut session
					).await
				{
					Ok(_) => {}
					Err(err) => {
						debug!("Inserting vote failed: {}", err);
						continue 'atomic_vote;
					}
				}
			}
			Some(existing_vote) => {
				match
					db.collection::<Vote>("comment_votes").update_one_with_session(
						doc! {
							"content": {"$eq": comment_id},
							"user": {"$eq": user.id},
							"value": {"$eq": existing_vote},
						},
						doc! {
							"$set": {
								"value": *request,
							},
						},
						None,
						&mut session
					).await
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
		let votes_down_difference = -i32::from(existing_vote == Some(-1)) + i32::from(*request == -1);
		let difference = -existing_vote.unwrap_or(0) + *request;

		let trending_score_time = get_trending_score_time(&comment_id.timestamp());

		// TODO: Are update pipelines atomic? I haven’t found a straight answer yet.
		let comment_update = db.collection::<Comment>("comments").update_one_with_session(
			doc! {
				"_id": {"$eq": comment_id},
			},
			vec! [
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
			&mut session
		)
			.await
			.map_err(to_unexpected!("Updating comment score failed"))?;

		if comment_update.matched_count != 1 {
			error!("Updating comment score failed: no such comment");
			return Err(Failure::Unexpected);
		}

		if let Err(err) = session.commit_transaction().await {
			debug!("Committing voting transaction failed: {}", err);
			continue 'atomic_vote;
		}

		break;
	}

	let votes = db.collection::<Votes>("comments").find_one(
		doc! {
			"_id": {"$eq": comment_id},
		},
		FindOneOptions::builder()
			.projection(doc! {
				"_id": false,
				"up": "$votes_up",
				"down": "$votes_down",
			})
			.build()
	)
		.await
		.map_err(to_unexpected!("Retrieving updated comment votes failed"))?
		.ok_or_else(|| {
			error!("Retrieving updated comment votes failed: no more comments");
			Failure::Unexpected
		})?;

	success(votes)
}

#[post("/comments/")]
pub async fn create_comment(
	mongo_client: web::Data<MongoClient>,
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	request: web::Json<CreateRequest>,
) -> ApiResult<Created, ()> {
	if request.text.len() > conf::COMMENT_MAX_SIZE {
		return Err(Failure::BadRequest("oversized comment text"));
	}

	if request.parent_comments.len() > conf::COMMENT_MAX_DEPTH {
		return Err(Failure::BadRequest("too many parent comments"));
	}

	let mut insert_doc = doc! {
		"owner": &user.id,
		"text": &request.text,
		"parent_post": masking_key.unmask(&request.parent_post).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?,
		"parent_comments": request.parent_comments.iter().map(|masked_oid| masking_key.unmask(masked_oid).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))).collect::<Result<Vec<_>, _>>()?,
		"replies": 0,
		"deleted": false,
		"votes_up": 0,
		"votes_down": 0,
		"absolute_score": 0,
		"trending_score": get_trending_score_time(&DateTime::now()),  // approximate, but will match `_id` exactly with the next vote
	};

	let mut session = mongo_client.start_session(None)
		.await
		.map_err(to_unexpected!("Starting session failed"))?;

	let transaction_options = TransactionOptions::builder()
		.write_concern(mongodb::options::WriteConcern::builder().w(Some(mongodb::options::Acknowledgment::Majority)).build())
		.build();

  	let mut attempt = 0;
	'lp: loop {
		attempt += 1;
		if attempt > 8 {
		error!("Too many comment creation attempts");
		return Err(Failure::Unexpected);
		}

		if attempt > 1 {
			session.abort_transaction()
				.await
				.map_err(to_unexpected!("Aborting vote transaction failed"))?;
		}

		let last_sequential_id =
		db.collection::<Comment>("comments")
		.aggregate(
			[
			doc! {"$sort": {"sequential_id": -1}},
			doc! {"$limit": 1},
			doc! {"$project": {"_id": false, "sequential_id": true}},
			],
			None
		)
		.await
		.map_err(to_unexpected!("Getting next comment sequential id cursor failed"))?
		.try_next()
		.await
		.map_err(to_unexpected!("Getting next comment sequential id failed"))?
		.map(|doc| doc.get_i32("sequential_id").unwrap());

		let new_sequential_id = last_sequential_id.unwrap_or(0) + 1;
		insert_doc.insert("sequential_id", new_sequential_id);

		session.start_transaction(transaction_options.clone())
			.await
			.map_err(to_unexpected!("Starting transaction failed"))?;

		// execute atomic increment of parent comment reply counter (if array of parent comments is not empty)
		if request.parent_comments.len() > 0 {
		let direct_parent_id = masking_key.unmask(&request.parent_comments.last().unwrap()).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
		match db.collection::<Comment>("comments")
			.update_one_with_session(
			doc! {"_id": direct_parent_id},
			doc! {"$inc": {"replies": 1}},
			None,
			&mut session
			).await {
			Ok(update_result) => if update_result.modified_count != 1 {
							error!("updating parent comment failed");
							return Err(Failure::BadRequest("parent comment doesn't exist"));
						}
			Err(_) => {
							error!("updating parent comment failed");
							continue 'lp;
						},
			}
		}

		// execute atomic increment of parent post reply counter
		let parent_post_id = masking_key.unmask(&request.parent_post).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
		match db.collection::<Post>("posts")
			.update_one_with_session(
			doc! {"_id": parent_post_id},
			doc! {"$inc": {"replies": 1}},
			None,
			&mut session
			).await {
			Ok(update_result) => if update_result.modified_count != 1 {
							error!("updating comment parent post failed");
							return Err(Failure::BadRequest("comment parent post doesn't exist"));
						}
			Err(_) => {
							error!("updating comment parent post failed");
							continue 'lp;
						},
			}

		// inserting the comment
		match db.collection::<Document>("comments").insert_one_with_session(&insert_doc, None, &mut session).await {
		Ok(insertion) => if let Err(err) = session.commit_transaction().await {
					debug!("Committing comment transaction failed: {}", err);
					continue 'lp;
				} else {
					return success(
						Created {
							id: masking_key.mask(&insertion.inserted_id.as_object_id().unwrap()),
						}
					);
				}
		Err(err) => {
			error!("Creating comment failed: {}", err);
					continue 'lp;
		}
		};
	};
}

#[delete("/comments/{comment_id}/")]
pub async fn delete_comment(
  	db: web::Data<Database>,
  	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	comment_id: web::Path<MaskedObjectId>,
) -> ApiResult<(), ()> {
	match db.collection::<Comment>("comments").update_one(
			doc! {"_id": masking_key.unmask(&comment_id).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?, "owner": &user.id},
			doc! {"$set": {"deleted": true}},
			None,
		).await {
			Ok(_) => success(()), // idempotent deletion
			Err(_) => Err(Failure::Unexpected),
		}
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum CommentFetchType {
    Root,
		Thread,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum CommentSort {
		New, // sequential_id
		Top, // absolute_score
		Worst, // absolute_score
		Controversial, // absoluted absolute_score (closest absolute_score to -1)
		Best, // trending_score
		Replies, // most number of replies
}

impl CommentSort {
		fn sort(&self) -> Vec<Document> {
				match self {
						CommentSort::New => vec![doc! {"sequential_id": -1}],
						CommentSort::Top => vec![doc! {"absolute_score": -1}],
						CommentSort::Worst => vec![doc! {"absolute_score": 1}],
						CommentSort::Controversial => vec![
							doc! {
								"$addFields": {
										"score_diff": {
												"$abs": {
														"$subtract": ["$absolute_score", -1]
												}
										}
								}
							},
							doc! {
									"$sort": { "score_diff": 1 }
							},
						],
						CommentSort::Best => vec![doc! {"trending_score": -1}],
						CommentSort::Replies => vec![doc! {"replies": -1}],
				}
		}
}

#[derive(Deserialize)]
pub struct ListQuery {
		kind: CommentFetchType,
		parent: MaskedObjectId,
		seen: Vec<MaskedObjectId>,
		sort: CommentSort,
}

#[get("/comments/")]
pub async fn get_comment(
    db: web::Data<Database>,
    masking_key: web::Data<&'static MaskingKey>,
    query: web::Json<ListQuery>,
) -> ApiResult<Box<Vec<CommentDetail>>, ()> {

		let id  = masking_key.unmask(&query.parent).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

		let unmasked_seen: Result<Vec<Bson>, _> = query.seen.iter()
				.map(|masked_oid| {
						masking_key.unmask(masked_oid)
								.map_err(|_| Failure::BadRequest("bad masked id"))
								.map(|oid| Bson::ObjectId(oid))
				})
				.collect();
		let mut excluded_ids = unmasked_seen?;

		let mut find_filter = match &query.kind {
			CommentFetchType::Root { .. } => {
				vec![
					doc! {
						"parent_post": id,
					},
					doc! {
						"parent_comments": {
							"$size": 0
						}
					}
				]
			},
			CommentFetchType::Thread { .. } => {
				vec![
					doc! {
						"$expr": {
								"$eq": [
										{ "$arrayElemAt": [ "$parent_comments", -1 ] },
										id
								]
						}
					},
				]
			},
		};

		find_filter.push(doc! {
			"_id": {
					"$not": {
							"$in": &excluded_ids
					}
			}
		});

    let find_filter = doc! {
			"$and": find_filter
		};

		let mut found_comments;
    if let CommentSort::Controversial = query.sort {
			// populate found_comments based on the same filters, but also sort by the absolute value of absolute_score and find those comments closest to it
			// use the aggregation framework to do this
			let mut pipeline = vec![
				doc! {
					"$match": find_filter
				},
				doc! {
						"$limit": i64::from(conf::COMMENTS_PAGE_SIZE)
				},
			];
			pipeline.splice(1..1, query.sort.sort());
			found_comments = db.collection::<Comment>("comments")
					.aggregate(pipeline, None)
					.await
					.map_err(to_unexpected!("Getting comments failed"))?
					.map_ok(|doc| bson::from_bson(bson::Bson::Document(doc)).map_err(to_unexpected!("Deserializing comment failed")))
					.try_collect::<Vec<Result<Comment, Failure<()>>>>()
					.await
					.map_err(to_unexpected!("Getting comments failed"))?
					.into_iter()
					.map(|result| result.and_then(|comment| Ok(CommentDetail {
							id: masking_key.mask(&comment.id),
							parent_comments: comment.parent_comments.iter().map(|id| masking_key.mask(id)).collect(),
							parent_post: masking_key.mask(&comment.parent_post),
							text: if comment.deleted {"[deleted]".to_string()} else {comment.text},
							replies: comment.replies,
							children: vec![],
							votes: Votes {
									up: u32::try_from(comment.votes_up).unwrap(),
									down: u32::try_from(comment.votes_down).unwrap(),
							},
					})))
					.collect::<Result<Vec<CommentDetail>, Failure<()>>>()?;
		} else {
			found_comments = db.collection::<Comment>("comments")
					.find(find_filter, FindOptions::builder()
							.sort(query.sort.sort()[0].clone())
							.limit(i64::from(conf::COMMENTS_PAGE_SIZE))
							.build()
					)
					.await
					.map_err(to_unexpected!("Getting comments cursor failed"))?
					.map_ok(|comment| Ok(CommentDetail {
							id: masking_key.mask(&comment.id),
							parent_comments: comment.parent_comments.iter().map(|id| masking_key.mask(id)).collect(),
							parent_post: masking_key.mask(&comment.parent_post),
							text: if comment.deleted {"[deleted]".to_string()} else {comment.text},
							replies: comment.replies,
							children: vec![],
							votes: Votes {
								up: u32::try_from(comment.votes_up).unwrap(),
								down: u32::try_from(comment.votes_down).unwrap(),
							},
					}))
					.try_collect::<Vec<Result<CommentDetail, Failure<()>>>>()
					.await
					.map_err(to_unexpected!("Getting comments failed"))?
					.into_iter()
					.collect::<Result<Vec<CommentDetail>, Failure<()>>>()?;
		}

		let init_depth: i32;
		if found_comments.len() == 0 { return success(Box::new(vec![])) } else {
			init_depth = found_comments[0].parent_comments.len() as i32;
		}

		let mut deque: VecDeque<CommentDetail> = VecDeque::from(found_comments.clone());
		let mut count = 0;
		while let Some(parent_comment) = deque.pop_front() {
			if count > conf::MAX_REPLYING_COMMENTS_PER_LOAD { break };
			if parent_comment.replies == 0 { continue };
			let replies = db.collection::<Comment>("comments")
					.find(
							doc! {
								"$and": vec![
									doc! {
										"_id": {
												"$not": {
														"$in": &excluded_ids
												}
										}
									},
									doc! {
										"$expr": {
												"$eq": [
														{ "$arrayElemAt": [ "$parent_comments", -1 ] },
														masking_key.unmask(&parent_comment.id).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?
												]
										}
									},
								]
							},
							FindOptions::builder()
							    .sort(doc! {"replies": -1}) // sort threaded comments by replies to help build the best tree structures
									.limit(i64::from(conf::MAX_REPLYING_COMMENTS_PER_LOAD))
									.build()
					)
					.await
					.map_err(to_unexpected!("Getting comments cursor failed"))?
					.map_ok(|comment| Ok(CommentDetail {
							id: masking_key.mask(&comment.id),
							parent_comments: comment.parent_comments.iter().map(|id| masking_key.mask(id)).collect(),
							parent_post: masking_key.mask(&comment.parent_post),
							text: if comment.deleted {"[deleted]".to_string()} else {comment.text},
							replies: comment.replies,
							children: vec![],
							votes: Votes {
								up: u32::try_from(comment.votes_up).unwrap(),
								down: u32::try_from(comment.votes_down).unwrap(),
							},
					}))
					.try_collect::<Vec<Result<CommentDetail, Failure<()>>>>()
					.await
					.map_err(to_unexpected!("Getting comments failed"))?
					.into_iter()
					.collect::<Result<Vec<CommentDetail>, Failure<()>>>()?;
				for comment in replies {
						if count < conf::MIN_REPLYING_COMMENTS_PER_LOAD_IF_AVAILABLE || rand::thread_rng().gen_bool(p(1.0, parent_comment.replies as f64)) {
								count += 1;
								excluded_ids.push(Bson::ObjectId(masking_key.unmask(&comment.id).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?));
								deque.push_back(comment.clone());
								found_comments.push(comment.clone());
						}
				}
		}

		let threaded_comments = thread_comments(found_comments.clone(), init_depth);

		let results: Vec<CommentDetail> = found_comments
		    .iter()
				.filter(|c| c.parent_comments.len() == init_depth as usize && threaded_comments.iter().any(|tc| tc.id == c.id))
				.filter_map(|c| threaded_comments.iter().find(|tc| tc.id == c.id).cloned())
				.collect();

    success(Box::new(results))
}

fn thread_comments(comments: Vec<CommentDetail>, init_depth: i32) -> Vec<CommentDetail> {
		let mut comment_map: HashMap<String, CommentDetail> = HashMap::new();

		// first pass: create comment map and add each comment to the map
		for comment in comments {
				comment_map.insert(comment.id.to_string(), comment);
		}

		// second pass: thread each top-level comment and its children recursively
		let mut threaded_comments: Vec<CommentDetail> = vec![];
		for comment in comment_map.clone().values() {
				if comment.parent_comments.len() == (init_depth) as usize {
						let threaded_comment = thread_comment((init_depth as u32), comment, &mut comment_map);
						threaded_comments.push(threaded_comment);
				}
		}
		threaded_comments
}

fn thread_comment(depth: u32, comment: &CommentDetail, comment_map: &mut HashMap<String, CommentDetail>) -> CommentDetail {
		let mut threaded_comment = comment.clone();
		for c in comment_map.clone().values() {
				if c.parent_comments.contains(&comment.id) && c.parent_comments.len() <= (depth + 1).try_into().unwrap() {
						let threaded_child = thread_comment(depth + 1, c, comment_map);
						threaded_comment.children.push(threaded_child);
				}
		}
		comment_map.remove(&threaded_comment.id.to_string());
		threaded_comment
}


fn p(numerator: f64, denominator: f64) -> f64 {
		if denominator == 0.0 {
				0.0
		} else {
				if denominator > conf::MAX_REPLYING_COMMENTS_PER_LOAD as f64 {
						return numerator / conf::MAX_REPLYING_COMMENTS_PER_LOAD as f64
				}
				numerator / denominator
		}
}

