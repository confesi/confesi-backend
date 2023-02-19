use std::convert::TryFrom;
use actix_web::guard::fn_guard;
use chrono;
use actix_web::{
	get,
	post,
	put,
};
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{DateTime};
use actix_web::web;
use futures::{
	TryStreamExt,
};
use log::{
	debug,
	info,
	error,
};
use mongodb::{
	Client as MongoClient,
	Database,
};
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
	FindOneOptions,
};
use serde::{Deserialize, Serialize};

use crate::{
	to_unexpected,
};
use crate::auth::AuthenticatedUser;
use crate::api_types::{
	ApiResult,
	Failure,
	success, ApiError,
};
use crate::conf;
use crate::masked_oid::{
	self,
	MaskingKey,
	MaskedObjectId,
	MaskedSequentialId,
};
use crate::types::{
	Post,
	Vote, Comment,
};

#[derive(Serialize, Deserialize)]
pub struct CreateComment {
    pub post_id: MaskedObjectId,
	pub parent_comment_id: Option<MaskedObjectId>,
    pub text: String
}

/// The various ways a user can sort comments.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CommentSort {
   Best { last_seen_absolute_score: i32 }, 
   Controversial { last_seen_absolute_score: i32 },
   Recent { last_seen_date: DateTime },
   MostLiked { last_seen_votes_up: i32 },
   MostDisliked { last_seen_votes_down: i32 },
}

#[derive(Deserialize, Debug)]
pub struct CommentQuery {
    pub sort: CommentSort,
    pub last_seen_id: Option<ObjectId>, // TODO: make MaskedObjectId
}

// TODO: create a comment detail? that has masks?
// pub struct CommentDetail {
//     pub 
// }

#[get("/comments/")]
async fn get_comments(
    query: web::Query<CommentQuery>,
) -> ApiResult<(), ()> {
    let mut find_query = Document::new();
	let sort: Document;

    match &query.sort {
        CommentSort::Best { last_seen_absolute_score } => sort = doc! {"absolute_score": -1},
        CommentSort::Controversial { last_seen_absolute_score } => sort = doc! { "absolute_score": { "$subtract": [0, "$absolute_score"] } },
        CommentSort::Recent { last_seen_date } => sort = doc! {"created_at": -1},
        CommentSort::MostLiked { last_seen_votes_up } => sort = doc! {"votes_up": -1},
        CommentSort::MostDisliked { last_seen_votes_down } => sort = doc! {"votes_down": -1},
    };

    let mut condition_1 = Document::new();
    let mut condition_2 = Document::new();

    match &query.last_seen_id {
        None => (),
        Some(post_id) => {
            condition_1.insert("_id", doc!{"$lt": post_id});
            condition_1.insert("_id", doc!{"$ne": post_id});
        },
    };

    let combined_query = doc!{
        "$or": [ condition_1, condition_2 ]
    };

    find_query.insert("$and", vec![
        combined_query,
        doc! {"parent_comment_id": { "$eq": null }}
    ]);

    println!("SORT: {}, QUERY: {}", sort, find_query);

    // masking_key.unmask(&post_id).map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?

    success(())
} 

#[post("/comments/")]
async fn create_comment(
    db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
    user: AuthenticatedUser,
	comment: web::Json<CreateComment>,
) -> ApiResult<(), ()> {
    if comment.text.len() > conf::COMMENT_MAX_SIZE {
        return Err(Failure::BadRequest("oversized comment text"))
    }
    // TODO: check comment length + validation
    // TODO: can you pass it masked id from a different collection?
    let post_id = masking_key.unmask(&*&comment.post_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
    db.collection::<Document>("comments")
        .insert_one(
            // TODO: Add "trending" category in the future?
            doc! {
                "user_id": user.id,
                "text": &*comment.text,
                "parent_comment_id": match &comment.parent_comment_id {
                    Some(parent_comment_id) => Some(masking_key.unmask(parent_comment_id)
                    .map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?),
                    None => None,
                },
                "created_at": DateTime::now(),
                "post_id": post_id,
                "votes_up": 0,
                "votes_down": 0,
                "absolute_score": 0,
            }, None)
        .await
        .map_err(|_| Failure::Unexpected)?;
	success(())
}