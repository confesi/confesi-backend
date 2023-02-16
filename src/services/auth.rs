use actix_web::{
	post,
};
use actix_web::web;
use actix_web::http::StatusCode;
use log::{
	debug,
	error,
};
use mongodb::Database;
use mongodb::bson::{
	DateTime,
	doc,
};
use mongodb::error::{
	ErrorKind,
	WriteFailure,
};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::{
	to_unexpected,
};
use crate::auth::{
	Authorization,
	AuthenticationError,
	AuthenticatedUser,
	Guest,
};
use crate::api_types::{
	ApiError,
	ApiResult,
	Failure,
	failure,
	success,
};
use crate::types::{
	Session,
	SessionToken,
	User,
	Username, PosterYearOfStudy, PosterFaculty,
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
	// Fcaulty of the poster.
	pub faculty: Option<PosterFaculty>,
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
	let user =
		db.collection::<User>("users")
			.find_one(doc! {"username": {"$eq": credentials.username.as_ref()}}, None)
			.await
			.map_err(to_unexpected!("Finding user failed"))?
			.ok_or(Failure::Expected(LoginError::UsernameNotFound))?;

	let token = SessionToken::generate();

	db.collection::<Session>("sessions")
		.insert_one(Session {
			id: token.hash(),
			user: user.id,
			last_used: DateTime::now(),
		}, None)
		.await
		.map_err(to_unexpected!("Creating session failed"))?;

	success(NewSession {
		token,
	})
}

#[post("/logout")]
pub async fn logout(db: web::Data<Database>, authorization: Authorization) -> ApiResult<(), AuthenticationError> {
	let result =
		db.collection::<Session>("sessions")
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

#[post("/users/")]
pub async fn register(db: web::Data<Database>, _guest: Guest, new_user: web::Json<NewUser>) -> ApiResult<(), RegistrationError> {
	let users = db.collection::<NewUser>("users");

	let op = users.insert_one(&*new_user, None);

	match op.await {
		Ok(result) => {
			debug!("Inserted user: {:?}", result);

			// TODO: possible to apply `StatusCode::CREATED`?
			success(())
		}
		Err(err) => {
			match err.kind.as_ref() {
				ErrorKind::Write(WriteFailure::WriteError(err)) if err.code == 11000 =>
					failure(RegistrationError::UsernameTaken),
				_ => {
					error!("Inserting user failed: {}", err);
					Err(Failure::Unexpected)
				}
			}
		}
	}
}
