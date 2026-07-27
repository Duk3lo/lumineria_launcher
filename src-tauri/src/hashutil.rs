use sha1::Sha1;
use sha2::{Digest, Sha256};

pub fn hash_bytes(bytes: &[u8], format: &str) -> Option<String> {
    match format.to_lowercase().as_str() {
        "sha256" => { let mut h = Sha256::new(); h.update(bytes); Some(hex::encode(h.finalize())) }
        "sha1" => { let mut h = Sha1::new(); h.update(bytes); Some(hex::encode(h.finalize())) }
        _ => None,
    }
}

pub fn verify_bytes(bytes: &[u8], expected: &str, format: &str) -> Result<(), String> {
    match hash_bytes(bytes, format) {
        Some(computed) if computed.eq_ignore_ascii_case(expected) => Ok(()),
        Some(computed) => Err(format!("hash no coincide (esperado {expected}, obtenido {computed})")),
        None => Err(format!("formato de hash desconocido: {format}")),
    }
}