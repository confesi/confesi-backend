use mongodb::{bson::{
	doc,
    to_bson, Document, Bson,
}, options::{FindOptions, Hint}};
use log::{
	error,
};
use futures::{StreamExt, TryStreamExt};


use serde::{Deserialize, Serialize};
use actix_web::{ put, web, get, delete, post };
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

/// Deletes a list of universities from a user's watched list.
///
/// If nothing is deleted (you passed an invalid university, or one you want to delete isn't there) a 400 is returned.
#[delete("/users/watched/")]
pub async fn delete_watched(
	user: AuthenticatedUser,
	db: web::Data<Database>,
	delete_school_ids: web::Json<Vec<String>>,
) -> ApiResult<(), ()> {
	let to_be_deleted_schools = to_bson(&delete_school_ids).map_err(|_| Failure::Unexpected)?;
	let filter = doc! {"_id": {"$eq": user.id}};
  let update = doc! { "$pull": { "watched_school_ids": { "$in": &to_be_deleted_schools } } };
	let possible_update_result = db.collection::<User>("users").update_one(filter, update, None).await;
	match possible_update_result {
		Ok(update_result) => if update_result.modified_count == 1 {
			return success(())
		} else {
			return Err(Failure::BadRequest("nothing deleted"))
		}
		Err(_) => return Err(Failure::Unexpected),
	};
}

/// Adds a list of universities to a user's watched list.
///
/// If a university already exists in the list, or if adding anything from the list would put it over the
/// max watched university's size limit, a 400 is returned.
#[post("/users/watched/")]
pub async fn add_watched(
	user: AuthenticatedUser,
	db: web::Data<Database>,
	mut new_school_ids: web::Json<Vec<String>>,
) -> ApiResult<(), ()> {
	new_school_ids.dedup();
	let schools_bson = to_bson(&new_school_ids).map_err(|_| Failure::Unexpected)?;
	let schools_length_bson = to_bson(&new_school_ids.len()).map_err(|_| Failure::Unexpected)?;
	let schools_length_i32;
	if new_school_ids.len() > i32::MAX as usize {
		return Err(Failure::Unexpected);
	} else {
		schools_length_i32 = (new_school_ids.len() as i32);
	}

	// Check if all passed school ids are valid

	let filter = doc! { "_id": { "$in": &schools_bson } };

	let possible_found_schools = db.collection::<School>("schools")
			.find(filter, None)
			.await;

	match possible_found_schools {
		Ok(cursor) => {
			let items_found = cursor.count().await;
			if items_found > usize::MAX {return Err(Failure::Unexpected)};
			if items_found != new_school_ids.len() {return Err(Failure::BadRequest("not all items provided are valid schools"))};
		}
		Err(_) => return Err(Failure::Unexpected),
	};

	let pipeline = vec![
			doc! {
					"$addFields": {
							"watched_school_ids": {
									"$cond": {
											"if": {
													"$lte": [
															{ "$size": { "$ifNull": ["$watched_school_ids", []] } },
															conf::MAX_WATCHED_UNIVERSITIES - schools_length_i32
													],
											},
											"then": {
													"$let": {
															"vars": {
																	"union": { "$setUnion": [{ "$ifNull": ["$watched_school_ids", []] }, &schools_bson] }
															},
															"in": {
																	"$cond": {
																			"if": {
																					"$eq": [
																							{ "$size": "$$union" },
																							{ "$add": [{ "$size": { "$ifNull": ["$watched_school_ids", []] } }, schools_length_bson] }
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

#[derive(Serialize)]
pub struct SchoolDetail {
		pub school_id: String,
    pub full_name: String,
}

/// Searches for schools by query.
///
/// This matches to either the name (ex: "University of Victoria") or abbreviation (ex: "UVIC") of a school. It does so
/// by initially having the name and abbreviation both stored in the `name` field of the `School` document.
/// This is because you can only have 1 text index per collection. It then separates them out before
/// sending them back to the frontend.
#[get("/schools/{search_query}/")]
pub async fn school_by_query(
    db: web::Data<Database>,
    search_query: web::Path<String>,
) -> ApiResult<Vec<SchoolDetail>, ()> {
    let query = doc! {
        "name": {
            "$regex": format!(".*{}.*", search_query),
            "$options": "iu"
        }
    };

    let options = FindOptions::builder()
        .limit(i64::from(conf::MAX_SCHOOL_RESULTS_PER_QUERY))
        .build();

    let possible_cursor = db.collection::<School>("schools").find(query, options).await;
    match possible_cursor {
        Ok(cursor) => {
					let schools = cursor.try_collect::<Vec<School>>().await.map_err(|_| Failure::Unexpected)?;
					let mut school_details = Vec::new();
					for school in schools {
							let full_name = match extract_name(&school.name) {
									Some(full_name) => full_name,
									None => return Err(Failure::Unexpected),
							};

							let school_detail = SchoolDetail {
									full_name: full_name.to_string(),
									school_id: school.id,
							};
							school_details.push(school_detail);
					}
					return success(school_details);
        }
        Err(_) => return Err(Failure::Unexpected),
    }
}

/// Extracts the name from a `String` that has additional details in brackets.
///
/// Example: "University of Victoria (UVIC)" -> "University of Victoria".
///
/// Example: "University of British Columbia (UBC)" -> "University of British Columbia".
fn extract_name(input: &str) -> Option<&str> {
	if let Some(idx) = input.rfind('(') {
			return Some(&input[..idx].trim());
	}
	None
}

/// Gets the current list of universities the user is watching.
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
