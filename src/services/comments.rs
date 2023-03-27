// todo: do comments even need a sequential id?
// todo: add metric fields for comments (ex: votes)

use std::collections::{VecDeque, HashMap, HashSet};
use rand::Rng;

use actix_web::{
  get,
  post,
	delete,
  web,
};
use futures::TryStreamExt;
use log::{error, debug};
use mongodb::{Database, bson::{doc, Document, Bson, oid::ObjectId}, options::{TransactionOptions, FindOptions}, Client as MongoClient};
use serde::{Deserialize, Serialize};

use crate::{masked_oid::{MaskingKey, MaskedObjectId, self}, api_types::{ApiResult, Failure, success}, to_unexpected, auth::AuthenticatedUser, services::posts::Created, conf, types::Comment};

#[derive(Serialize, Clone, Debug)]
pub struct CommentDetail {
	pub id: MaskedObjectId,
	pub parent_comments: Vec<MaskedObjectId>,
	pub parent_post: MaskedObjectId,
	pub text: String,
	pub replies: i32,
	pub children: Vec<CommentDetail>,
}

#[derive(Deserialize)]
pub struct CreateRequest {
  pub text: String,
  pub parent_post: MaskedObjectId,
  pub parent_comments: Vec<MaskedObjectId>,
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
  };

	let mut session = mongo_client.start_session(None)
		.await
		.map_err(to_unexpected!("Starting session failed"))?;

	let transaction_options = TransactionOptions::builder()
		.write_concern(mongodb::options::WriteConcern::builder().w(Some(mongodb::options::Acknowledgment::Majority)).build())
		.build();

  let mut attempt = 0;
  'lp: loop {
		debug!("LOOPED");
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

    // execute atomic increment of parent comment (if array of parent comments is not empty)
    if request.parent_comments.len() > 0 {
      let direct_parent_id = masking_key.unmask(&request.parent_comments.last().unwrap()).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
			debug!("ID: {direct_parent_id}");
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
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ListQuery {
	Root {
		parent_post: MaskedObjectId,
		seen: Vec<MaskedObjectId>
	},
	Thread {
		parent_comment: MaskedObjectId,
		seen: Vec<MaskedObjectId>
	},
}

#[get("/comments/")]
pub async fn get_comment(
    db: web::Data<Database>,
    masking_key: web::Data<&'static MaskingKey>,
    query: web::Json<ListQuery>,
) -> ApiResult<Box<Vec<CommentDetail>>, ()> {
    let id = match &*query {
        ListQuery::Root {
            parent_post,
            ..
        } => masking_key.unmask(parent_post).map_err(|_| Failure::BadRequest("bad masked id"))?,
        ListQuery::Thread {
            parent_comment,
            ..
        } => masking_key.unmask(parent_comment).map_err(|_| Failure::BadRequest("bad masked id"))?,
    };

    let mut excluded_ids: Vec<Bson> = match &*query {
			ListQuery::Root { seen, .. } | ListQuery::Thread { seen, .. } => {
					let unmasked_seen: Result<Vec<Bson>, _> = seen.iter()
							.map(|masked_oid| {
									masking_key.unmask(masked_oid)
											.map_err(|_| Failure::BadRequest("bad masked id"))
											.map(|oid| Bson::ObjectId(oid))
							})
							.collect();
					unmasked_seen?
			}
		};

		let mut find_filter = match &*query {
			ListQuery::Root { .. } => {
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
			ListQuery::Thread { .. } => {
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

    let mut found_comments = db.collection::<Comment>("comments")
        .find(find_filter, FindOptions::builder()
            .sort(doc! {}) // todo: add sort to comments (recents, top voted, etc.)
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
        }))
        .try_collect::<Vec<Result<CommentDetail, Failure<()>>>>()
        .await
        .map_err(to_unexpected!("Getting comments failed"))?
        .into_iter()
        .collect::<Result<Vec<CommentDetail>, Failure<()>>>()?;

		println!("FOUND COMMENTS: {}", found_comments.len());

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
							.sort(doc! {"replies": -1})
							.limit(i64::from(conf::MAX_REPLYING_COMMENTS_PER_LOAD)) // todo: update this?
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
					}))
					.try_collect::<Vec<Result<CommentDetail, Failure<()>>>>()
					.await
					.map_err(to_unexpected!("Getting comments failed"))?
					.into_iter()
					.collect::<Result<Vec<CommentDetail>, Failure<()>>>()?;
				for comment in replies {
					println!("REPLY FOUND!");
					if count < conf::MIN_REPLYING_COMMENTS_PER_LOAD_IF_AVAILABLE || rand::thread_rng().gen_bool(p(1.0, parent_comment.replies as f64)) {
						count += 1;
						excluded_ids.push(Bson::ObjectId(masking_key.unmask(&comment.id).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?));
						println!("PUSHED BACK {}", &comment.text);
						deque.push_back(comment.clone());
						found_comments.push(comment.clone());
					}
				}
		}

		println!("OVERALL: {}", found_comments.len());
    success(Box::new(thread_comments(found_comments)))
}

fn thread_comments(comments: Vec<CommentDetail>) -> Vec<CommentDetail> {
	let mut comment_map: HashMap<String, CommentDetail> = HashMap::new();

	// first pass: Create comment map and add each comment to the map
	for comment in comments {
			comment_map.insert(comment.id.to_string(), comment);
	}

	// second pass: Thread each top-level comment and its children recursively
	let mut threaded_comments: Vec<CommentDetail> = vec![];
	for comment in comment_map.clone().values() {
			if comment.parent_comments.is_empty() {
					let threaded_comment = thread_comment(0, comment, &mut comment_map);
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
		if conf::MAX_REPLYING_COMMENTS_PER_LOAD as f64 > denominator {
			return numerator / conf::MAX_REPLYING_COMMENTS_PER_LOAD as f64
		}
		numerator / denominator
	}
}
