use bcrypt::{hash, verify, DEFAULT_COST};

pub fn hash_password(password: &str) -> Result<String, anyhow::Error> {
    let hashed = hash(password, DEFAULT_COST)
        .map_err(|e| anyhow::anyhow!("Password hashing error: {:?}", e))?;
    Ok(hashed)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, anyhow::Error> {
    match verify(password, hash) {
        Ok(is_valid) => Ok(is_valid),
        Err(_) => Ok(false),
    }
}
