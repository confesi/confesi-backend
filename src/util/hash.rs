use bcrypt;

pub fn hash_password(p: &str) -> Result<String, bcrypt::BcryptError> {
	let cost = 10;
	return bcrypt::hash(p, cost);
}

pub fn compare(plain: &str, hash: &str) -> Result<bool, bcrypt::BcryptError> {
	return bcrypt::verify(plain, hash);
}
