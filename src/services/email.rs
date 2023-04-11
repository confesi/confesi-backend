use chrono;
use log::error;
use mongodb::bson::{doc, to_bson};
use mongodb::options::{FindOneAndUpdateOptions, UpdateOptions};
use mongodb::Client as MongoClient;
use regex::Regex;

use crate::{
	api_types::{success, ApiResult, Failure},
	auth::AuthenticatedUser,
	conf::{EMAIL_VERIFICATION_LINK_EXPIRATION, HOST},
	masked_oid::{MaskedObjectId, MaskingKey},
	to_unexpected,
	types::{School, User},
};
use actix_web::{get, post, put, web, HttpResponse};
use jsonwebtoken::{
	decode, encode,
	errors::{Error, ErrorKind},
	DecodingKey, EncodingKey, Header, Validation,
};
use mongodb::Database;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct VerificationClaims {
	masked_user_id: MaskedObjectId,
	email: String,
	email_type: EmailType,
	exp: usize,
}

#[derive(Serialize, Deserialize)]
struct DeletionClaims {
	masked_user_id: MaskedObjectId,
	email_type: EmailType,
	exp: usize,
}

// todo: use .env for JWT secrets?
// todo: update docs for new routes and alterations to old routes
// todo: will the jwts work multiple times if you use them fast enough? how do I "expire" them after one use?

fn decode_jwt<T: DeserializeOwned>(token: &str, secret: &[u8]) -> Result<T, Error> {
	let key = DecodingKey::from_secret(secret);
	let decoded = decode::<T>(token, &key, &Validation::default())?;
	Ok(decoded.claims)
}

fn create_jwt<T: Serialize>(claims: &T, secret: &[u8]) -> Result<String, Error> {
	let key = EncodingKey::from_secret(secret);
	encode(&Header::default(), claims, &key).map_err(|err| err.into())
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

// todo: if the user has already verified the address they're putting into another field, skip email verification?
#[get("/verify_link")]
pub async fn send_verification_email_to_link_email(
	jwt_secret: web::Data<Vec<u8>>,
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

	// is the email already in use?
	if matches!(
		db.collection::<User>("users")
			.find_one(
				doc! {
						"$or": [
								{ "personal_email": &verification.email },
								{ "school_email": &verification.email }
						]
				},
				None
			)
			.await
			.map_err(to_unexpected!("finding a user with these emails failed"))?,
		Some(_)
	) {
		return Err(Failure::BadRequest("email already in use"));
	}

	let empty_field_name = match &verification.email_type {
		EmailType::Personal => "personal_email",
		EmailType::School => "school_email",
	};

	// does the user already have an email of this type?
	if matches!(
		db.collection::<User>("users")
			.find_one(
				doc! {
						"$and": [
								{"_id": user.id},
								{empty_field_name: {"$eq": null}}
						]
				},
				None
			)
			.await
			.map_err(to_unexpected!("finding a user with THIS TODO"))?,
		None
	) {
		return Err(Failure::BadRequest(
			"either your account doesn't exist, or you already have an email of this type",
		));
	}

	// todo: check if the user already has a primary/school email, if so, we need to update it

	let claims = VerificationClaims {
		masked_user_id: masking_key.mask(&user.id),
		email: verification.email.to_string(),
		email_type: verification.email_type.clone(),
		exp: (chrono::Utc::now() + chrono::Duration::seconds(EMAIL_VERIFICATION_LINK_EXPIRATION))
			.timestamp() as usize,
	};

	match create_jwt(&claims, jwt_secret.as_ref()) {
		Ok(token) => {
			success(format!("http://{}/verify_creation/{}/", HOST, token)) // todo: send email here
		}
		Err(err) => {
			error!("Error creating JWT: {}", err);
			return Err(Failure::Unexpected);
		}
	}
}

fn gen_html(content: &str) -> HttpResponse {
	let html = format!(
		"<!DOCTYPE html>
		<html>
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

// todo: should this be a POST request? It's creating a resource, but it needs to be called directly when
// todo: the link is opened in the browswer, which initiates a GET request.
#[get("/verify_creation/{token}/")]
pub async fn verify_email(
	jwt_secret: web::Data<Vec<u8>>,
	db: web::Data<Database>,
	token: web::Path<String>,
	masking_key: web::Data<&'static MaskingKey>,
) -> HttpResponse {
	let claims = match decode_jwt::<VerificationClaims>(&token, jwt_secret.as_ref()) {
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

	let empty_field_name = match claims.email_type {
		EmailType::Personal => "personal_email",
		EmailType::School => "school_email",
	};

	// update user with newly verified email
	match db
		.collection::<User>("users")
		.update_one(
			doc! {
					"$and": [
							{"_id": user_id},
							{empty_field_name: {"$eq": null}}
					]
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
		Err(_) => gen_html("Error verifying email, please try again later 😳"),
	}
}

// todo: add what type of primary email a user has to their account and what it is so they can fetch it

#[put("/email")]
pub async fn change_primary_email(
	db: web::Data<Database>,
	user: AuthenticatedUser,
	email_type: web::Json<EmailType>,
) -> ApiResult<(), ()> {
	let (not_null_field, update_name) = match &email_type.into_inner() {
		EmailType::Personal => ("personal_email", "personal"),
		EmailType::School => ("school_email", "school"),
	};

	match db
		.collection::<User>("users")
		.update_one(
			doc! {"_id": user.id, not_null_field: { "$ne": null }}, // query
			doc! {"$set": {"primary_email": update_name}},          // update
			None,
		)
		.await
	{
		Ok(result) => {
			if result.matched_count == 0 {
				Err(Failure::BadRequest(
					"this email type doesn't exist for this user",
				))
			} else if result.modified_count == 1 {
				success(())
			} else {
				println!("already changed");
				success(())
			}
		}
		Err(_) => Err(Failure::Unexpected),
	}
}

/// Sends a verification email to the address that is to be deleted
#[get("/verify_unlink")]
pub async fn send_verification_email_to_unlink_email(
	jwt_secret: web::Data<Vec<u8>>,
	user: AuthenticatedUser,
	email_type: web::Json<EmailType>,
	masking_key: web::Data<&'static MaskingKey>,
) -> ApiResult<String, ()> {
	let claims = DeletionClaims {
		masked_user_id: masking_key.mask(&user.id),
		email_type: email_type.into_inner(),
		exp: (chrono::Utc::now() + chrono::Duration::seconds(EMAIL_VERIFICATION_LINK_EXPIRATION))
			.timestamp() as usize,
	};
	match create_jwt(&claims, jwt_secret.as_ref()) {
		Ok(token) => {
			success(format!("http://{}/verify_deletion/{}/", HOST, token)) // todo: send email here
		}
		Err(err) => {
			error!("Error creating JWT: {}", err);
			return Err(Failure::Unexpected);
		}
	}
}

// todo: should this be a POST request? It's creating a resource, but it needs to be called directly when
// todo: the link is opened in the browswer, which initiates a GET request.
#[get("/verify_deletion/{token}/")]
pub async fn verify_deleting_email(
	db: web::Data<Database>,
	jwt_secret: web::Data<Vec<u8>>,
	mongo_client: web::Data<MongoClient>,
	token: web::Path<String>,
	masking_key: web::Data<&'static MaskingKey>,
) -> HttpResponse {
	let claims = match decode_jwt::<DeletionClaims>(&token, jwt_secret.as_ref()) {
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

	let (email_type, opposite_type_label) = match claims.email_type {
		EmailType::Personal => ("personal_email", "school"),
		EmailType::School => ("school_email", "personal"),
	};

	let update_options = FindOneAndUpdateOptions::builder()
		.return_document(mongodb::options::ReturnDocument::After)
		.build();

	// start session
	let mut session = match mongo_client.start_session(None).await {
		Ok(session) => session,
		Err(_) => return gen_html("Error verifying email, please try again later 😳"),
	};

	// start transaction
	match session.start_transaction(None).await {
		Ok(_) => {}
		Err(_) => return gen_html("Error verifying email, please try again later 😳"),
	}

	let updated_doc = db
		.collection::<User>("users")
		.find_one_and_update_with_session(
			doc! { "_id": user_id },
			doc! { "$set": { email_type: null , "primary_email": opposite_type_label} },
			Some(update_options),
			&mut session,
		)
		.await;

	match updated_doc {
		Ok(doc) => {
			if let Some(user) = doc {
				if user.personal_email.is_none() && user.school_email.is_none() {
					let second_update = db
						.collection::<User>("users")
						.find_one_and_update_with_session(
							doc! { "_id": user_id },
							doc! { "$set": { "primary_email": "no-email" } },
							None,
							&mut session,
						)
						.await;
					match second_update {
						Ok(_) => {}
						Err(_) => {
							return gen_html("Error deleting email, please try again later 😳")
						}
					}
				}
			} else {
				return gen_html("Error deleting email, please try again later 😳");
			}
		}
		Err(_) => return gen_html("Error deleting email, please try again later 😳"),
	}
	match session.commit_transaction().await {
		Ok(_) => return gen_html("Email deleted successfully ✅"),
		Err(_) => {
			return gen_html("Error deleting email, please try again later 😳");
		}
	}
}
