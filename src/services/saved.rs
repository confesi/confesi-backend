use actix_web::http::StatusCode;
use actix_web::{
	get,
	post,
	delete
};
use actix_web::web;
use chrono::Utc;
use futures::{StreamExt};
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
use crate::conf;
use crate::masked_oid::{
	self,
	MaskingKey,
	MaskedObjectId,
};
use crate::types::{
 SavedType, SavedContent, Post, Rfc3339DateTime,
};

use super::posts::Detail;

// The unique error(s) that can occur when saving content.
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

/// Allows a user to save comments or posts to view later.
#[post("/users/saved/")]
pub async fn save_content(
	db: web::Data<Database>,
	user: AuthenticatedUser,
	request: web::Json<SaveContentRequest>,
	masking_key: web::Data<&'static MaskingKey>,
) -> ApiResult<(), SaveError> {

	// First, verifies the the passed `content_id` actually exists in the
	// corresponding `SavedType` (comments or posts) collection, else throw a 400.

	let content_id = masking_key.unmask(&request.content_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

	let collection_to_verify = match &request.content_type {
		// TODO: Change this to a `Comment` collection once it is implemented.
		SavedType::Comment => db.collection::<Post>("posts"),
		SavedType::Post => db.collection::<Post>("posts"),
	};

	match collection_to_verify.find_one(doc! {"_id": {"$eq": content_id}}, None).await {
		Ok(possible_content) => if let None = possible_content {return Err(Failure::BadRequest("content doesn't exist as specified type"))},
		Err(_) => return Err(Failure::Unexpected),
	}

	// Save the content to the `saved` collection.

	let content_type_bson = to_bson(&request.content_type).map_err(|_| Failure::Unexpected)?;

	let content_object_id = masking_key.unmask(&request.content_id)
	.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;

	let content_object_id_bson = to_bson(&content_object_id).map_err(|_| Failure::Unexpected)?;

	// Document that respresents a saved bit of content.
	let content_to_be_saved = doc! {
		"user_id": user.id,
		"content_type": content_type_bson,
		"content_id": content_object_id_bson,
		"saved_at": DateTime::now(),
	};

	// Insert the saved content reference to the database. If it already exists (content already saved),
	// then throw a custom 409 error ("AlreadySaved"). If successful, return a 200. If unknown error occurs,
	// then throw a 500.
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

/// Allows a user to delete a comment or post they've previously saved.
#[delete("/users/saved/")]
pub async fn delete_content(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	request: web::Json<SaveContentRequest>,
) -> ApiResult<(), ()> {

	// Unmasks ID of content to be deleted.
	let content_object_id = masking_key.unmask(&request.content_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
	let content_id_bson = to_bson(&content_object_id).map_err(|_| Failure::Unexpected)?;

	// Document that'll match against content to be deleted.
	let to_delete_document = doc! {
		"content_id": content_id_bson,
		"user_id": user.id
	};

	// Upon successful deletion, returns a 200. If nothing is deleted, but no errors are thrown,
	// then a 400 is returned because the resource didn't exist in the first place. If
	// something else goes wrong, a 500 is returned.
	match db.collection::<SavedContent>("saved").delete_one(to_delete_document, None).await {
		Ok(delete_result) => if delete_result.deleted_count == 1 {return success(())} else {return Err(Failure::BadRequest("this saved content doesn't exist"))},
		Err(_) => return Err(Failure::Unexpected),
	}
}

/// The two ways you can filter viewing your saved content.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Filter {
	Comments,
	Posts,
}

/// The query request for fetching your saved content.
#[derive(Deserialize)]
pub struct FetchSavedRequest {
	pub filter: Filter,
	pub after_date: Option<String>,
	pub after_id: Option<MaskedObjectId>
}

/// Type of return for fetching saved content.
///
/// Returns the desired saved content in a vector.
///
/// Also returns `after_date` and `after_id`, which allow you to call for the next set of data.
#[derive(Serialize)]
pub struct SavedContentDetail {
	pub after_date: Option<Rfc3339DateTime>,
	pub after_id: Option<MaskedObjectId>,
	#[serde(skip_serializing_if = "<[_]>::is_empty")]
	pub posts: Vec<Detail>,
	#[serde(skip_serializing_if = "<[_]>::is_empty")]
	pub comments: Vec<Detail>, // TODO: Make for Comment once commenting is implemented.
}


/// The query request for fetching a user's saved content.
///
/// Allows you to specify a `filter` to determine if you want comments or posts returned.
///
/// Also allows for you to optionally add an `after_date` and `after_id` to return content
/// after these 2 fields (static cursor-based pagination). Requires both because `ObjectId`s alone
/// aren't fully accurate, and `after_date` isn't guaranteed to be unique.
///
/// Technically speaking, it doesn't look for `ObjectId`s AFTER `after_id`, it just ensures
/// that it doesn't return the same `ObjectId`, hence allowing you to not miss any bits of content
/// with duplicate saved-dates. Named `after_id` for simplicity.
#[get("/users/saved/")]
pub async fn get_content(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	query: web::Query<FetchSavedRequest>,
) -> ApiResult<Box<SavedContentDetail>, ()> {

	// The two filters used to determine which batch of documents to return.
	let mut date_filter = doc! {};
	let mut id_filter = doc! {};

	// Counts how many filters are applied via query params.
	let mut filters_added = 0;

	// Sets the date filter.
	if let Some(datetime) = &query.after_date {
		filters_added += 1;
    match datetime.parse::<chrono::DateTime<Utc>>() {
			Ok(date) => {
				let date = Bson::DateTime(DateTime::from_millis(date.timestamp_millis()));
				date_filter = doc! {"saved_at": { "$lte": date }};
			},
			Err(_) => return Err(Failure::BadRequest("invalid date")),
		}
	}

	// Sets the id filter.
	if let Some(id) = &query.after_id {
		filters_added += 1;
		let unmasked_id = masking_key.unmask(id)
			.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
		id_filter = doc! {"_id": { "$ne": unmasked_id }};
	}

	// You can either use no filters (to get the first bit of data), or both filters (to get subsequent data).
	// Using only 1 filter doesn't guarentee accurate data, thus, doing so yields a 400-level response.
	if filters_added != 2 && filters_added != 0 {return Err(Failure::BadRequest("must use no filters, or both"))}

	// Determines which collection to do the `$lookup`s from.
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

	// Determines which fields to return using `$project`.
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

	// The aggregation pipeline.
	let pipeline = vec![
		// Sorts by `saved_at` date.
		doc! { "$sort": {"saved_at": -1 }},
		// Applies the filters determined above via query params. Also ensures a user only
		// can search through their own saved posts.
		doc! {
			"$match": {
					"$and": [
							id_filter,
							date_filter,
							{ "user_id": { "$eq": user.id } }
					]
			}
		},
		// Applies the `$lookup` and `$project` stages created above.
		lookup_comments_or_posts,
		projection,
		// Limits the number of results returned.
		doc! { "$limit":  conf::SAVED_CONTENT_PAGE_SIZE },
	];

	// Executes query.
	let cursor = db.collection::<SavedContent>("saved").aggregate(pipeline, None).await;

	// Found posts.
	let mut posts: Vec<Detail> = Vec::new();

	// Found comments.
	// TODO: Change this to a `Comment` vector once commenting is implemented.
	let comments: Vec<Detail> = Vec::new();

	// The two cursors for retrieving subsequent data, explicitly set to `None` to start.
	let mut date_after: Option<Rfc3339DateTime> = None;
	let mut id_after: Option<MaskedObjectId> = None;

	match cursor {
		Ok(mut cursor) => {
				while let Some(content) = cursor.next().await {
						match content {
								Ok(content) => {

									// Convert results found to `SavedContent` type.
									let saved_content: SavedContent = bson::from_bson(Bson::Document(content)).map_err(|_| return Failure::Unexpected)?;

									// Set the cursors with the newest found details.
									date_after = Some(saved_content.saved_at);
									id_after = Some(masking_key.mask(&saved_content.id));

									// If it has posts, add them to the `posts` vector to be returned.
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

									// TODO: Implement adding `Comment`s to the comments vector once commenting is implemented.
								}
								Err(_) => return Err(Failure::Unexpected),
						}
				}
		}
		Err(_) => return Err(Failure::Unexpected),
	}
	// Return resulting vector of either comments or posts alongside cursors needed
	// to access the next set of data.
	success(Box::new(SavedContentDetail {
		after_id: id_after,
		after_date: date_after,
		comments: comments,
		posts: posts,
	}))
}
