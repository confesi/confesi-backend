use actix_web::{
	post,
};
use actix_web::web;

use mongodb::{ Database };

use crate::api_types::{
	ApiResult,
	Failure,
	success,
};
use crate::conf;
use crate::types::{
	 Feedback,
};

/// Allows for the sending of feedback.
/// 
/// Takes header text, and body text.
#[post("/feedback/")]
pub async fn send_feedback(
    db: web::Data<Database>,
    feedback: web::Json<Feedback>,
) -> ApiResult<(), ()> {
	// Checks to see if feedback is too long, if so, returns 400.
    if feedback.body_text.len() > conf::FEEDBACK_BODY_MAX_SIZE || feedback.header_text.len() > conf::FEEDBACK_HEADER_MAX_SIZE {
        return Err(Failure::BadRequest("oversized header or body text"));
    }
	// Inserts feedback into collection.
    db.collection::<Feedback>("feedback")
        .insert_one(&*feedback, None)
        .await
        .map_err(|_| Failure::Unexpected)?;
	// Returns success (200).
	success(())
}






