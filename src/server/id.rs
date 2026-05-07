use uuid::Uuid;

use crate::server::error::AppError;

/// Encode a UUID as a 22-character base58 string (Bitcoin alphabet).
pub fn encode(uuid: Uuid) -> String {
    bs58::encode(uuid.as_bytes()).into_string()
}

/// Decode a base58 string back to a UUID.
pub fn decode(s: &str) -> Result<Uuid, AppError> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|_| AppError::BadRequest("invalid id".into()))?;
    Uuid::from_slice(&bytes).map_err(|_| AppError::BadRequest("invalid id".into()))
}
