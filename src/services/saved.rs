use actix_web::http::StatusCode;
use chrono::{Utc, TimeZone, ParseError};
use actix_web::{
	get,
	post,
	delete
};
use actix_web::web;
use futures::TryStreamExt;
use mongodb::options::FindOptions;
use mongodb::{
	Database,
	bson
};
use mongodb::bson::{
	DateTime,
	Document,
	doc, to_bson, Bson,
};
use mongodb::error::{
	ErrorKind,
	WriteFailure,
};
use serde::{Deserialize, Serialize};

use crate::auth::AuthenticatedUser;
use crate::api_types::{
	ApiResult,
	Failure,
	success, ApiError, failure,
};
use crate::services::posts::Detail;
use crate::{conf, to_unexpected};
use crate::masked_oid::{
	self,
	MaskingKey,
	MaskedObjectId,
};
use crate::types::{
 SavedType, SavedContent,
};

#[derive(Debug, Serialize)]
pub enum SaveError {
	AlreadySaved,
}

impl ApiError for SaveError {
	fn status_code(&self) -> StatusCode {
		match self {
			Self::AlreadySaved => StatusCode::CONFLICT,
		}
	}
}


#[derive(Deserialize)]
pub struct SaveContentRequest {
	pub content_type: SavedType,
	pub content_id: MaskedObjectId,
}

// TODO: is is necessary to ensure the user is saving either a `Comment` or `Post`?
#[post("/users/saved/")]
pub async fn save_content(
	db: web::Data<Database>,
	user: AuthenticatedUser,
	request: web::Json<SaveContentRequest>,
	masking_key: web::Data<&'static MaskingKey>,
) -> ApiResult<(), SaveError> {

	// Save content
	let content_type_bson = to_bson(&request.content_type).map_err(|_| Failure::Unexpected)?;

	let content_object_id = masking_key.unmask(&request.content_id)
	.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

	let content_id_bson = to_bson(&content_object_id).map_err(|_| Failure::Unexpected)?;

	let content_to_be_saved = doc! {
		"user_id": user.id,
		"content_type": content_type_bson,
		"content_id": content_id_bson,
		"saved_at": DateTime::now(),
	};
	match db.collection::<Document>("saved").insert_one(content_to_be_saved, None).await {
		Ok(_) => return success(()),
		Err(err) => {
			match err.kind.as_ref() {
				ErrorKind::Write(WriteFailure::WriteError(write_err)) if write_err.code == 11000 => {
					failure(SaveError::AlreadySaved)
				}
				_ => {
					return Err(Failure::Unexpected);
				}
			}
		}
	}
}

#[delete("/users/saved/")]
pub async fn delete_content(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	request: web::Json<SaveContentRequest>,
) -> ApiResult<(), ()> {

	let content_object_id = masking_key.unmask(&request.content_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

	let content_id_bson = to_bson(&content_object_id).map_err(|_| Failure::Unexpected)?;

	let to_delete_document = doc! {
		"content_id": content_id_bson,
		"user_id": user.id
	};

	match db.collection::<SavedContent>("saved").delete_one(to_delete_document, None).await {
		Ok(delete_result) => if delete_result.deleted_count == 1 {return success(())} else {return Err(Failure::BadRequest("this saved content doesn't exist"))},
		Err(_) => return Err(Failure::Unexpected),
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Filter {
	Comments,
	Posts,
	Mixed,
}

#[derive(Deserialize)]
pub struct FetchSavedRequest {
	pub filter: Filter,
	pub after: Option<String>
}

#[derive(Serialize)]
pub struct FetchSavedDetail {
	// pub results: Vec<>
}


#[get("/users/saved/")]
pub async fn get_content(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	query: web::Query<FetchSavedRequest>,
) -> ApiResult<Box<[Detail]>, ()> {
	let c = conf::SAVED_CONTENT_PAGE_SIZE;
	let filter: Document;
	if let Some(datetime) = &query.after {
    match datetime.parse::<chrono::DateTime<Utc>>() {
			Ok(date) => {
				let date = Bson::DateTime(DateTime::from_millis(date.timestamp_millis()));
				filter = doc! {"date": { "$gt": date }};
			},
			Err(_) => return Err(Failure::BadRequest("invalid date")),
		}
	} else {
		filter = doc! {}
	}

	let options = FindOptions::builder()
        .limit(5)
        .sort(doc! { "date": -1 })
        .build();

	db.collection::<SavedContent>("saved").find(filter, options).await
			.map_err(to_unexpected!("Getting posts cursor failed"))?
			.map_ok(|post| Ok(Detail {
				id: masking_key.mask(&post.id),
				sequential_id: masking_key.mask_sequential(u64::try_from(post.sequential_id).unwrap()),
				reply_context: None,
				text: post.text,
				created_at: (
					post.id.timestamp()
						.try_to_rfc3339_string()
						.map_err(to_unexpected!("Formatting post timestamp failed"))?
				),
				votes: Votes {
					up: u32::try_from(post.votes_up).unwrap(),
					down: u32::try_from(post.votes_down).unwrap(),
				},
			}))
			.try_collect::<Vec<Result<Detail, Failure<()>>>>()
			.await
			.map_err(to_unexpected!("Getting posts failed"))?
			.into_iter()
			.collect::<Result<Vec<Detail>, Failure<()>>>()?;

	println!("RESULTS: {:?}", {&query.after});

	success(())
}
