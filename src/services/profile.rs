use actix_web::{
	delete,
	get,
	post,
	put,
	web,
};
use futures::StreamExt;
use log::error;
use mongodb::bson::{
	doc,
	to_bson,
	Document,
};
use mongodb::Database;
use serde::{
	Deserialize,
	Serialize,
};

use crate::{
	api_types::{
		success,
		ApiResult,
		Failure,
	},
	auth::AuthenticatedUser,
	to_unexpected,
	types::{
		PosterFaculty,
		PosterYearOfStudy,
		School,
		User,
	},
};

#[derive(Deserialize)]
pub struct UpdatableProfileData {
	// Year of study of the poster.
	pub year_of_study: Option<PosterYearOfStudy>,
	// Faculty of the poster.
	pub faculty: Option<PosterFaculty>,
	// School of the poster.
	pub school_id: Option<String>,
}

#[derive(Serialize)]
pub struct ProfileData {
	// Year of study of the poster.
	pub year_of_study: Option<PosterYearOfStudy>,
	// Faculty of the poster.
	pub faculty: Option<PosterFaculty>,
	// School of the poster.
	pub school_id: String,
	// Username of user
	pub username: String,
}

/// Fetches user profile information.
#[get("/users/profile/")]
pub async fn get_profile(
	db: web::Data<Database>,
	user: AuthenticatedUser,
) -> ApiResult<ProfileData, ()> {
	let user = db
		.collection::<User>("users")
		.find_one(doc! {"_id": {"$eq": user.id}}, None)
		.await;
	// Finds and returns the user profile data.
	// Throws 400 if no account matches id, and 500 upon unknown find error.
	match user {
		Ok(possible_user) => match possible_user {
			Some(user) => {
				return success(ProfileData {
					year_of_study: user.year_of_study,
					faculty: user.faculty,
					school_id: user.school_id,
					username: user.username.into(),
				})
			}
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(err) => {
			error!("Fetching user information failed: {}", err);
			return Err(Failure::Unexpected);
		}
	};
}

/// Updates user profile information.
///
/// The [`year_of_study`] or [`faculty`] fields can be set to `null` (or not included) to indicate
/// the user desires them to be kept private.
///
/// [`school_id`] is optional when updating, and will only be changed if a valid [`school_id`] is passed.
///
/// Not a PATCH request because couldn't do this: https://stackoverflow.com/questions/44331037/how-can-i-distinguish-between-a-deserialized-field-that-is-missing-and-one-that
/// because Actix-web's [`web::Json`] inferes both `undefined` and `null` as `None`, making PATCH requests difficult.
#[put("/users/profile/")]
pub async fn update_profile(
	db: web::Data<Database>,
	update_data: web::Json<UpdatableProfileData>,
	user: AuthenticatedUser,
) -> ApiResult<(), ()> {
	// Document to be updated.
	let mut update_doc = Document::new();

	// Update the [`faculty`] field.
	update_doc.insert(
		"faculty",
		to_bson(&update_data.faculty)
			.map_err(to_unexpected!("Converting faculty to bson failed"))?,
	);

	// Update the [`year_of_study`] field.
	update_doc.insert(
		"year_of_study",
		to_bson(&update_data.year_of_study)
			.map_err(to_unexpected!("Converting year of study to bson failed"))?,
	);

	if let Some(school_id) = &update_data.school_id {
		// Check to see if a user's new proposed [`school_id`] is valid (and exists), before adding
		// it to the [`update_doc`].
		db.collection::<School>("schools")
			.find_one(doc! {"_id": {"$eq": school_id}}, None)
			.await
			.map_err(to_unexpected!("validating school's existence failed"))?
			.ok_or(Failure::BadRequest("invalid school id"))?;
		update_doc.insert("school_id", school_id);
	}

	// Finds, updates, and returns a success-200 response.
	// Throws 400 if no account matches id, and 500 upon unknown update error.
	let user = db
		.collection::<User>("users")
		.find_one_and_update(
			doc! {"_id": {"$eq": user.id}},
			doc! {"$set": update_doc},
			None,
		)
		.await;
	match user {
		Ok(possible_user) => match possible_user {
			Some(_) => return success(()),
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(err) => {
			error!("Updating user information failed: {}", err);
			return Err(Failure::Unexpected);
		}
	};
}

/// Deletes a list of universities from a user's watched list.
#[delete("/users/watched/")]
pub async fn delete_watched(
	user: AuthenticatedUser,
	db: web::Data<Database>,
	delete_school_ids: web::Json<Vec<String>>,
) -> ApiResult<(), ()> {
	let to_be_deleted_schools =
		to_bson(&delete_school_ids).map_err(to_unexpected!("Converting schools to bson failed"))?;
	let filter = doc! {"_id": {"$eq": user.id}};
	let update = doc! { "$pull": { "watched_school_ids": { "$in": &to_be_deleted_schools } } };
	db.collection::<User>("users")
		.update_one(filter, update, None)
		.await
		.map_err(to_unexpected!("Deleting watched schools failed"))?;
	success(())
}

/// Adds a list of universities to a user's watched list.
///
/// If a university already exists in the list a 400 is returned.
#[post("/users/watched/")]
pub async fn add_watched(
	user: AuthenticatedUser,
	db: web::Data<Database>,
	mut new_school_ids: web::Json<Vec<String>>,
) -> ApiResult<(), ()> {
	new_school_ids.dedup();
	let schools_bson =
		to_bson(&new_school_ids).map_err(to_unexpected!("Converting schools to bson failed"))?;

	// Check if all passed school ids are valid

	let filter = doc! { "_id": { "$in": &schools_bson } };

	let possible_found_schools = db.collection::<School>("schools").find(filter, None).await;

	match possible_found_schools {
		Ok(cursor) => {
			let items_found = cursor.count().await;
			if items_found != new_school_ids.len() {
				return Err(Failure::BadRequest(
					"not all items provided are valid schools",
				));
			};
		}
		Err(err) => {
			error!("Finding schools failed: {}", err);
			return Err(Failure::Unexpected);
		}
	};

	let pipeline = vec![doc! {
		"$addFields": {
			"watched_school_ids": {
				"$setUnion": [ "$watched_school_ids", schools_bson ]
			}
		}
	}];

	let filter = doc! {"_id": user.id};

	let possible_update_result = db
		.collection::<User>("users")
		.update_one(filter, pipeline, None)
		.await;
	match possible_update_result {
		Ok(update_result) => {
			if update_result.modified_count == 1 {
				return success(());
			} else {
				return Err(Failure::BadRequest(
					"too many watched universities or duplicates",
				));
			}
		}
		Err(err) => {
			error!("Updating user's watched schools failed: {}", err);
			return Err(Failure::Unexpected);
		}
	};
}

#[derive(Serialize)]
pub struct SchoolDetail {
	pub school_id: String,
	pub full_name: String,
}

/// Gets the current list of universities the user is watching.
#[get("/users/watched/")]
pub async fn get_watched(
	user: AuthenticatedUser,
	db: web::Data<Database>,
) -> ApiResult<Box<Vec<String>>, ()> {
	let user = db
		.collection::<User>("users")
		.find_one(doc! {"_id": {"$eq": user.id}}, None)
		.await;
	match user {
		Ok(possible_user) => match possible_user {
			Some(user) => return success(Box::new(user.watched_school_ids)),
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(err) => {
			error!("Getting list of watched schools failed: {}", err);
			return Err(Failure::Unexpected);
		}
	};
}
