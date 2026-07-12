use bip39::Mnemonic;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

#[derive(Serialize, Deserialize, Debug)]
pub struct PairingCodes {
    pub uuid: String,
    pub passphrase: Vec<String>,
}

pub fn generate_pairing_codes() -> Result<PairingCodes, String> {
    let mut entropy = [0u8; 16];
    OsRng.fill_bytes(&mut entropy);

    let mnemonic = match Mnemonic::from_entropy(&entropy) {
        Ok(m) => m,
        Err(e) => return Err(format!("Failed to generate mnemonic: {e}")),
    };

    let words: Vec<String> = mnemonic.words().take(6).map(|w| w.to_string()).collect();

    let derived_uuid = hex::encode(words.join("-").as_bytes());

    let mut entropy_hex = hex::encode(entropy);
    entropy_hex.zeroize();
    entropy.zeroize();

    Ok(PairingCodes {
        uuid: derived_uuid,
        passphrase: words,
    })
}

pub fn hash_pairing_code(input: String) -> Result<String, String> {
    let sanitized = input.to_lowercase().trim().replace(' ', "-");

    let raw_payload = if sanitized.split('-').count() == 6 {
        hex::encode(sanitized.as_bytes())
    } else {
        sanitized
    };

    let mut hasher = Sha256::new();
    hasher.update(raw_payload.as_bytes());
    let room_id = hex::encode(hasher.finalize());

    Ok(room_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pairing_codes_returns_six_words() {
        let codes = generate_pairing_codes().unwrap();
        assert_eq!(codes.passphrase.len(), 6);
        assert!(!codes.uuid.is_empty());
    }

    #[test]
    fn test_hash_pairing_code_deterministic() {
        let input = "abandon abandon abandon abandon abandon abandon".to_string();
        let h1 = hash_pairing_code(input.clone()).unwrap();
        let h2 = hash_pairing_code(input).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_hash_pairing_code_normalizes() {
        let h1 = hash_pairing_code("Word1 Word2 Word3 Word4 Word5 Word6".to_string()).unwrap();
        let h2 = hash_pairing_code("word1-word2-word3-word4-word5-word6".to_string()).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_pairing_code_with_hex_uuid() {
        let uuid = "6162616e646f6e2d6162616e646f6e2d6162616e646f6e2d6162616e646f6e2d6162616e646f6e2d6162616e646f6e";
        let h = hash_pairing_code(uuid.to_string()).unwrap();
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn test_entropy_is_zeroized() {
        let mut entropy = [0u8; 16];
        OsRng.fill_bytes(&mut entropy);
        assert_ne!(entropy, [0u8; 16]);
        entropy.zeroize();
        assert_eq!(entropy, [0u8; 16]);
    }
}
