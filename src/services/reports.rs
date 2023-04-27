use std::convert::TryFrom;

use actix_web::web;
use actix_web::{get, post, put};
use futures::TryStreamExt;
use log::{debug, error, info};
use mongodb::bson::{doc, DateTime, Document};
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::options::{FindOneOptions, FindOptions};
use mongodb::{Client as MongoClient, Database};
use serde::{Deserialize, Serialize};

use crate::api_types::{success, ApiResult, Failure};
use crate::auth::AuthenticatedUser;
use crate::conf;
use crate::masked_oid::{self, MaskedObjectId, MaskedSequentialId, MaskingKey};
use crate::to_unexpected;
use crate::types::{Post, Vote};

// todo: make for comments & posts

#[put("/posts/{post_id}/report")]
pub async fn report_post(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	post_id: web::Path<MaskedObjectId>,
) -> ApiResult<(), ()> {
	success(())
}

#[get("/posts/reports/")]
pub async fn get_reported_posts(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	post_id: web::Path<MaskedObjectId>,
) -> ApiResult<(), ()> {
	success(())
}

#[derive(Deserialize)]
pub struct RemoveRequest {
	pub removed: bool,
	pub post_id: MaskedObjectId,
}

// todo: make admin-only
#[put("/posts/{post_id}/remove")]
pub async fn remove_post(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	remove_req: web::Path<RemoveRequest>,
) -> ApiResult<(), ()> {
	// Unmask the ID, in order for it to be used for querying.
	let post_id = masking_key
		.unmask(&remove_req.post_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
	// Query the database for the post and update it with new `removed` status.
	let possible_post = db
		.collection::<Post>("posts")
		.update_one(doc! {"_id": post_id}, doc! {"removed": &remove_req.removed}, None)
		.await;
	match possible_post {
		Ok(update_result) => if update_result.matched_count == 0 {
			return Err(Failure::BadRequest("no post found"));
		} else {
			return success(());
		}
		Err(_) => return Err(Failure::Unexpected),
	};
}
