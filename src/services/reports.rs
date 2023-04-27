use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::{get, put};
use futures::TryStreamExt;
use log::{debug, error};
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{doc, Document};
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::options::FindOptions;
use mongodb::{Client as MongoClient, Database};
use serde::{Deserialize, Serialize};

use crate::api_types::{failure, success, ApiError, ApiResult, Failure};
use crate::auth::AuthenticatedUser;
use crate::masked_oid::{self, MaskedObjectId, MaskingKey};
use crate::services::posts::Votes;
use crate::types::{Post, Report, ReportCategory};
use crate::{conf, to_unexpected};

use super::posts::Detail;

// todo: ensure routes that need admin access have it
// todo: update new docs and old docs that got changed by this
// todo: add/update required indices

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

	let next_sequential_id = db
		.collection::<Report>("reports")
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
			"Getting next reports sequential id cursor failed"
		))?
		.try_next()
		.await
		.map_err(to_unexpected!("Getting next report's sequential id failed"))?
		.map(|doc| doc.get_i32("sequential_id").unwrap())
		.unwrap_or(0)
		+ 1;

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
				sequential_id: next_sequential_id,
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
					error!("Incrementing report count failed on post: {}", err);
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

#[derive(Serialize)]
pub struct ReportDetailItem {
	reason: String,
	category: ReportCategory,
}

#[derive(Serialize)]
pub struct ReportDetail {
	reports: Vec<ReportDetailItem>,
	next: Option<i32>,
}

#[get("/reports/{post_id}/")]
pub async fn get_reports_from_post(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	post_id: web::Path<MaskedObjectId>,
	next: web::Json<u32>,
) -> ApiResult<Box<ReportDetail>, ()> {
	let post_id = masking_key
		.unmask(&post_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
	let mut last_sequential_id: Option<i32> = None;
	let reports = db
		.collection::<Report>("reports")
		.find(
			doc! {"post": post_id, "sequential_id": { "$gt": &*next }},
			FindOptions::builder()
				.sort(doc! {"sequential_id": 1})
				.limit(i64::from(conf::REPORT_DETAILS_PAGE_SIZE))
				.build(),
		)
		.await
		.map_err(to_unexpected!("Getting reports cursor failed"))?
		.map_ok(|report| {
			last_sequential_id = Some(report.sequential_id);
			Ok(ReportDetailItem {
				reason: report.reason,
				category: report.category,
			})
		})
		.try_collect::<Vec<Result<ReportDetailItem, Failure<()>>>>()
		.await
		.map_err(to_unexpected!("Getting reports failed"))?
		.into_iter()
		.collect::<Result<Vec<ReportDetailItem>, Failure<()>>>()?;
	success(Box::new(ReportDetail {
		reports,
		next: last_sequential_id,
	}))
}

#[put("/posts/{post_id}/remove")]
pub async fn remove_post(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	removed: web::Json<bool>,
	post_id: web::Path<MaskedObjectId>,
) -> ApiResult<(), ()> {
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

#[derive(Deserialize)]
pub struct ListReportsRequest {
	seen: Vec<MaskedObjectId>,
	min_reports: u32,
	show_removed: bool,
}

#[derive(Serialize)]
pub struct ReportedPostDetail {
	post: Detail,
	reports: i32,
	removed: bool,
}

#[get("/reports/posts/")]
pub async fn get_reported_posts(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	request: web::Json<ListReportsRequest>,
) -> ApiResult<Vec<ReportedPostDetail>, ()> {
	let unmasked_seen: Vec<ObjectId> = request
		.seen
		.iter()
		.map(|masked_id| {
			masking_key
				.unmask(masked_id)
				.map_err(|_| Failure::BadRequest("bad masked id"))
		})
		.collect::<Result<Vec<_>, _>>()?;

	let mut sort: Document = doc! {
			"reports": { "$gte": request.min_reports },
			"_id": { "$nin": unmasked_seen }
	};

	if !request.show_removed {
			sort.insert("removed", doc! { "$ne": true });
	}

	let reports = db
		.collection::<Post>("posts")
		.find(
			sort,
			FindOptions::builder()
				.sort(doc! {"reports": -1})
				.limit(i64::from(conf::REPORTED_POSTS_PAGE_SIZE))
				.build(),
		)
		.await
		.map_err(to_unexpected!("Getting reports cursor failed"))?
		.map_ok(|post| {
			Ok(ReportedPostDetail {
				post: Detail {
					id: masking_key.mask(&post.id),
					sequential_id: masking_key
						.mask_sequential(u64::try_from(post.sequential_id).unwrap()),
					reply_context: None,
					text: post.text,
					created_at: (post
						.id
						.timestamp()
						.try_to_rfc3339_string()
						.map_err(to_unexpected!("Formatting post timestamp failed"))?),
					votes: Votes {
						up: u32::try_from(post.votes_up).unwrap(),
						down: u32::try_from(post.votes_down).unwrap(),
					},
				},
				removed: post.removed,
				reports: post.reports,
			})
		})
		.try_collect::<Vec<Result<ReportedPostDetail, Failure<()>>>>()
		.await
		.map_err(to_unexpected!("Getting posts failed"))?
		.into_iter()
		.collect::<Result<Vec<ReportedPostDetail>, Failure<()>>>()?;
	success(reports)
}
