use actix_web::{
	get,
	post,
	put, web,
};
use mongodb::{Database, bson::doc};

use crate::{masked_oid::{MaskingKey}, api_types::{ApiResult, Failure, success}, to_unexpected, auth::AuthenticatedUser};

// TODO: implement route
#[get("/comments/")]
pub async fn get_comment(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
) -> ApiResult<(), ()> {
	success(())
}

// TODO: implement route
#[post("/comments/")]
pub async fn create_comment(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
) -> ApiResult<(), ()> {
	success(())
}
