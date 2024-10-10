use std::fmt;
use std::future::{
	self,
	Future,
};
use std::pin::Pin;
use std::time::SystemTime;

use actix_web::http::header;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::{
	FromRequest,
	HttpRequest,
	HttpResponse,
	ResponseError,
};
use log::{
	error,
	warn,
};
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{
	doc,
	to_bson,
};
use mongodb::options::{
	Acknowledgment,
	UpdateOptions,
	WriteConcern,
};
use mongodb::Database;
use serde::Serialize;

use crate::api_types::ApiError;
use crate::conf;
use crate::types::{
	Session,
	SessionToken,
	SessionTokenHash,
};

#[derive(Debug)]
pub struct Guest;

impl FromRequest for Guest {
	type Error = GuestRequired;
	type Future = future::Ready<Result<Self, Self::Error>>;

	fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
		future::ready(if req.headers().contains_key(header::AUTHORIZATION) {
			Err(GuestRequired)
		} else {
			Ok(Guest)
		})
	}
}

#[derive(Debug)]
pub struct GuestRequired;

impl fmt::Display for GuestRequired {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "can't perform this action as a registered user")
	}
}

impl ResponseError for GuestRequired {
	fn status_code(&self) -> StatusCode {
		StatusCode::BAD_REQUEST
	}
}

#[derive(Debug)]
/// The authorization presented with a request, hashed away safely. May or may not be valid.
pub struct Authorization {
	pub session_id: SessionTokenHash,
}

fn authorization_from_request(req: &HttpRequest) -> Result<Authorization, AuthenticationError> {
	req.headers()
		.get(header::AUTHORIZATION)
		.and_then(|h| h.to_str().ok())
		.and_then(|h| h.strip_prefix("Bearer "))
		.and_then(|t| t.parse::<SessionToken>().ok())
		.map(|token| Authorization {
			session_id: token.hash(),
		})
		.ok_or(AuthenticationError::Unauthenticated)
}

impl FromRequest for Authorization {
	type Error = AuthenticationError;
	type Future = future::Ready<Result<Self, Self::Error>>;

	fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
		future::ready(authorization_from_request(req))
	}
}

#[derive(Debug)]
pub struct AuthenticatedUser {
	pub id: ObjectId,
}

impl FromRequest for AuthenticatedUser {
	type Error = AuthenticationError;
	type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

	fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
		let db = req
			.app_data::<web::Data<Database>>()
			.expect("app_data should include a database client")
			.clone();
		let auth = authorization_from_request(req);

		Box::pin(async move {
			// TODO: request processing can take arbitrarily long, meaning operations might continue arbitrary long after an completed logout-all request. not ideal, but definitely not worth the overhead and restrictions of a request-wrapping transaction with MongoDB's causal consistency.
			let session_query = doc! {"_id": {"$eq": to_bson(&auth?.session_id).unwrap()}};
			let session = db
				.collection::<Session>("sessions")
				.find_one(session_query.clone(), None)
				.await
				.map_err(|err| {
					error!("Looking up session failed: {}", err);
					AuthenticationError::Unexpected
				})?
				.ok_or(AuthenticationError::Unauthenticated)?;

			match SystemTime::now().duration_since(session.last_used.to_system_time()) {
				Ok(t) if t >= conf::SESSION_MIN_TIME_BETWEEN_REFRESH => {
					// refresh the session
					match (db
						.collection::<Session>("sessions")
						.update_one(
							session_query.clone(),
							doc! {
								"$currentDate": {
									"last_used": true,
								},
							},
							// TODO: is there way to specify that this write should not be retried?
							UpdateOptions::builder()
								.write_concern(
									WriteConcern::builder().w(Acknowledgment::Nodes(1)).build(),
								)
								.build(),
						)
						.await)
					{
						Ok(update) => {
							if update.matched_count != 1 {
								// the session became invalid between the find and the refresh
								return Err(AuthenticationError::Unauthenticated);
							}
						}
						Err(err) => {
							error!("Refreshing session failed: {}", err);
							// continue without refreshing
						}
					}
				}
				Ok(_) => {
					// no need to refresh yet
				}
				Err(err) => {
					warn!("Session was last used {:?} in the future", err.duration());
				}
			}

			Ok(AuthenticatedUser { id: session.user })
		})
	}
}

#[derive(Debug, Serialize)]
pub enum AuthenticationError {
	Unauthenticated,
	Unexpected,
}

impl fmt::Display for AuthenticationError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Unauthenticated => write!(f, "can't perform this action without authentication"),
			Self::Unexpected => write!(f, "unexpected error"),
		}
	}
}

impl ResponseError for AuthenticationError {
	fn status_code(&self) -> StatusCode {
		match self {
			Self::Unauthenticated => StatusCode::UNAUTHORIZED,
			Self::Unexpected => StatusCode::INTERNAL_SERVER_ERROR,
		}
	}

	fn error_response(&self) -> HttpResponse {
		match self {
			Self::Unauthenticated => HttpResponse::Unauthorized()
				.insert_header(("WWW-Authenticate", r#"Bearer realm="user""#))
				.content_type("application/json")
				.body(r#"{"error":"Unauthenticated"}"#),
			Self::Unexpected => {
				HttpResponse::InternalServerError().body(r#"{"error":"Unexpected"}"#)
			}
		}
	}
}

impl ApiError for AuthenticationError {
	fn status_code(&self) -> StatusCode {
		ResponseError::status_code(self)
	}
}
