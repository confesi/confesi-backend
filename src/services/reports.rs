use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::{get, put};
use log::{error, debug};
use mongodb::bson::doc;
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::{Client as MongoClient, Database};
use serde::{Deserialize, Serialize};

use crate::api_types::{failure, success, ApiError, ApiResult, Failure};
use crate::auth::AuthenticatedUser;
use crate::masked_oid::{self, MaskedObjectId, MaskingKey};
use crate::types::{Post, Report, ReportCategory};
use crate::{conf, to_unexpected};

// todo: make for comments & posts

#[derive(Debug, Serialize)]
pub enum ReportError {
	AlreadyReported,
}

impl ApiError for ReportError {
	fn status_code(&self) -> StatusCode {
		match self {
			Self::AlreadyReported => StatusCode::CONFLICT,
		}
	}
}

#[derive(Deserialize)]
pub struct ReportRequest {
	reason: String,
	category: ReportCategory,
}

#[put("/posts/{post_id}/report")]
pub async fn report_post(
	mongo_client: web::Data<MongoClient>,
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	post_id: web::Path<MaskedObjectId>,
	report: web::Json<ReportRequest>,
	user: AuthenticatedUser,
) -> ApiResult<(), ReportError> {
	if report.reason.len() > conf::REPORT_REASON_MAX_SIZE {
		return Err(Failure::BadRequest("oversized reason text"));
	}

	let post_id = masking_key
		.unmask(&post_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

	let mut session = mongo_client
		.start_session(None)
		.await
		.map_err(to_unexpected!("Starting session failed"))?;

	session
		.start_transaction(None)
		.await
		.map_err(to_unexpected!("Starting transaction failed"))?;

	let report = db
		.collection::<Report>("reports")
		.insert_one_with_session(
			Report {
				post: post_id,
				reason: report.reason.clone(),
				category: report.category.clone(),
				user: user.id.clone(),
			},
			None,
			&mut session,
		)
		.await;
	return match report {
		Ok(_) => {
			let increment_result = db
				.collection::<Post>("posts")
				.update_one_with_session(
					doc! {"_id": post_id},
					doc! {"$inc": {"reports": 1}},
					None,
					&mut session,
				)
				.await;
			return match increment_result {
				Ok(update_result) => {
					if update_result.modified_count > 0 {
						if let Err(err) = session.commit_transaction().await {
							debug!("Committing voting transaction failed: {}", err);
							Err(Failure::Unexpected)
						} else {
							success(())
						}
					} else {
						Err(Failure::Unexpected)
					}
				}
				Err(err) => {
					error!("Incrementing report count failed: {}", err);
					return Err(Failure::Unexpected);
				}
			};
		}
		Err(err) => match err.kind.as_ref() {
			ErrorKind::Write(WriteFailure::WriteError(err)) if err.code == 11000 => {
				failure(ReportError::AlreadyReported)
			}
			_ => {
				error!("Inserting report failed: {}", err);
				Err(Failure::Unexpected)
			}
		},
	};
}

#[get("/posts/reports/")]
pub async fn get_reported_posts(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	post_id: web::Path<MaskedObjectId>,
) -> ApiResult<(), ()> {
	success(())
}

// todo: make admin-only
#[put("/posts/{post_id}/remove")]
pub async fn remove_post(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	removed: web::Json<bool>,
	post_id: web::Path<MaskedObjectId>,
) -> ApiResult<(), ()> {
	// Unmask the ID, in order for it to be used for querying.
	let post_id = masking_key
		.unmask(&post_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
	// Query the database for the post and update it with new `removed` status.
	let possible_post = db
		.collection::<Post>("posts")
		.update_one(
			doc! {"_id": post_id},
			doc! {"$set": {"removed": &*removed}},
			None,
		)
		.await;
	match possible_post {
		Ok(update_result) => {
			if update_result.matched_count == 0 {
				return Err(Failure::BadRequest("no post found"));
			} else {
				return success(());
			}
		}
		Err(err) => {
			error!("error updating post: {}", err);
			return Err(Failure::Unexpected);
		}
	};
}
