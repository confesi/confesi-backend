use chrono;
use log::error;
use mongodb::bson::doc;
use regex::Regex;

use crate::{
	api_types::{success, ApiResult, Failure},
	auth::AuthenticatedUser,
	conf::HOST,
	masked_oid::{self, MaskedObjectId, MaskingKey},
	to_unexpected,
	types::{School, User},
};
use actix_web::{get, post, web};
use jsonwebtoken::{decode, encode, errors::Error, DecodingKey, EncodingKey, Header, Validation};
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
		r"^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,6})",
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
		exp: (chrono::Utc::now() + chrono::Duration::seconds(60)).timestamp() as usize,
	};

	match claims.create_jwt() {
		Ok(token) => {
			println!("Token: {}", token);
			let decoded = Claims::decode_jwt(&token).unwrap();
			println!(
				"Decoded: {:?}",
				masking_key.unmask(&decoded.masked_user_id).map_err(
					|masked_oid::PaddingError| Failure::BadRequest("bad masked sequential id"),
				)?,
			);
			success(format!("http://{}/verify/{}/", HOST, token)) // todo: send email here
		}
		Err(err) => {
			error!("Error creating JWT: {}", err);
			return Err(Failure::Unexpected);
		}
	}
}

// todo: return simple HTML disclosing results?
#[get("/verify/{token}/")]
pub async fn verify_link(
	db: web::Data<Database>,
	token: web::Path<String>,
	masking_key: web::Data<&'static MaskingKey>,
) -> ApiResult<(), ()> {
	// todo: when verifying, do one last check to ensure the email isn't already in use (potential race condition between 2 people verifying the same email). does it need to be atomic?
	let claims = Claims::decode_jwt(&token).map_err(|_| Failure::BadRequest("invalid token"))?;
	let user_id = masking_key
		.unmask(&claims.masked_user_id)
		.map_err(|masked_oid::PaddingError| Failure::BadRequest("bad masked sequential id"))?;

	// is the email already in use?
	let potential_user = db
		.collection::<User>("users")
		.find_one(
			doc! {
				"email": &claims.email
			},
			None,
		)
		.await
		.map_err(to_unexpected!("finding a user with this email failed"))?;

	if let Some(_) = potential_user {
		return Err(Failure::BadRequest("email already in use"));
	}

	// update user with verified and email_verified
	db.collection::<User>("users")
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
		.map_err(to_unexpected!("updating user's email failed"))?;

	success(())
}
