use chrono;
use log::error;
use mongodb::bson::{doc, to_bson};
use regex::Regex;

use crate::{
	api_types::{success, ApiResult, Failure},
	auth::AuthenticatedUser,
	conf::{EMAIL_VERIFICATION_LINK_EXPIRATION, HOST},
	masked_oid::{MaskedObjectId, MaskingKey},
	to_unexpected,
	types::{PrimaryEmail, School, User},
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
	email_type: EmailType,
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

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum EmailType {
	Personal,
	School,
}

#[derive(Deserialize)]
pub struct VerificationRequest {
	email: String,
	email_type: EmailType,
}

#[post("/verify")]
pub async fn send_verification_email(
	db: web::Data<Database>,
	masking_key: web::Data<&'static MaskingKey>,
	user: AuthenticatedUser,
	verification: web::Json<VerificationRequest>,
) -> ApiResult<String, ()> {
	// todo: gmail ignores dots? outlook doesn't? what about other providers? should I force remove dots from the email (or just ignore them)?

	// validate the email
	let email_matcher = Regex::new(
		r"(?i)^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,6})",
	)
	.unwrap();
	if (!email_matcher.is_match(&verification.email)) {
		return Err(Failure::BadRequest("incorrectly formatted email"));
	} else if verification.email.contains("+") {
		return Err(Failure::BadRequest("email can't be an alias"));
	}

	// if we're trying to verify a school email, make sure the domain is valid
	if matches!(verification.email_type, EmailType::School) {
		let domain = &verification
			.email
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
	}

	let matching_user = match verification.email_type {
		EmailType::Personal => doc! {
			"personal_email": &verification.email
		},
		EmailType::School => doc! {
			"school_email": &verification.email
		},
	};

	// is the email already in use?
	let potential_user = db
		.collection::<User>("users")
		.find_one(matching_user, None)
		.await
		.map_err(to_unexpected!("finding a user with this email failed"))?;

	if let Some(_) = potential_user {
		return Err(Failure::BadRequest("email already in use for this email type"));
	}

	// todo: check if the user already has a primary/school email, if so, we need to update it

	let claims = Claims {
		masked_user_id: masking_key.mask(&user.id),
		email: verification.email.to_string(),
		email_type: verification.email_type.clone(),
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
	let claims = match Claims::decode_jwt(&token) {
		Ok(claims) => claims,
		Err(err) => match err.kind() {
			ErrorKind::ExpiredSignature => {
				return gen_html("Verification link expired, please send another email 🥶")
			}
			_ => return gen_html("Malformed verification link 🤨"),
		},
	};
	let user_id = match masking_key.unmask(&claims.masked_user_id) {
		Ok(user_id) => user_id,
		Err(_) => return gen_html("Malformed verification link 🤨"),
	};

	// every time you verify a new email, it'll become your "primary" email by default
	let email_type = match to_bson(&claims.email_type) {
		Ok(bson) => bson,
		Err(_) => return gen_html("Error verifying email, please try again later 😳"),
	};

	let update_doc = match claims.email_type {
		EmailType::Personal => doc! {
				"$set": {
						"primary_email": email_type,
						"personal_email": &claims.email,
				}
		},
		EmailType::School => doc! {
				"$set": {
						"primary_email": email_type,
						"school_email": &claims.email,
				}
		},
	};

	// update user with newly verified email
	match db
		.collection::<User>("users")
		.update_one(
			doc! {
				"_id": user_id
			},
			update_doc,
			None,
		)
		.await
	{
		Ok(result) => {
			if result.modified_count == 1 {
				gen_html("Email verified ✅")
			} else {
				gen_html("Email already verified 😅")
			}
		}
		Err(_) => gen_html("Error verifying email, please try again later 😳")
	}
}

// todo: update docs for new routes and alterations to old routes
