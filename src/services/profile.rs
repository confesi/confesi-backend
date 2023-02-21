use mongodb::{bson::{
	doc,
    to_bson, Document, Bson,
}, options::{FindOneAndUpdateOptions, ReturnDocument, UpdateOptions, FindOneAndReplaceOptions}};
use log::{
	error,
};
use serde::{Deserialize, Serialize};
use actix_web::{ put, web, get, post };
use mongodb::Database;
use crate::{auth::{
	AuthenticatedUser,
}, api_types::{ApiResult, Failure, success}, types::{User, PosterYearOfStudy, PosterFaculty, School}, to_unexpected, conf};

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
	let user = db.collection::<User>("users").find_one(doc! {"_id": {"$eq": user.id}}, None).await;
	// Finds and returns the user profile data.
	// Throws 400 if no account matches id, and 500 upon unknown find error.
	match user {
		Ok(possible_user) => match possible_user {
			Some(user) => return success(ProfileData {year_of_study: user.year_of_study, faculty: user.faculty, school_id: user.school_id, username: user.username.into()}),
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(_) => return Err(Failure::Unexpected)
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
	update_doc.insert("faculty", to_bson(&update_data.faculty).map_err(|_| Failure::Unexpected)?);

	// Update the [`year_of_study`] field.
	update_doc.insert("year_of_study", to_bson(&update_data.year_of_study).map_err(|_| Failure::Unexpected)?);

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
	let user = db.collection::<User>("users")
		.find_one_and_update(
		doc! {"_id": {"$eq": user.id}},
		doc! {"$set": update_doc},
		None,
	).await;
	match user {
		Ok(possible_user) => match possible_user {
			Some(_) => return success(()),
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(_) => return Err(Failure::Unexpected)
	};
}

#[put("/users/watched/")]
pub async fn update_watched(
	user: AuthenticatedUser,
	db: web::Data<Database>,
	new_school_ids: web::Json<Vec<String>>,
) -> ApiResult<(), ()> {
let schools = to_bson(&new_school_ids).map_err(|_| Failure::Unexpected)?;
let schools_length = to_bson(&new_school_ids.len()).map_err(|_| Failure::Unexpected)?;

// TODO: check if all passed school ids are valid (contained inside `schools` collection)

let pipeline = vec![
    doc! {
        "$addFields": {
            "watched_school_ids": {
                "$cond": {
                    "if": {
                        "$lte": [
                            { "$size": { "$ifNull": ["$watched_school_ids", []] } },
                            conf::MAX_WATCHED_UNIVERSITIES - (new_school_ids.len() as i32)
                        ],
                    },
                    "then": {
                        "$let": {
                            "vars": {
                                "union": { "$setUnion": [{ "$ifNull": ["$watched_school_ids", []] }, &schools] }
                            },
                            "in": {
                                "$cond": {
                                    "if": {
                                        "$eq": [
                                            { "$size": "$$union" },
                                            { "$add": [{ "$size": { "$ifNull": ["$watched_school_ids", []] } }, schools_length] }
                                        ]
                                    },
                                    "then": "$$union",
                                    "else": "$watched_school_ids"
                                }
                            }
                        }
                    },
                    "else": "$watched_school_ids"
                }
            }
        }
    },
];

let filter = doc! {"_id": user.id};

let possible_update_result = db.collection::<User>("users").update_one(filter, pipeline, None).await;
	match possible_update_result {
		Ok(update_result) => if update_result.modified_count == 1 {
			return success(())
		} else {
			return Err(Failure::BadRequest("too many watched universities or duplicates"))
		}
		Err(_) => return Err(Failure::Unexpected),
	};
}

#[get("/users/watched/")]
pub async fn get_watched(
	user: AuthenticatedUser,
	db: web::Data<Database>,
) -> ApiResult<Box<Vec<String>>, ()>{
	let user = db.collection::<User>("users")
		.find_one(
		doc! {"_id": {"$eq": user.id}},
		None
	).await;
	match user {
		Ok(possible_user) => match possible_user {
			Some(user) => match user.watched_school_ids {
				Some(watched_school_ids) => return success(Box::new(watched_school_ids)),
				None => return success(Box::new(vec![])),
			},
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(_) => return Err(Failure::Unexpected),
	};
}
