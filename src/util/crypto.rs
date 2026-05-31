use aes_gcm::{
	Aes256Gcm, Key, Nonce,
	aead::{Aead, AeadCore, KeyInit, OsRng},
};
use base64::Engine;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
	#[error("Failed to decode base64: {0}")]
	Base64Decode(#[from] base64::DecodeError),
	#[error("Encryption failed: {0}")]
	EncryptionFailed(String),
	#[error("Decryption failed: {0}")]
	DecryptionFailed(String),
	#[error("Ciphertext too short to contain nonce")]
	InvalidLength,
}

use std::sync::OnceLock;

static CIPHER: OnceLock<Aes256Gcm> = OnceLock::new();

pub fn init_cipher(key_bytes: &[u8]) -> Result<(), String> {
	CIPHER
		.set(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key_bytes)))
		.map_err(|_| "Cipher already initialized".to_string())
}

pub fn encrypt_string(plain_password: &str) -> Result<String, CryptoError> {
	let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

	let cipher = CIPHER
		.get()
		.ok_or_else(|| CryptoError::EncryptionFailed("Cipher not initialized".to_string()))?;
	let ciphertext = cipher
		.encrypt(&nonce, plain_password.as_bytes())
		.map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

	let mut combined = nonce.to_vec();
	combined.extend_from_slice(&ciphertext);

	Ok(base64::engine::general_purpose::STANDARD.encode(combined))
}

pub fn decrypt_string(encrypted_b64: &str) -> Result<String, CryptoError> {
	let combined = base64::engine::general_purpose::STANDARD.decode(encrypted_b64)?;

	if combined.len() < 12 {
		return Err(CryptoError::InvalidLength);
	}

	let (nonce_bytes, ciphertext) = combined.split_at(12);
	let nonce = Nonce::from_slice(nonce_bytes);

	let cipher = CIPHER
		.get()
		.ok_or_else(|| CryptoError::DecryptionFailed("Cipher not initialized".to_string()))?;
	let decrypted = cipher
		.decrypt(nonce, ciphertext)
		.map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

	String::from_utf8(decrypted).map_err(|_| CryptoError::DecryptionFailed("Invalid UTF-8".into()))
}
