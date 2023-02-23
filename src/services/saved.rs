use actix_web::http::StatusCode;
use actix_web::{
	get,
	post,
	delete
};
use actix_web::web;
use mongodb::{
	Database,
};
use mongodb::bson::{
	DateTime,
	Document,
	doc, to_bson,
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
pub struct SavedContentRequest {
	pub content_type: SavedType,
	pub content_id: MaskedObjectId,
}

// TODO: is is necessary to ensure the user is saving either a `Comment` or `Post`?
#[post("/users/saved/")]
pub async fn save_content(
	db: web::Data<Database>,
	user: AuthenticatedUser,
	request: web::Json<SavedContentRequest>,
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
	request: web::Json<SavedContentRequest>,
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

#[get("/users/saved/")]
pub async fn get_content(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
) -> ApiResult<(), ()> {
	todo!()
}
