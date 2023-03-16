use serde::Deserialize;
use serde_json::from_str as from_json;

#[derive(Deserialize)]
struct Foo(
	#[serde(with = "super")]
	[u8; 2]
);

#[test]
fn test_correct_input() {
	let result = from_json::<Foo>("\"abA\"");
	assert!(result.is_ok());
}

#[test]
fn test_missized_input() {
	let result = from_json::<Foo>("\"ab\"");
	assert!(result.is_err());

	let result = from_json::<Foo>("\"abcd\"");
	assert!(result.is_err());

	let result = from_json::<Foo>("\"abB\"");
	assert!(result.is_err());
}
