pub mod auth;
pub mod posts;
pub mod profile;

use actix_web::{
	HttpRequest,
	get,
};
use actix_web::web;
use futures::TryStreamExt;
use log::{
	error,
	info,
	warn,
};
use maxminddb::geoip2;
use mongodb::Database;
use mongodb::bson::{
	Document,
	doc,
};
use mongodb::bson::document::ValueAccessError;
use serde::Serialize;

use crate::{
	GeoIpReader,
	to_unexpected,
};
use crate::api_types::{
	ApiResult,
	Failure,
	success,
};
use crate::types::School;

#[derive(Serialize)]
pub struct SchoolListing {
	pub id: String,
	pub name: String,
	/// Distance in kilometres.
	pub distance: Option<f32>,
}

struct GeoPoint {
	longitude: f32,
	latitude: f32,
}

#[get("/schools/")]
pub async fn schools_list(
	req: HttpRequest,
	db: web::Data<Database>,
	geoip: web::Data<Option<GeoIpReader>>,
) -> ApiResult<Box<[SchoolListing]>, ()> {
	let mut location: Option<GeoPoint> = None;

	if let Some(geoip) = geoip.get_ref() {
		if let Some(peer_addr) = req.peer_addr() {
			match geoip.lookup::<geoip2::City>(peer_addr.ip()) {
				Ok(city) => {
					location = city.location.and_then(|location| {
						Some(GeoPoint {
							longitude: location.longitude? as f32,
							latitude: location.latitude? as f32,
						})
					});
				}
				Err(err) => {
					info!("Not using location for schools list: {}", err);
				}
			}
		} else {
			warn!("Not using location for schools list: no peer address");
		}
	}

	let operator = match location {
		Some(location) => doc! {
			"$geoNear": {
				"distanceField": "distance",
				"distanceMultiplier": 1e-3,
				"near": {
					"type": "Point",
					"coordinates": [location.longitude, location.latitude],
				},
				"spherical": true,
			},
		},
		None => doc! {
			"$project": {
				"name": true,
				"distance": {"$literal": null},
			}
		},
	};

	success(
		db.collection::<School>("schools")
			.aggregate([operator], None)
			.await
			.map_err(to_unexpected!("Getting list of schools cursor failed"))?
			.map_ok(|doc: Document| -> Result<SchoolListing, ValueAccessError> {
				Ok(SchoolListing {
					id: String::from(doc.get_str("_id")?),
					name: String::from(doc.get_str("name")?),
					distance:
						if doc.is_null("distance") {
							None
						} else {
							Some(doc.get_f64("distance")? as f32)
						},
				})
			})
			.try_collect::<Vec<Result<SchoolListing, ValueAccessError>>>()
			.await
			.map_err(to_unexpected!("Reading list of schools cursor failed"))?
			.into_iter()
			.collect::<Result<Box<[SchoolListing]>, _>>()
			.map_err(to_unexpected!("Deserializing school listing failed"))?
	)
}
