#[cfg(test)]
mod tests;

use std::fmt;

use base64::display::Base64Display;
use serde::de::{Deserializer, Error, Unexpected, Visitor};
use serde::ser::Serializer;

pub fn serialize<S: Serializer>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
	serializer.collect_str(&Base64Display::with_config(value, base64::URL_SAFE_NO_PAD))
}

pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
where
	D: Deserializer<'de>,
{
	struct Base64ArrayVisitor<const N: usize>;

	impl<'de, const N: usize> Visitor<'de> for Base64ArrayVisitor<N> {
		type Value = [u8; N];

		fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
			write!(formatter, "a base64-encoded string of {} bytes", N)
		}

		fn visit_str<E: Error>(self, s: &str) -> Result<Self::Value, E> {
			if (N * 4 + 2) / 3 == s.len() {
				let mut result = [0_u8; N];

				match base64::decode_config_slice(s, base64::URL_SAFE_NO_PAD, &mut result) {
					Ok(decoded_count) if decoded_count == N => {
						return Ok(result);
					}
					_ => {}
				}
			}

			Err(Error::invalid_value(Unexpected::Str(s), &self))
		}
	}

	deserializer.deserialize_str(Base64ArrayVisitor)
}
