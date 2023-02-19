use actix_web::{
	get,
	post,
};
use mongodb::bson::{DateTime};
use actix_web::web;
use futures::{
	TryStreamExt,
};
use log::{
	error,
};
use mongodb::options::FindOptions;
use mongodb::{
	Database,
};
use mongodb::bson::{
	Document,
	doc,
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
};
use crate::types::{
	Comment,
};

#[derive(Serialize, Deserialize)]
pub struct CreateComment {
    pub post_id: MaskedObjectId,
	pub parent_comment_id: Option<MaskedObjectId>,
    pub text: String
}

#[derive(Deserialize, Debug)]
pub struct CommentQuery {
    pub last_seen_absolute_score: Option<i32>,
    pub last_seen_id: Option<MaskedObjectId>,
    pub post_id: MaskedObjectId,
}

#[derive(Serialize)]
pub struct CommentDetail {
    pub id: MaskedObjectId,
    pub text: String,
    pub absolute_score: i32,
}

// TODO: Add sorts for: recents (time-based), liked (highest absolute), hated (lowest absolute), controversial (closest to 0 absolute). 
#[get("/comments/")]
async fn get_comments(
    db: web::Data<Database>,
    query: web::Query<CommentQuery>,
    masking_key: web::Data<&'static MaskingKey>,
) -> ApiResult<Box<[CommentDetail]>, ()> {
    let mut find_query = Document::new();
	let sort = doc! {
        "absolute_score": -1,
        "_id": -1,
    };

    let mut condition_1 = Document::new();
    let mut condition_2 = Document::new();

    match &query.last_seen_id {
        None => (),
        Some(comment_id) => {
            let comment_id = masking_key.unmask(&*&comment_id)
		        .map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
            condition_1.insert("_id", doc!{"$gt": comment_id});
            condition_2.insert("_id", doc!{"$ne": comment_id});
        },
    };

    match &query.last_seen_absolute_score {
        None => (),
        Some(votes_up) => {
            condition_1.insert("votes_up", doc! {"$lte": votes_up});
            condition_2.insert("votes_up", doc! {"$lt": votes_up});
        }
    }

    let combined_query = doc!{
        "$or": [ condition_1, condition_2 ]
    };

    let parent_post = masking_key.unmask(&*&query.post_id)
		        .map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
    find_query.insert("$and", vec![
        combined_query,
        doc! {"post_id": {"$eq": parent_post}},
        doc! {"parent_comment_id": { "$eq": null }}
    ]);

    let comments =
		db.collection::<Comment>("comments")
			.find(
				find_query,
				FindOptions::builder()
					.sort(sort)
					.limit(i64::from(conf::COMMENTS_PAGE_SIZE))
					.build()
			)
			.await
			.map_err(to_unexpected!("Getting comments cursor failed"))?
			.map_ok(|post| Ok(
                CommentDetail { id: masking_key.mask(&post.comment_id), text: post.text, absolute_score: post.absolute_score }
            ))
			.try_collect::<Vec<Result<CommentDetail, Failure<()>>>>()
			.await
			.map_err(to_unexpected!("Getting comments failed"))?
			.into_iter()
			.collect::<Result<Vec<CommentDetail>, Failure<()>>>()?;

	success(comments.into())
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