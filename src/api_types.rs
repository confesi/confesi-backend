//! actix-web's built-in error wrappers respond with the error message, and we don't really want to worry about sensitive information leaking that way.

use std::fmt;

use actix_web::{
	HttpRequest,
	HttpResponse,
	Responder,
	ResponseError,
};
use actix_web::body::MessageBody;
use actix_web::http::StatusCode;
use actix_web::http::header::{self, HeaderValue};
use log::{
	error,
};
use serde::{Serialize};
use serde::ser::{
	SerializeStruct,
	Serializer,
};

pub trait ApiError: Serialize {
	fn status_code(&self) -> StatusCode;
}

// we're using this instead of [`std::convert::Infallible`] because `Infallible` doesn't implement `Serialize` :(
impl ApiError for () {
	fn status_code(&self) -> StatusCode {
		StatusCode::INTERNAL_SERVER_ERROR
	}
}

pub type ApiResult<T, E> = Result<Success<T>, Failure<E>>;

pub const fn success<T: Serialize, E: ApiError>(x: T) -> ApiResult<T, E> {
	Ok(Success(x))
}

pub const fn failure<T: Serialize, E: ApiError>(err: E) -> ApiResult<T, E> {
	Err(Failure::Expected(err))
}

pub struct Success<T: Serialize>(pub T);

impl<T: Serialize> Serialize for Success<T> {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let mut s = serializer.serialize_struct("ApiResult", 2)?;
		s.serialize_field("error", &(None as Option<()>))?;
		s.serialize_field("value", &self.0)?;
		s.end()
	}
}

impl<T: Serialize> Responder for Success<T> {
	type Body = String;

	fn respond_to(self, _: &HttpRequest) -> HttpResponse<Self::Body> {
		let (status, body) = match serde_json::to_string(&self) {
			Ok(body) => (StatusCode::OK, body),
			Err(err) => {
				error!("JSON serialization failure: {}", err);
				(StatusCode::INTERNAL_SERVER_ERROR, String::from(r#"{"error":"InternalError"}"#))
			}
		};

		let mut response = HttpResponse::with_body(status, body);
		response.headers_mut().insert(
			header::CONTENT_TYPE,
			HeaderValue::from_static("application/json"),
		);
		response
	}
}

#[derive(Debug)]
pub enum Failure<E: ApiError> {
	Expected(E),
	BadRequest(&'static str),
	Unexpected,
}

#[derive(Serialize)]
enum UnexpectedError {
	BadRequest,
	Unexpected,
}

// This is required by `ResponseError`, but should be unused. We don't want to require `E: Display`.
impl<E: ApiError> fmt::Display for Failure<E> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Expected(_) => write!(f, "expected error"),
			Self::BadRequest(detail) => write!(f, "bad request: {}", detail),
			Self::Unexpected => write!(f, "unexpected error"),
		}
	}
}

impl<E: ApiError> Serialize for Failure<E> {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		let mut s = serializer.serialize_struct("ApiResult", 3)?;
		match self {
			Self::Expected(error) => {
				s.serialize_field("error", error)?;
				s.skip_field("detail")?;
			}
			Self::BadRequest(detail) => {
				s.serialize_field("error", &UnexpectedError::BadRequest)?;
				s.serialize_field("detail", detail)?;
			}
			Self::Unexpected => {
				s.serialize_field("error", &UnexpectedError::Unexpected)?;
				s.skip_field("detail")?;
			}
		}
		s.skip_field("value")?;
		s.end()
	}
}

impl<E: ApiError + fmt::Debug> ResponseError for Failure<E> {
	fn status_code(&self) -> StatusCode {
		match self {
			Self::Expected(error) => error.status_code(),
			Self::BadRequest(_detail) => StatusCode::BAD_REQUEST,
			Self::Unexpected => StatusCode::INTERNAL_SERVER_ERROR,
		}
	}

	fn error_response(&self) -> HttpResponse {
		let (status, body) = match serde_json::to_string(&self) {
			Ok(body) => (self.status_code(), body),
			Err(err) => {
				error!("JSON serialization failure: {}", err);
				(StatusCode::INTERNAL_SERVER_ERROR, String::from(r#"{"error":"InternalError"}"#))
			}
		};

		HttpResponse::build(status)
			.insert_header((header::CONTENT_TYPE, HeaderValue::from_static("application/json")))
			.message_body(body.boxed())
			.expect("no response builder errors should be possible")
	}
}

#[macro_export]
macro_rules! to_unexpected {
	($message: literal) => {
		|err| {
			error!(concat!($message, ": {}"), err);
			Failure::Unexpected
		}
	}
}