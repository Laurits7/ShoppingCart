//! Encrypting stored integration tokens (SPEC §5.5, §7.4): XChaCha20-Poly1305 with a key from
//! `SECRET_KEY` or `data/secret.key` (32 random bytes, created on first start). Secrets are
//! never logged and never sent back to clients.

use std::path::Path;

use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub struct Secrets {
    cipher: XChaCha20Poly1305,
}

impl Secrets {
    /// The key from `SECRET_KEY` (any text; hashed to 32 bytes), else from `data/secret.key`,
    /// which is created on first start.
    pub fn load(data_dir: &Path, from_env: Option<&str>) -> anyhow::Result<Self> {
        let key: [u8; 32] = match from_env {
            Some(k) => Sha256::digest(k.trim().as_bytes()).into(),
            None => {
                let path = data_dir.join("secret.key");
                match std::fs::read_to_string(&path) {
                    Ok(s) => STANDARD
                        .decode(s.trim())
                        .ok()
                        .and_then(|b| <[u8; 32]>::try_from(b).ok())
                        .ok_or_else(|| anyhow::anyhow!("{} is not a valid key", path.display()))?,
                    Err(_) => {
                        let mut k = [0u8; 32];
                        rand::thread_rng().fill_bytes(&mut k);
                        std::fs::write(&path, STANDARD.encode(k))?;
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            std::fs::set_permissions(
                                &path,
                                std::fs::Permissions::from_mode(0o600),
                            )?;
                        }
                        k
                    }
                }
            }
        };
        Ok(Self {
            cipher: XChaCha20Poly1305::new(&key.into()),
        })
    }

    /// Encrypt; the result is base64 of a random 24-byte nonce followed by the ciphertext.
    pub fn seal(&self, plain: &str) -> String {
        let mut nonce = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        let mut out = nonce.to_vec();
        out.extend(
            self.cipher
                .encrypt(XNonce::from_slice(&nonce), plain.as_bytes())
                .expect("encryption can't fail"),
        );
        STANDARD.encode(out)
    }

    /// Decrypt what `seal` made; `None` if it was made with another key or tampered with.
    pub fn open(&self, sealed: &str) -> Option<String> {
        let b = STANDARD.decode(sealed).ok()?;
        if b.len() < 24 {
            return None;
        }
        let (nonce, ct) = b.split_at(24);
        let plain = self.cipher.decrypt(XNonce::from_slice(nonce), ct).ok()?;
        String::from_utf8(plain).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seals_and_opens() {
        let dir = std::env::temp_dir().join(format!("sc-secrets-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = Secrets::load(&dir, None).unwrap();
        let sealed = s.seal("mealie-token");
        assert_ne!(sealed, s.seal("mealie-token"), "a fresh nonce each time");
        assert_eq!(s.open(&sealed).as_deref(), Some("mealie-token"));
        // The key file is reused on the next start.
        let again = Secrets::load(&dir, None).unwrap();
        assert_eq!(again.open(&sealed).as_deref(), Some("mealie-token"));
        // Another key, or a changed byte: nothing.
        let other = Secrets::load(&dir, Some("another key")).unwrap();
        assert_eq!(other.open(&sealed), None);
        let mut b = STANDARD.decode(&sealed).unwrap();
        *b.last_mut().unwrap() ^= 1;
        assert_eq!(s.open(&STANDARD.encode(b)), None);
    }
}
