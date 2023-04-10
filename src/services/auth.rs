use actix_web::http::StatusCode;
use actix_web::post;
use actix_web::web;
use log::{debug, error};
use mongodb::bson::{doc, to_bson, DateTime, Document};
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::Database;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use crate::api_types::{failure, success, ApiError, ApiResult, Failure};
use crate::auth::{AuthenticatedUser, AuthenticationError, Authorization, Guest};
use crate::to_unexpected;
use crate::types::PrimaryEmail;
use crate::types::{
	PosterFaculty, PosterYearOfStudy, School, Session, SessionToken, User, Username,
};

#[derive(Deserialize)]
pub struct Credentials {
	username: Username,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NewUser {
	username: Username,
	// Year of study of the poster.
	pub year_of_study: Option<PosterYearOfStudy>,
	// Faculty of the poster.
	pub faculty: Option<PosterFaculty>,
	// School of the poster.
	pub school_id: String,
}

#[serde_as]
#[derive(Serialize)]
pub struct NewSession {
	#[serde_as(as = "DisplayFromStr")]
	token: SessionToken,
}

#[derive(Debug, Serialize)]
pub enum LoginError {
	UsernameNotFound,
}

impl ApiError for LoginError {
	fn status_code(&self) -> StatusCode {
		match self {
			Self::UsernameNotFound => StatusCode::BAD_REQUEST,
		}
	}
}

#[derive(Debug, Serialize)]
pub enum RegistrationError {
	UsernameTaken,
}

impl ApiError for RegistrationError {
	fn status_code(&self) -> StatusCode {
		match self {
			Self::UsernameTaken => StatusCode::CONFLICT,
		}
	}
}

#[post("/login")]
pub async fn login(
	db: web::Data<Database>,
	_guest: Guest,
	credentials: web::Json<Credentials>,
) -> ApiResult<NewSession, LoginError> {
	let user = db
		.collection::<User>("users")
		.find_one(
			doc! {"username": {"$eq": credentials.username.as_ref()}},
			None,
		)
		.await
		.map_err(to_unexpected!("Finding user failed"))?
		.ok_or(Failure::Expected(LoginError::UsernameNotFound))?;

	let token = SessionToken::generate();

	db.collection::<Session>("sessions")
		.insert_one(
			Session {
				id: token.hash(),
				user: user.id,
				last_used: DateTime::now(),
			},
			None,
		)
		.await
		.map_err(to_unexpected!("Creating session failed"))?;

	success(NewSession { token })
}

#[post("/logout")]
pub async fn logout(
	db: web::Data<Database>,
	authorization: Authorization,
) -> ApiResult<(), AuthenticationError> {
	let result = db
		.collection::<Session>("sessions")
		.delete_one(doc! {"_id": {"$eq": authorization.session_id}}, None)
		.await
		.map_err(to_unexpected!("Deleting one session failed"))?;

	if result.deleted_count == 1 {
		success(())
	} else {
		failure(AuthenticationError::Unauthenticated)
	}
}

#[post("/logout-all")]
pub async fn logout_all(db: web::Data<Database>, user: AuthenticatedUser) -> ApiResult<(), ()> {
	db.collection::<Session>("sessions")
		.delete_many(doc! {"user": {"$eq": user.id}}, None)
		.await
		.map_err(to_unexpected!("Deleting all sessions failed"))?;

	success(())
}

/// Registers a new user.
///
/// Requires a [`username`] and [`school_id`].
///
/// The [`year_of_study`] and [`faculty`] fields can be set to `null` (or not included) to indicate
/// the user desires them to be kept private.
#[post("/users/")]
pub async fn register(
	db: web::Data<Database>,
	_guest: Guest,
	new_user: web::Json<NewUser>,
) -> ApiResult<(), RegistrationError> {
	// Check to see if their [`school_id`] is valid.
	db.collection::<School>("schools")
		.find_one(doc! {"_id": {"$eq": new_user.school_id.clone()}}, None)
		.await
		.map_err(to_unexpected!("validating school's existence failed"))?
		.ok_or(Failure::BadRequest("invalid school id"))?;

	let users = db.collection::<Document>("users");

	let op = users.insert_one(
		doc! {
			"year_of_study": to_bson(&new_user.year_of_study).map_err(to_unexpected!("Converting year of study to bson failed"))?,
			"faculty": to_bson(&new_user.faculty).map_err(to_unexpected!("Converting faculty to bson failed"))?,
			"username": to_bson(&new_user.username).map_err(to_unexpected!("Converting username to bson failed"))?,
			"watched_school_ids": to_bson::<Vec<String>>(&vec![]).map_err(to_unexpected!("Converting empty vector to bson failed"))?,
			"school_id": &new_user.school_id,
			"primary_email": to_bson(&PrimaryEmail::NoEmail).map_err(to_unexpected!("Converting primary email to bson failed"))?,
		},
		None
	);


	match op.await {
		Ok(result) => {
			debug!("Inserted user: {:?}", result);

			// TODO: possible to apply `StatusCode::CREATED`?
			success(())
		}
		Err(err) => match err.kind.as_ref() {
			ErrorKind::Write(WriteFailure::WriteError(err)) if err.code == 11000 => {
				failure(RegistrationError::UsernameTaken)
			}
			_ => {
				error!("Inserting user failed: {}", err);
				Err(Failure::Unexpected)
			}
		},
	}
}
