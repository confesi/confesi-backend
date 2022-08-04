#![allow(unused_parens)]

mod api_types;
mod auth;
mod base64_serde;
mod conf;
mod masked_oid;
mod middleware;
mod services;
mod types;

use std::env;
use std::error::Error;
use actix_cors::Cors;
use actix_web::{
	App,
	HttpServer,
};
use actix_web::http::header;
use actix_web::middleware::Logger;
use actix_web::web;
use futures::{
	try_join,
};
use log::{
	info,
};
use mongodb::{
	Client as MongoClient,
	Database,
	IndexModel,
};
use mongodb::bson::{
	doc,
};
use mongodb::options::{
	Collation,
	CollationStrength,
	IndexOptions,
	ReadConcernLevel,
};

use crate::masked_oid::MaskingKey;
use crate::middleware::HostCheckWrap;
use crate::types::{
	Post,
	Session,
	User,
	Vote,
};

async fn initialize_database(db: &Database) -> mongodb::error::Result<()> {
	let users = db.collection::<User>("users");
	let sessions = db.collection::<Session>("sessions");
	let posts = db.collection::<Post>("posts");
	let votes = db.collection::<Vote>("votes");

	try_join!(
		users.create_index(
			IndexModel::builder()
				.keys(doc! {"username": 1})
				.options(
					IndexOptions::builder()
						.unique(true)
						.collation(
							Collation::builder()
								.locale("en")
								.strength(CollationStrength::Primary)
								.build()
						)
						.build()
				)
				.build(),
			None,
		),

		sessions.create_index(
			IndexModel::builder()
				.keys(doc! {"user": 1})
				.build(),
			None,
		),
		sessions.create_index(
			IndexModel::builder()
				.keys(doc! {"last_used": 1})
				.options(
					IndexOptions::builder()
						.expire_after(conf::UNUSED_SESSION_TTL)
						.build()
				)
				.build(),
			None,
		),

		posts.create_index(
			IndexModel::builder()
				.keys(doc! {"sequential_id": -1})
				.build(),
			None,
		),

		votes.create_index(
			IndexModel::builder()
				.keys(doc! {"post": 1, "user": 1})
				.options(
					IndexOptions::builder()
						.unique(true)
						.build()
				)
				.build(),
			None,
		),
	)?;

	Ok(())
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
	env_logger::init_from_env(env_logger::Env::default());

	let masking_key: &'static MaskingKey = {
		let mut key_bytes = [0; 16];
		// TODO: read this from `/run/secrets` instead
		hex::decode_to_slice(env::var("OID_SECRET")?, &mut key_bytes)?;
		Box::leak(Box::new(MaskingKey::new(&key_bytes)))
	};

	info!("Initializing database");

	let db =
		MongoClient::with_uri_str(env::var("DB_CONNECT")?).await?
		.default_database()
		.expect("no default database");

	assert_eq!(db.read_concern().map(|c| &c.level), Some(&ReadConcernLevel::Majority));

	initialize_database(&db).await?;

	info!("Database initialized");

	HttpServer::new(move || {
		let cors =
			Cors::default()
				.allowed_origin_fn(|origin, _req_head| {
					origin.to_str()
						.map(|origin| conf::PERMITTED_ORIGINS.contains(&origin))
						.unwrap_or(false)
				})
				.allow_any_method()
				.allowed_header(header::AUTHORIZATION)
				.allowed_header(header::CONTENT_TYPE);

		App::new()
			.wrap(cors)
			.wrap(HostCheckWrap(conf::HOST))
			.wrap(Logger::default())
			.app_data(web::Data::new(db.clone()))
			.app_data(web::Data::new(masking_key))
			.service(services::auth::login)
			.service(services::auth::logout)
			.service(services::auth::logout_all)
			.service(services::auth::register)
			.service(services::posts::create)
			.service(services::posts::list)
	})
		.bind(("0.0.0.0", 3000))?
		.run()
		.await?;

	Ok(())
}
