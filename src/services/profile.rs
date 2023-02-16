use mongodb::{bson::{
	doc,
    to_bson, Document,
}, options::{FindOneAndUpdateOptions, ReturnDocument}};
use serde::{Deserialize, Serialize};
use actix_web::{ patch, web };
use mongodb::Database;
use crate::{auth::{
	AuthenticatedUser,
}, api_types::{ApiResult, Failure, success}, types::{User, PosterYearOfStudy, PosterFaculty}};

#[derive(Deserialize, Serialize)]
pub struct UpdateProfileRequest {
	// Year of study of the poster.
	pub year_of_study: Option<PosterYearOfStudy>,
	// Fcaulty of the poster.
	pub faculty: Option<PosterFaculty>,
}

/// Updates user profile information.
#[patch("/users/profile/")]
pub async fn update_profile(
	db: web::Data<Database>,
	update_data: web::Json<UpdateProfileRequest>,
	user: AuthenticatedUser,
) -> ApiResult<UpdateProfileRequest, ()> {
	// Converts incoming enum variants to valid Bson.
	let faculty_bson = to_bson(&update_data.faculty).map_err(|_| Failure::Unexpected)?;
	let year_of_study_bson = to_bson(&update_data.year_of_study).map_err(|_| Failure::Unexpected)?;

	// Only adds non-null, valid fields to the [`update_doc`].
	let mut update_doc = Document::new();
	if let Some(_) = &update_data.faculty {
		update_doc.insert("faculty", faculty_bson);
	}
	if let Some(_) = &update_data.year_of_study {
		update_doc.insert("year_of_study", year_of_study_bson);
	}

	// Return the document *after* it has been updated, to reflect the changes.
	let options = FindOneAndUpdateOptions::builder().return_document(ReturnDocument::After).build();

	// Finds, updates, and returns the user profile data.
	// Throws 400 if no account matches id, and 500 upon unknown update error.
	let user = db.collection::<User>("users")
		.find_one_and_update(
		doc! {"_id": {"$eq": user.id}},
		doc! {
			"$set": update_doc
		},
		Some(options)
	).await;
	match user {
		Ok(possible_user) => match possible_user {
			Some(user) => return success(UpdateProfileRequest {year_of_study: user.year_of_study, faculty: user.faculty}),
			None => return Err(Failure::BadRequest("no account matches this id")),
		},
		Err(_) => return Err(Failure::Unexpected)
	};
}