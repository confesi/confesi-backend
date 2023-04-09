//! Encrypts MongoDB ObjectIDs to avoid leaking information and having enumerable resources.

use std::error::Error;
use std::fmt;

use aes::cipher::{
	BlockDecrypt,
	BlockEncrypt,
	KeyInit,
};
use aes::Aes128;
use mongodb::bson::oid::ObjectId;
use serde::{
	Deserialize,
	Serialize,
};

const TYPE_OBJECT_ID: u8 = 0;
const TYPE_SEQUENTIAL_ID: u8 = 1;

#[derive(Clone, Deserialize, Serialize)]
pub struct MaskedObjectId(#[serde(with = "crate::base64_serde")] [u8; 16]);

#[derive(Clone, Deserialize, Serialize)]
pub struct MaskedSequentialId(#[serde(with = "crate::base64_serde")] [u8; 16]);

pub struct MaskingKey(Aes128);

#[derive(Clone, Copy, Debug)]
pub struct PaddingError;

impl fmt::Display for PaddingError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Invalid id")
	}
}

impl Error for PaddingError {}

impl MaskingKey {
	pub fn new(key: &[u8; 16]) -> Self {
		Self(Aes128::new(key.into()))
	}

	pub fn mask(&self, oid: &ObjectId) -> MaskedObjectId {
		let mut plain_block = [TYPE_OBJECT_ID; 16];
		plain_block[0..12].copy_from_slice(&oid.bytes());

		let mut result = MaskedObjectId(Default::default());
		self.0
			.encrypt_block_b2b((&plain_block).into(), (&mut result.0).into());
		result
	}

	pub fn unmask(&self, masked_oid: &MaskedObjectId) -> Result<ObjectId, PaddingError> {
		let mut plain_block = [0_u8; 16];
		self.0
			.decrypt_block_b2b((&masked_oid.0).into(), (&mut plain_block).into());

		if plain_block[12..16] == [TYPE_OBJECT_ID; 4] {
			Ok(ObjectId::from_bytes(plain_block[0..12].try_into().unwrap()))
		} else {
			Err(PaddingError)
		}
	}

	// TODO: `Maskable` trait
	pub fn mask_sequential(&self, seq: u64) -> MaskedSequentialId {
		let mut plain_block = [TYPE_SEQUENTIAL_ID; 16];
		plain_block[0..8].copy_from_slice(&seq.to_le_bytes());

		let mut result = MaskedSequentialId(Default::default());
		self.0
			.encrypt_block_b2b((&plain_block).into(), (&mut result.0).into());
		result
	}

	pub fn unmask_sequential(&self, masked_seq: &MaskedSequentialId) -> Result<u64, PaddingError> {
		let mut plain_block = [0_u8; 16];
		self.0
			.decrypt_block_b2b((&masked_seq.0).into(), (&mut plain_block).into());

		if plain_block[8..16] == [TYPE_SEQUENTIAL_ID; 8] {
			Ok(u64::from_le_bytes(plain_block[0..8].try_into().unwrap()))
		} else {
			Err(PaddingError)
		}
	}
}
