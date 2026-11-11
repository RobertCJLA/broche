// SPDX-License-Identifier: MPL-2.0
//! Linux: seals the device key (and the other sealed files) with a random
//! wrapping key kept in the desktop keyring: the Secret Service, or the Secret
//! portal inside a Flatpak (`oo7` picks). If the keyring entry is deleted, the
//! key can't be opened and the devices have to be paired again.

/// The protector ID written into the key file. Known everywhere, so a build
/// without the keyring can say why it can't open the key.
pub(crate) const ID: u8 = 3;

#[cfg(all(target_os = "linux", feature = "secret-service"))]
pub(crate) use imp::KeyringProtector;

#[cfg(all(target_os = "linux", feature = "secret-service"))]
mod imp {
    use std::{io, time::Duration};

    use ring::{
        aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
        rand::{SecureRandom, SystemRandom},
    };

    use crate::identity::KeyProtector;

    const MAGIC: &[u8; 4] = b"NLS1";
    const LABEL: &str = "Broche device key";
    const ATTRIBUTES: [(&str, &str); 2] =
        [("application", "io.github.robertcjla.Broche"), ("purpose", "device-key-wrap")];
    /// Unlocking may show a password prompt: wait for it, but not forever.
    const WAIT: Duration = Duration::from_secs(120);

    pub(crate) struct KeyringProtector {
        key: LessSafeKey,
        rng: SystemRandom,
    }

    fn other(e: impl std::fmt::Display) -> io::Error {
        io::Error::other(e.to_string())
    }

    impl KeyringProtector {
        /// Reads the wrapping key from the keyring, creating it the first time.
        pub(crate) fn open() -> io::Result<Self> {
            // Own thread: this runs while the node starts, inside its runtime. The runtime must
            // outlive this call: in a Flatpak oo7 goes through ashpd, whose shared D-Bus
            // connection then lives on this runtime and serves every later portal request.
            static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
            let wrap = std::thread::spawn(|| {
                let rt = match RT.get() {
                    Some(rt) => rt,
                    None => {
                        let rt = tokio::runtime::Builder::new_multi_thread()
                            .worker_threads(1)
                            .thread_name("keyring")
                            .enable_all()
                            .build()?;
                        RT.get_or_init(|| rt)
                    }
                };
                rt.block_on(async {
                    tokio::time::timeout(WAIT, wrapping_key())
                        .await
                        .map_err(|_| other("the keyring did not answer"))?
                })
            })
            .join()
            .map_err(|_| other("the keyring thread panicked"))??;
            Self::from_key(wrap.as_bytes())
        }

        fn from_key(key: &[u8]) -> io::Result<Self> {
            let key =
                UnboundKey::new(&AES_256_GCM, key).map_err(|_| other("the wrapping key is not 32 bytes"))?;
            Ok(Self { key: LessSafeKey::new(key), rng: SystemRandom::new() })
        }
    }

    async fn wrapping_key() -> io::Result<oo7::Secret> {
        let keyring = oo7::Keyring::new().await.map_err(other)?;
        keyring.unlock().await.map_err(other)?;
        if let Some(item) = keyring.search_items(&ATTRIBUTES).await.map_err(other)?.first() {
            item.unlock().await.map_err(other)?;
            return item.secret().await.map_err(other);
        }
        let mut key = [0u8; 32];
        SystemRandom::new().fill(&mut key).map_err(|_| other("no random numbers"))?;
        let secret = oo7::Secret::blob(key);
        key.fill(0);
        keyring.create_item(LABEL, &ATTRIBUTES, secret.clone(), false).await.map_err(other)?;
        Ok(secret)
    }

    impl KeyProtector for KeyringProtector {
        fn id(&self) -> u8 {
            super::ID
        }

        fn protect(&self, plaintext: &[u8]) -> io::Result<Vec<u8>> {
            let mut nonce = [0u8; NONCE_LEN];
            self.rng.fill(&mut nonce).map_err(|_| other("no random numbers"))?;
            let mut sealed = plaintext.to_vec();
            self.key
                .seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(MAGIC), &mut sealed)
                .map_err(|_| other("can't seal"))?;
            Ok([MAGIC.as_slice(), &nonce, &sealed].concat())
        }

        fn unprotect(&self, ciphertext: &[u8]) -> io::Result<Vec<u8>> {
            let Some(rest) = ciphertext.strip_prefix(MAGIC.as_slice()) else {
                // Clipboard history and LocalSend files written before the keyring was used
                // were not sealed: read them as they are; the next save seals them.
                return Ok(ciphertext.to_vec());
            };
            if rest.len() < NONCE_LEN {
                return Err(other("sealed data is too short"));
            }
            let (nonce, sealed) = rest.split_at(NONCE_LEN);
            let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| other("bad nonce"))?;
            let mut buf = sealed.to_vec();
            let plain = self
                .key
                .open_in_place(nonce, Aad::from(MAGIC), &mut buf)
                .map_err(|_| other("sealed data was changed or belongs to another keyring"))?;
            Ok(plain.to_vec())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn seals_opens_and_reads_older_unsealed_files() {
            let p = KeyringProtector::from_key(&[7; 32]).unwrap();
            let sealed = p.protect(b"secret").unwrap();
            assert!(!sealed.windows(6).any(|w| w == b"secret"));
            assert_eq!(p.unprotect(&sealed).unwrap(), b"secret");
            assert_ne!(p.protect(b"secret").unwrap(), sealed, "fresh nonce each time");

            let mut tampered = sealed.clone();
            *tampered.last_mut().unwrap() ^= 1;
            assert!(p.unprotect(&tampered).is_err());
            let other_key = KeyringProtector::from_key(&[8; 32]).unwrap();
            assert!(other_key.unprotect(&sealed).is_err());

            assert_eq!(p.unprotect(b"{\"items\":[]}").unwrap(), b"{\"items\":[]}");
        }
    }
}
