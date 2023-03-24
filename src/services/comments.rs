// todo: do comments even need a sequential id?

use actix_web::{
  get,
  post,
	delete,
  web,
};
use futures::TryStreamExt;
use log::{error, debug};
use mongodb::{Database, bson::{doc, Document, Bson}, options::{TransactionOptions, FindOptions}, Client as MongoClient};
use serde::{Deserialize, Serialize};

use crate::{masked_oid::{MaskingKey, MaskedObjectId, self}, api_types::{ApiResult, Failure, success}, to_unexpected, auth::AuthenticatedUser, services::posts::Created, conf, types::Comment};

#[derive(Serialize)]
pub struct CommentDetail {
	pub id: MaskedObjectId,
	pub parent_comments: Vec<MaskedObjectId>,
	pub parent_post: MaskedObjectId,
	pub text: String,
	pub replies: i32,
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
	match &*query {
		ListQuery::Root { parent_post, seen } => {

			// Find comments to return
			let unmasked_parent_post = masking_key.unmask(&parent_post)
				.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
			let unmasked_seen: Result<Vec<Bson>, _> = seen.iter()
				.map(|masked_oid| {
						masking_key.unmask(masked_oid)
							.map_err(|_| Failure::BadRequest("bad masked id"))
							.map(|oid| Bson::ObjectId(oid))
				})
				.collect();
				let excluded_ids: Vec<Bson> = unmasked_seen?;
				let found_comments = db.collection::<Comment>("comments")
					.find(
						doc! {
							"$and": [
								{
									"_id": {
										"$not": {
											"$in": excluded_ids
										}
									}
								},
								{
									"parent_post": unmasked_parent_post
								},
								{
									"parent_comments": {
										"$size": 0
									}
								}
							]
						},
						FindOptions::builder()
							.sort(doc! {}) // todo: add sort to comments (recents, top voted, etc.)
							.limit(i64::from(conf::POSTS_PAGE_SIZE))
							.build()
					)
					.await
					.map_err(to_unexpected!("Getting comments cursor failed"))?
					.map_ok(|comment| Ok(CommentDetail {
						id: masking_key.mask(&comment.id),
						parent_comments: comment
							.parent_comments
							.iter()
							.map(|id| masking_key.mask(&id))
    					.collect(),
						parent_post: masking_key.mask(&comment.parent_post),
						text: if comment.deleted {"[deleted]".to_string()} else {comment.text} ,
						replies: comment.replies,
					}))
					.try_collect::<Vec<Result<CommentDetail, Failure<()>>>>()
					.await
					.map_err(to_unexpected!("Getting comments failed"))?
					.into_iter()
					.collect::<Result<Vec<CommentDetail>, Failure<()>>>()?;
				return success(Box::new(found_comments));
		},
    ListQuery::Thread { parent_comment, seen } => todo!(),
	};
}


