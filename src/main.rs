#![allow(unused_parens)]

mod api_types;
mod auth;
mod base64_serde;
mod conf;
mod masked_oid;
mod middleware;
mod services;
mod types;

use actix_cors::Cors;
use actix_web::http::header;
use actix_web::middleware::Logger;
use actix_web::web;
use actix_web::{App, HttpServer};
use futures::try_join;
use log::{info, warn};
use memmap::Mmap;
use mongodb::bson::doc;
use mongodb::options::{
	Collation, CollationStrength, IndexOptions, ReadConcernLevel, UpdateOptions,
};
use mongodb::{Client as MongoClient, Database, IndexModel};
use std::env;
use std::error::Error;
use std::fs::File;

use crate::masked_oid::MaskingKey;
use crate::middleware::HostCheckWrap;
use crate::types::{Post, School, Session, User, Vote};

pub type GeoIpReader = &'static maxminddb::Reader<Mmap>;

async fn initialize_database(db: &Database) -> mongodb::error::Result<()> {
	let schools = db.collection::<School>("schools");
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
		users.create_index(
			IndexModel::builder()
				.keys(doc! {"personal_email": 1})
				.options(
					IndexOptions::builder()
						.unique(true)
						.build()
				)
				.build(),
			None,
		),
		users.create_index(
			IndexModel::builder()
				.keys(doc! {"school_email": 1})
				.options(
					IndexOptions::builder()
						.unique(true)
						.build()
				)
				.build(),
			None,
		),
		sessions.create_index(IndexModel::builder().keys(doc! {"user": 1}).build(), None,),
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
		posts.create_index(
			IndexModel::builder()
				.keys(doc! {"absolute_score": -1})
				.build(),
			None,
		),
		posts.create_index(
			IndexModel::builder()
				.keys(doc! {"trending_score": -1})
				.build(),
			None,
		),
		async {
			schools
				.create_index(
					IndexModel::builder()
						.keys(doc! {"position": "2dsphere"})
						.build(),
					None,
				)
				.await?;

			schools
				.update_one(
					doc! {
						"_id": {"$eq": "UVIC"},
					},
					doc! {
						"$set": {
							"name": "University of Victoria",
							"position": {
								"type": "Point",
								"coordinates": [-123.3117, 48.4633],
							},
							"email_domains": vec!["uvic.ca"],
						},
					},
					UpdateOptions::builder().upsert(true).build(),
				)
				.await?;

			schools
				.update_one(
					doc! {
						"_id": {"$eq": "UBC"},
					},
					doc! {
						"$set": {
							"name": "University of British Columbia",
							"position": {
								"type": "Point",
								"coordinates": [-123.2460, 49.2606],
							},
							"email_domains": vec!["student.ubc.ca", "allumni.ubc.ca"],
						},
					},
					UpdateOptions::builder().upsert(true).build(),
				)
				.await?;

			Ok(())
		},
		schools.create_index(
			IndexModel::builder()
				.keys(doc! {"email_domains": 1})
				.build(),
			None,
		),
		votes.create_index(
			IndexModel::builder()
				.keys(doc! {"post": 1, "user": 1})
				.options(IndexOptions::builder().unique(true).build())
				.build(),
			None,
		),
	)?;

	Ok(())
}

fn open_geoip_database() -> Result<GeoIpReader, Box<dyn Error>> {
	let geoip_file = File::open("GeoLite2-City.mmdb")?;
	let geoip_mmap = unsafe { Mmap::map(&geoip_file) }?;
	let reader = maxminddb::Reader::from_source(geoip_mmap)?;
	Ok(Box::leak(Box::new(reader)))
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
	env_logger::init_from_env(env_logger::Env::default());

	let jwt_secret = env::var("JWT_SECRET")
		.expect("JWT_SECRET environment variable not set")
		.into_bytes();


	let masking_key: &'static MaskingKey = {
		let mut key_bytes = [0; 16];
		// TODO: read this from `/run/secrets` instead
		hex::decode_to_slice(env::var("OID_SECRET")?, &mut key_bytes)?;
		Box::leak(Box::new(MaskingKey::new(&key_bytes)))
	};

	info!("Initializing database");

	let mongo_client = MongoClient::with_uri_str(env::var("DB_CONNECT")?).await?;
	let db = mongo_client
		.default_database()
		.expect("no default database");

	assert_eq!(
		db.read_concern().map(|c| &c.level),
		Some(&ReadConcernLevel::Majority)
	);

	initialize_database(&db).await?;

	info!("Database initialized");

	let geoip_reader = match open_geoip_database() {
		Ok(geoip_reader) => Some(geoip_reader),
		Err(err) => {
			warn!("Failed to open GeoIP database: {}", err);
			None
		}
	};

	HttpServer::new(move || {
		let cors = Cors::default()
			.allowed_origin_fn(|origin, _req_head| {
				origin
					.to_str()
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
			.app_data(web::Data::new(mongo_client.clone()))
			.app_data(web::Data::new(db.clone()))
			.app_data(web::Data::new(geoip_reader))
			.app_data(web::Data::new(masking_key))
			.app_data(web::Data::new(jwt_secret.clone()))
			.service(services::schools_list)
			.service(services::auth::login)
			.service(services::auth::logout)
			.service(services::auth::logout_all)
			.service(services::auth::register)
			.service(services::posts::create)
			.service(services::posts::list)
			.service(services::posts::vote)
			.service(services::profile::update_profile)
			.service(services::profile::get_profile)
			.service(services::posts::get_single_post)
			.service(services::profile::get_watched)
			.service(services::profile::add_watched)
			.service(services::profile::delete_watched)
			.service(services::email::verify_email)
			.service(services::email::send_verification_email)
			.service(services::email::change_primary_email)
			.service(services::email::delete_email)
			.service(services::email::verify_deleting_email)
	})
	.bind(("0.0.0.0", 3000))?
	.run()
	.await?;

	Ok(())
}
