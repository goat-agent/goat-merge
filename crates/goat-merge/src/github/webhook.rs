use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const SIGNATURE_HEADER: &str = "x-hub-signature-256";
pub const EVENT_HEADER: &str = "x-github-event";
pub const DELIVERY_HEADER: &str = "x-github-delivery";

pub fn signature_holds(secret: &str, body: &[u8], claimed: &str) -> bool {
    let Some(hex) = claimed.strip_prefix("sha256=") else {
        return false;
    };
    let Some(offered) = from_hex(hex) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&offered).is_ok()
}

pub fn sign(secret: &str, body: &[u8]) -> String {
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return String::new();
    };
    mac.update(body);
    let tag = mac.finalize().into_bytes();
    format!(
        "sha256={}",
        tag.iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn from_hex(written: &str) -> Option<Vec<u8>> {
    if !written.len().is_multiple_of(2) {
        return None;
    }
    written
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect()
}
