use actix_web::{
	get,
	post,
};
use actix_web::web;
use futures::{
	TryStreamExt,
};
use log::{
	info,
	error,
};
use mongodb::Database;
use mongodb::bson::{
	Document,
	doc,
};
use mongodb::error::{
	ErrorKind,
	WriteFailure,
};
use mongodb::options::{
	FindOptions,
};
use serde::{Deserialize, Serialize};

use crate::{
	to_unexpected,
};
use crate::auth::AuthenticatedUser;
use crate::api_types::{
	ApiResult,
	Failure,
	success,
};
use crate::conf;
use crate::masked_oid::{
	self,
	MaskingKey,
	MaskedObjectId,
	MaskedSequentialId,
};
use crate::types::Post;

#[derive(Serialize)]
pub struct ReplyContext {
	pub id: MaskedObjectId,
}

#[derive(Serialize)]
pub struct Detail {
	pub id: MaskedObjectId,
	pub sequential_id: MaskedSequentialId,
	pub reply_context: Option<ReplyContext>,
	pub text: String,
	pub created_at: String,
	pub score: i32,
}

#[derive(Deserialize)]
#[serde(tag = "sort", rename_all = "kebab-case")]
pub enum ListQuery {
	Recent {
		before: Option<MaskedSequentialId>,
	},
	Trending,
}

#[derive(Serialize)]
pub struct Created {
	pub id: MaskedObjectId,
}

#[derive(Deserialize)]
pub struct CreateRequest {
	pub text: String,
}

#[get("/posts/")]
pub async fn list(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	query: web::Query<ListQuery>,
) -> ApiResult<Box<[Detail]>, ()> {
	match &*query {
		ListQuery::Recent { before } => {
			let find_query = match before {
				None => doc! {},
				Some(before) => {
					let before =
						masking_key.unmask_sequential(before)
						.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked sequential id"))?;
					doc! {
						"sequential_id": {"$lt": i64::try_from(before).unwrap()},
					}
				}
			};

			let posts =
				db.collection::<Post>("posts")
					.find(
						find_query,
						FindOptions::builder()
							.sort(doc! {"sequential_id": -1})
							.limit(i64::from(conf::POSTS_PAGE_SIZE))
							.build()
					)
					.await
					.map_err(to_unexpected!("Getting recent posts cursor failed"))?
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
						score: post.score,
					}))
					.try_collect::<Vec<Result<Detail, Failure<()>>>>()
					.await
					.map_err(to_unexpected!("Getting recent posts cursor failed"))?
					.into_iter()
					.collect::<Result<Vec<Detail>, Failure<()>>>()?;

			success(posts.into())
		}
		ListQuery::Trending => {
			error!("Trending sort not implemented");
			Err(Failure::Unexpected)
		}
	}
}

#[post("/posts/")]
pub async fn create(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	request: web::Json<CreateRequest>,
) -> ApiResult<Created, ()> {
	if request.text.len() > conf::POST_MAX_SIZE {
		return Err(Failure::BadRequest("oversized post text"));
	}

	let mut insert_doc = doc! {
		"owner": &user.id,
		"text": &request.text,
		"score": 0,
	};
	let mut attempt = 0;
	let insertion = loop {
		attempt += 1;
		if attempt > 100 {
			error!("Too many post creation attempts");
			return Err(Failure::Unexpected);
		}

		let last_sequential_id =
			db.collection::<Post>("posts")
			.aggregate(
				[
					doc! {"$sort": {"sequential_id": -1}},
					doc! {"$limit": 1},
					doc! {"$project": {"_id": false, "sequential_id": true}},
				],
				None
			)
			.await
			.map_err(to_unexpected!("Getting next post sequential id cursor failed"))?
			.try_next()
			.await
			.map_err(to_unexpected!("Getting next post sequential id failed"))?
			.map(|doc| doc.get_i32("sequential_id").unwrap());

		let new_sequential_id = last_sequential_id.unwrap_or(0) + 1;
		insert_doc.insert("sequential_id", new_sequential_id);

		match db.collection::<Document>("posts").insert_one(&insert_doc, None).await {
			Ok(insertion) => break insertion,
			Err(err) => {
				match err.kind.as_ref() {
					ErrorKind::Write(WriteFailure::WriteError(write_err)) if write_err.code == 11000 => {
						info!("Retrying post creation: {}", err);
					}
					_ => {
						error!("Creating post failed: {}", err);
						return Err(Failure::Unexpected);
					}
				}
			}
		}
	};

	success(
		Created {
			id: masking_key.mask(&insertion.inserted_id.as_object_id().unwrap()),
		}
	)
}
