use chrono;
use log::error;
use mongodb::bson::doc;
use regex::Regex;

use crate::{
	api_types::{success, ApiResult, Failure},
	auth::AuthenticatedUser,
	conf::{EMAIL_VERIFICATION_LINK_EXPIRATION, HOST},
	masked_oid::{MaskedObjectId, MaskingKey},
	to_unexpected,
	types::{School, User},
};
use actix_web::{get, post, web, HttpResponse};
use jsonwebtoken::{
	decode, encode,
	errors::{Error, ErrorKind},
	DecodingKey, EncodingKey, Header, Validation,
};
use mongodb::Database;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Claims {
	masked_user_id: MaskedObjectId,
	email: String,
	exp: usize,
}

// todo: use .env for JWT secrets?

trait JWT {
	fn create_jwt(&self) -> Result<String, Error>;
	fn decode_jwt(token: &str) -> Result<Self, Error>
	where
		Self: Sized;
}

impl JWT for Claims {
	fn create_jwt(&self) -> Result<String, Error> {
		let key = EncodingKey::from_secret("secret".as_ref());
		encode(&Header::default(), self, &key).map_err(|err| err.into())
	}

	fn decode_jwt(token: &str) -> Result<Self, Error> {
		let key = DecodingKey::from_secret("secret".as_ref());
		let decoded = decode::<Self>(token, &key, &Validation::default())?;
		Ok(decoded.claims)
	}
}

#[post("/verify")]
pub async fn send_verification_email(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	email: web::Json<String>,
) -> ApiResult<String, ()> {
	// validate the email
	let email_matcher = Regex::new(
		r"(?i)^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,6})",
	)
	.unwrap();
	if (!email_matcher.is_match(&email)) {
		return Err(Failure::BadRequest("incorrectly formatted email"));
	} else if email.contains("+") {
		return Err(Failure::BadRequest("email can't be an alias"));
	}

	let domain = email
		.split('@')
		.last()
		.ok_or(Failure::BadRequest("incorrectly formatted email"))?;

	// is the domain a valid school domain?
	db.collection::<School>("schools")
		.find_one(
			doc! {
				"email_domains": {
					"$in": [domain]
				}
			},
			None,
		)
		.await
		.map_err(to_unexpected!("validating school's domain failed"))?
		.ok_or(Failure::BadRequest("invalid school domain"))?;

	// is the email already in use?
	let potential_user = db
		.collection::<User>("users")
		.find_one(
			doc! {
				"email": email.to_string()
			},
			None,
		)
		.await
		.map_err(to_unexpected!("finding a user with this email failed"))?;

	if let Some(_) = potential_user {
		return Err(Failure::BadRequest("email already in use"));
	}

	let claims = Claims {
		masked_user_id: masking_key.mask(&user.id),
		email: email.to_string(),
		exp: (chrono::Utc::now() + chrono::Duration::seconds(EMAIL_VERIFICATION_LINK_EXPIRATION))
			.timestamp() as usize,
	};

	match claims.create_jwt() {
		Ok(token) => {
			println!("Token: {}", token);
			success(format!("http://{}/verify/{}/", HOST, token)) // todo: send email here
		}
		Err(err) => {
			error!("Error creating JWT: {}", err);
			return Err(Failure::Unexpected);
		}
	}
}

fn gen_html(content: &str) -> HttpResponse {
	let html = format!(
		"<html>
					<head>
							<title>Email verification</title>
					</head>
					<body>
							<h1 style='text-align: center;'>{}</h1>
					</body>
			</html>",
		content
	);
	HttpResponse::Ok()
		.content_type("text/html; charset=utf-8")
		.body(html)
}

#[get("/verify/{token}/")]
pub async fn verify_link(
	db: web::Data<Database>,
	token: web::Path<String>,
	masking_key: web::Data<&'static MaskingKey>,
) -> HttpResponse {
	// todo: will there be a race condition that could cause multiple users to be verified with the same email?
	let claims = match Claims::decode_jwt(&token) {
		Ok(claims) => claims,
		Err(err) => match err.kind() {
			ErrorKind::ExpiredSignature => return gen_html("Verification link expired 🥶"),
			_ => return gen_html("Malformed verification link 🤨"),
		},
	};
	let user_id = match masking_key.unmask(&claims.masked_user_id) {
		Ok(user_id) => user_id,
		Err(_) => return gen_html("Malformed verification link 🤨"),
	};

	// is the email already in use?
	let potential_user = match db
		.collection::<User>("users")
		.find_one(
			doc! {
				"email": &claims.email
			},
			None,
		)
		.await
	{
		Ok(potential_user) => potential_user,
		Err(err) => {
			error!("Error finding a user with this email: {}", err);
			return gen_html("Internal server error validating email 🥲");
		}
	};

	if let Some(_) = potential_user {
		return gen_html("Email already verified 😅");
	}

	// update user with verified and email_verified
	match db
		.collection::<User>("users")
		.update_one(
			doc! {
				"_id": user_id
			},
			doc! {
				"$set": {
					"email": claims.email,
					"email_verified": true,
				}
			},
			None,
		)
		.await
	{
		Ok(_) => gen_html("Email verified successfully ✅"),
		Err(err) => {
			error!("Error updating user's email: {}", err);
			return gen_html("Internal server error validating email 🥲");
		}
	}
}

// todo: update docs for new routes and alterations to old routes
