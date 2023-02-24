use actix_web::http::StatusCode;
use actix_web::{
	get,
	post,
	delete
};
use actix_web::web;
use chrono::Utc;
use futures::{StreamExt};
use mongodb::options::{FindOptions, AggregateOptions};
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
use crate::services::posts::Votes;
use crate::{to_unexpected, conf};
use crate::masked_oid::{
	self,
	MaskingKey,
	MaskedObjectId,
};
use crate::types::{
 SavedType, SavedContent, Post, Rfc3339DateTime,
};

use super::posts::Detail;

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

	// First verify the the passed `content_id` actually exists in the
	// corresponding `SavedType` collection, else throw a 400.

	let content_id = masking_key.unmask(&request.content_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

		let collection_to_verify = match &request.content_type {
			SavedType::Comment => db.collection::<Post>("posts"), // TODO: change this to `Comment` collection once it is implemented.
			SavedType::Post => db.collection::<Post>("posts"),
		};

	match collection_to_verify.find_one(doc! {"_id": {"$eq": content_id}}, None).await {
		Ok(possible_post) => if let None = possible_post {return Err(Failure::BadRequest("content doesn't exist as specified type"))},
		Err(_) => return Err(Failure::Unexpected),
	}

	// Save the content to the `saved` collection.

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
}

#[derive(Deserialize)]
pub struct FetchSavedRequest {
	pub filter: Filter,
	pub after: Option<String>
}

#[derive(Serialize)]
pub struct SavedContentDetail {
	pub after: Option<Rfc3339DateTime>,
	#[serde(skip_serializing_if = "<[_]>::is_empty")]
	pub posts: Vec<Detail>,
	#[serde(skip_serializing_if = "<[_]>::is_empty")]
	pub comments: Vec<Detail>, // TODO: make into `Comment`
}


// TODO: sort by 2 fields (objID & date)
// TODO: move `Detail` to `types.rs`?
#[get("/users/saved/")]
pub async fn get_content(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	query: web::Query<FetchSavedRequest>,
) -> ApiResult<Box<SavedContentDetail>, ()> {
	let date_filter: Document;
	if let Some(datetime) = &query.after {
    match datetime.parse::<chrono::DateTime<Utc>>() {
			Ok(date) => {
				let date = Bson::DateTime(DateTime::from_millis(date.timestamp_millis()));
				date_filter = doc! {"saved_at": { "$gt": date }};
			},
			Err(_) => return Err(Failure::BadRequest("invalid date")),
		}
	} else {
		date_filter = doc! {}
	}

	let lookup_comments_or_posts = match query.filter {
    Filter::Posts => doc! {
        "$lookup": {
            "from": "posts",
            "localField": "content_id",
            "foreignField": "_id",
            "as": "post"
        }
    },
    Filter::Comments => doc! {
        "$lookup": {
            "from": "comments",
            "localField": "content_id",
            "foreignField": "_id",
            "as": "comment"
        }
    }
	};

	// TODO: make more idiomatic - DRY principle
	let projection = match query.filter {
		Filter::Posts => doc! {
			"$project": {
					"_id": 1,
					"user_id": 1,
					"content_type": 1,
					"content_id": 1,
					"saved_at": 1,
					"post": {
							"$arrayElemAt": ["$post", 0]
					},
			}
		},
		Filter::Comments => doc! {
			"$project": {
					"_id": 1,
					"user_id": 1,
					"content_type": 1,
					"content_id": 1,
					"saved_at": 1,
					"comment": {
							"$arrayElemAt": ["$comment", 0]
					}
			}
		},
	};

	let pipeline = vec![
		doc! {
			"$match": {
					"$and": [
							date_filter,
							{ "user_id": { "$eq": user.id } }
					]
			}
		},
		lookup_comments_or_posts,
		projection,
		doc! { "$limit":  conf::SAVED_CONTENT_PAGE_SIZE }
	];

	// TODO: sort results?

	let cursor = db.collection::<SavedContent>("saved").aggregate(pipeline, None).await;

	let mut posts: Vec<Detail> = Vec::new();

	let mut comments: Vec<Detail> = Vec::new(); // todo: change to `Comments` type when they're implemented

	let mut content_after: Option<Rfc3339DateTime> = None;

	match cursor {
		Ok(mut cursor) => {
				while let Some(content) = cursor.next().await {
						match content {
								Ok(content) => {
									let saved_content: SavedContent = bson::from_bson(Bson::Document(content)).map_err(|_| return Failure::Unexpected)?;
									content_after = Some(saved_content.saved_at);
									// If it has posts
									// TODO: implement comments
									if let Some(post) = saved_content.post {
										posts.push(Detail {
											id: masking_key.mask(&post.id),
											sequential_id: masking_key.mask_sequential(u64::try_from(post.sequential_id).unwrap()),
											reply_context: None,
											text: post.text,
											created_at: (
												post.id.timestamp()
													.try_to_rfc3339_string()
													.map_err(|_| return Failure::Unexpected)?
											),
											votes: Votes {
												up: u32::try_from(post.votes_up).unwrap(),
												down: u32::try_from(post.votes_down).unwrap(),
											},
										});
									}
								}
								Err(_) => return Err(Failure::Unexpected),
						}
				}
		}
		Err(_) => return Err(Failure::Unexpected),
	}
	success(Box::new(SavedContentDetail {
		after: content_after,
		comments: comments,
		posts: posts,
	}))
}

// TODO: remove unnecessary `Serialize`s
