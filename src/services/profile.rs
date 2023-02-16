use mongodb::{bson::{
	doc,
    to_bson, Document,
}, options::{FindOneAndUpdateOptions, ReturnDocument}};
use log::{
	error,
};
use serde::{Deserialize, Serialize};
use actix_web::{ put, web, get };
use mongodb::Database;
use crate::{auth::{
	AuthenticatedUser,
}, api_types::{ApiResult, Failure, success}, types::{User, PosterYearOfStudy, PosterFaculty, School}, to_unexpected};

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
	pub school_id: Option<String>,
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
			Some(user) => return success(ProfileData {year_of_study: user.year_of_study, faculty: user.faculty, school_id: Some(user.school_id), username: user.username.into()}),
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(_) => return Err(Failure::Unexpected)
	};
}

/// Updates user profile information.
/// 
/// If a field is null or undefined it is set as [`<ENUM_NAME>::Hidden`] to indicate
/// the user desires it to be kept private.
/// 
/// Not a PATCH request because couldn't do this https://stackoverflow.com/questions/44331037/how-can-i-distinguish-between-a-deserialized-field-that-is-missing-and-one-that
/// because Actix-web's [`web::Json`] inferes both undefined and null as None, making PATCH requests difficult.
#[put("/users/profile/")]
pub async fn update_profile(
	db: web::Data<Database>,
	update_data: web::Json<UpdatableProfileData>,
	user: AuthenticatedUser,
) -> ApiResult<ProfileData, ()> {

	// Document to be updated.
	let mut update_doc = Document::new();
	
	// Update the [`faculty`] field.
	let faculty_bson;
	if let Some(faculty) = &update_data.faculty {
		faculty_bson = &*faculty;
	} else {
		faculty_bson = &PosterFaculty::Hidden;
	}
	update_doc.insert("faculty", to_bson(&faculty_bson).map_err(|_| Failure::Unexpected)?);

	// Update the [`year_of_study`] field.
	let year_of_study_bson;
	if let Some(year_of_study) = &update_data.year_of_study {
		year_of_study_bson = &*year_of_study;
	} else {
		year_of_study_bson = &PosterYearOfStudy::Hidden;
	}
	update_doc.insert("year_of_study", to_bson(&year_of_study_bson).map_err(|_| Failure::Unexpected)?);

	if let Some(school_id) = &update_data.school_id {
		// Check to see if their new proposed [`school_id`] is valid, before adding
		// it to the [`update_doc`].
		db.collection::<School>("schools")
			.find_one(doc! {"_id": {"$eq": school_id}}, None)
			.await
			.map_err(to_unexpected!("validating school's existence failed"))?
			.ok_or(Failure::BadRequest("invalid school id"))?;
		update_doc.insert("school_id", school_id);
	}

	// Return the document AFTER it has been updated, to reflect the changes.
	let options = FindOneAndUpdateOptions::builder().return_document(ReturnDocument::After).build();

	// Finds, updates, and returns the user profile data.
	// Throws 400 if no account matches id, and 500 upon unknown update error.
	let user = db.collection::<User>("users")
		.find_one_and_update(
		doc! {"_id": {"$eq": user.id}},
		doc! {"$set": update_doc},
		Some(options)
	).await;
	match user {
		Ok(possible_user) => match possible_user {
			Some(user) => return success(ProfileData {year_of_study: user.year_of_study.into(), faculty: user.faculty.into(), school_id: Some(user.school_id), username: user.username.into()}),
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(_) => return Err(Failure::Unexpected)
	};
}