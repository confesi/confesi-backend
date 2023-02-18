use std::convert::TryFrom;

use actix_web::{
	get,
	post,
	put,
};
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
	DateTime,
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
	success,
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

#[post("/comments/")]
async fn create_comment(
    db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
    user: AuthenticatedUser,
	comment: web::Json<CreateComment>,
) -> ApiResult<(), ()> {
    // TODO: check comment length + validation
    // TODO: can you pass it masked id from a different collection?
    let post_id = masking_key.unmask(&*&comment.post_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?;
    db.collection::<Comment>("comments")
        .insert_one(Comment {
            parent_comment_id: match &comment.parent_comment_id {
                Some(parent_comment_id) => Some(masking_key.unmask(parent_comment_id)
                .map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked id"))?),
                None => None,
            },
            post_id: post_id,
            user_id: user.id,
            text: (&*comment.text).to_string(),
        }, None)
        .await
        .map_err(|_| Failure::Unexpected)?;
	success(())
}