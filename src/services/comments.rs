// todo: do comments even need a sequential id?

use actix_web::{
  get,
  post,
  web,
};
use futures::TryStreamExt;
use log::{error, debug};
use mongodb::{Database, bson::{doc, Document}, options::TransactionOptions, Client as MongoClient};
use serde::Deserialize;

use crate::{masked_oid::{MaskingKey, MaskedObjectId, self}, api_types::{ApiResult, Failure, success}, to_unexpected, auth::AuthenticatedUser, services::posts::Created, conf, types::Comment};

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

// TODO: implement route
#[get("/comments/")]
pub async fn get_comment(
  db: web::Data<Database>,
  masking_key: web::Data<&'static MaskingKey>,
) -> ApiResult<(), ()> {
  success(())
}


