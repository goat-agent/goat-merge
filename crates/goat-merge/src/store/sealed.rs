use chacha20poly1305::aead::{Aead, KeyInit, OsRng, rand_core::RngCore};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

const NONCE: usize = 12;

#[derive(Clone)]
pub struct Seal {
    cipher: ChaCha20Poly1305,
}

impl Seal {
    pub fn holding(key: [u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(Key::from_slice(&key)),
        }
    }

    pub fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, SealError> {
        let mut nonce = [0_u8; NONCE];
        OsRng.fill_bytes(&mut nonce);
        let sealed = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plain)
            .map_err(|_| SealError::CouldNotSeal)?;
        Ok([nonce.as_slice(), sealed.as_slice()].concat())
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, SealError> {
        let (nonce, body) = sealed
            .split_at_checked(NONCE)
            .ok_or(SealError::Undecryptable)?;
        self.cipher
            .decrypt(Nonce::from_slice(nonce), body)
            .map_err(|_| SealError::Undecryptable)
    }

    pub fn open_text(&self, sealed: &[u8]) -> Result<String, SealError> {
        String::from_utf8(self.open(sealed)?).map_err(|_| SealError::Undecryptable)
    }
}

pub fn fresh_key() -> String {
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    key.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn key_from_hex(written: &str) -> Result<[u8; 32], SealError> {
    let trimmed = written.trim();
    if trimmed.len() != 64 {
        return Err(SealError::KeyIsNot32Bytes);
    }
    let mut key = [0_u8; 32];
    for (byte, pair) in key.iter_mut().zip(trimmed.as_bytes().chunks_exact(2)) {
        let pair = std::str::from_utf8(pair).map_err(|_| SealError::KeyIsNot32Bytes)?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| SealError::KeyIsNot32Bytes)?;
    }
    Ok(key)
}

#[derive(Debug, thiserror::Error)]
pub enum SealError {
    #[error("GOAT_MERGE_MASTER_KEY must be 64 hexadecimal characters, which is 32 bytes")]
    KeyIsNot32Bytes,
    #[error("a secret could not be encrypted")]
    CouldNotSeal,
    #[error(
        "a stored secret could not be decrypted with this GOAT_MERGE_MASTER_KEY. \
         Either the key changed or the row belongs to another installation"
    )]
    Undecryptable,
}
