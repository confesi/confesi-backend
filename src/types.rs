pub use self::token::{
	SessionToken,
	SessionTokenHash,
};
use serde::ser;
use std::convert::TryFrom;
use std::fmt;
use std::str::{self, FromStr};
use blake2::{
	Blake2b,
	Digest,
};
use blake2::digest::consts::U16;
use mongodb::bson::{
	Binary,
	Bson,
	DateTime, Document, doc,
};
use mongodb::bson::oid::ObjectId;
use mongodb::bson::spec::BinarySubtype;
use rand::RngCore;
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use serde::de::Error;

use crate::conf;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(into = "String", try_from = "String")]
pub struct Username(String);

impl AsRef<str> for Username {
	fn as_ref(&self) -> &str {
		self.0.as_ref()
	}
}

impl From<Username> for String {
	fn from(username: Username) -> Self {
		username.0
	}
}

impl TryFrom<String> for Username {
	type Error = UsernameInvalid;

	fn try_from(s: String) -> Result<Self, Self::Error> {
		if (1..=conf::USERNAME_MAX_LENGTH).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric()) {
			Ok(Self(s))
		} else {
			Err(UsernameInvalid)
		}
	}
}

#[derive(Clone, Copy)]
pub struct UsernameInvalid;

impl fmt::Display for UsernameInvalid {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "invalid username")
	}
}

#[derive(Deserialize)]
pub struct SavedContent {
	#[serde(rename = "_id")]
	pub id: ObjectId,
	pub user_id: ObjectId,
	pub content_type: SavedType,
	pub content_id: ObjectId,
	#[serde(with = "Rfc3339DateTime")]
	pub saved_at: Rfc3339DateTime,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub post: Option<Post>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub comment: Option<Post>, // TODO: make into `Comment` once commenting is implemented.
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedType {
	Comment,
	Post
}

#[derive(Deserialize)]
pub struct User {
	#[serde(rename = "_id")]
	pub id: ObjectId,
	pub username: Username,
	// Year of study of the poster.
	pub year_of_study: Option<PosterYearOfStudy>,
	// Faculty of the poster.
	pub faculty: Option<PosterFaculty>,
	// School of the user.
	pub school_id: String,
	// Watched universities of the user
	pub watched_school_ids: Vec<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Session {
	#[serde(rename = "_id")]
	pub id: SessionTokenHash,
	pub user: ObjectId,
	pub last_used: DateTime,
}

#[derive(Deserialize)]
pub struct Post {
	#[serde(rename = "_id")]
	pub id: ObjectId,
	pub sequential_id: i32,
	pub owner: ObjectId,
	pub text: String,
	pub votes_up: i32,
	pub votes_down: i32,
	pub absolute_score: i32,
	pub trending_score: f64,
}

 /// The various years of study the creator of a post can be.
 #[derive(Deserialize, Serialize, Clone, Debug)]
 #[serde(rename_all = "snake_case")]
 pub enum PosterYearOfStudy {
 	One,
 	Two,
 	Three,
 	Four,
 	Five,
 	Graduate,
 	PhD,
 	Alumni,
}

 /// The various faculties the creator of a post can be associated with.
 #[derive(Deserialize, Serialize, Clone, Debug)]
 #[serde(rename_all = "snake_case")]
 pub enum PosterFaculty {
 	Business,
 	Medicine,
 	SocialScience,
 	History,
 	Engineering,
 	ComputerScience,
 	Psychology,
 	Communication,
 	Arts,
 	Education,
 }

#[derive(Deserialize)]
pub struct School {
	#[serde(rename = "_id")]
	pub id: String,
	pub name: String,
}

#[derive(Deserialize, Serialize)]
pub struct Vote {
	pub post: ObjectId,
	pub user: ObjectId,
	pub value: i32,
}

mod token {
	use super::*;

	/// A 120-bit bearer token.
	#[derive(Deserialize, Serialize)]
	pub struct SessionToken(
		[u8; 24]
	);

	impl SessionToken {
		pub fn generate() -> Self {
			// emphasizing and asserting that this is a safe source of random tokens
			fn thread_crypto_rng() -> impl rand::CryptoRng + rand::Rng {
				rand::thread_rng()
			}

			let mut rng = thread_crypto_rng();
			let mut result = Self(Default::default());
			rng.fill_bytes(&mut result.0);

			for c in &mut result.0 {
				*c = (*c & 0x2f) + b'A';
			}

			result
		}

		pub fn hash(&self) -> SessionTokenHash {
			let mut hasher = Blake2b::<U16>::new();
			hasher.update(self.0);
			SessionTokenHash(hasher.finalize().into())
		}
	}

	impl fmt::Display for SessionToken {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			write!(f, "{}", str::from_utf8(&self.0).unwrap())
		}
	}

	impl FromStr for SessionToken {
		type Err = InvalidTokenFormat;

		fn from_str(s: &str) -> Result<Self, Self::Err> {
			if s.len() == 24 && s.bytes().all(|b| (b'a'..=b'p').contains(&(b | 32))) {
				Ok(Self(s.as_bytes().try_into().unwrap()))
			} else {
				Err(InvalidTokenFormat)
			}
		}
	}

	#[derive(Debug)]
	pub struct InvalidTokenFormat;

	/// A 128-bit hash of a [`SessionToken`].
	#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
	#[serde(into = "Binary", try_from = "Binary")]
	pub struct SessionTokenHash([u8; 16]);

	impl From<SessionTokenHash> for Binary {
		fn from(hash: SessionTokenHash) -> Self {
			Self {
				subtype: BinarySubtype::Generic,
				bytes: hash.0.into(),
			}
		}
	}

	impl From<SessionTokenHash> for Bson {
		fn from(hash: SessionTokenHash) -> Self {
			Self::from(Binary::from(hash))
		}
	}

	impl TryFrom<Binary> for SessionTokenHash {
		type Error = InvalidHashLength;

		fn try_from(b: Binary) -> Result<Self, Self::Error> {
			b.bytes.try_into()
				.map(Self)
				.map_err(|_| InvalidHashLength)
		}
	}

	#[derive(Debug)]
	pub struct InvalidHashLength;

	impl fmt::Display for InvalidHashLength {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			write!(f, "invalid hash length")
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Rfc3339DateTime(DateTime);

// Custom deserializer for `Rfc3339DateTime` (`bson::DateTime` wrapper).
impl<'de> Deserialize<'de> for Rfc3339DateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let datetime = DateTime::deserialize(deserializer)?;
        datetime.try_to_rfc3339_string().map_err(|e| D::Error::custom(format!("Error deserializing: {e}")))?;
        Ok(Rfc3339DateTime(datetime))
    }
}

// Custom serializer for `Rfc3339DateTime` (`bson::DateTime` wrapper).
impl Serialize for Rfc3339DateTime {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
			S: ser::Serializer,
	{
		let timestamp = self.0.try_to_rfc3339_string().map_err(|e| ser::Error::custom(format!("Error serializing: {e}")))?;
		timestamp.serialize(serializer)
	}
}
