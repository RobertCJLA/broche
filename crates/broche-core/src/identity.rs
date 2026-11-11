// SPDX-License-Identifier: MPL-2.0
//! The device key: created on first launch, stored encrypted at rest.

use std::{
    fs,
    io::{self, Write},
    path::Path,
    sync::Arc,
};

use iroh::SecretKey;

use crate::{Error, Result};

const FILE_NAME: &str = "identity.key";
const MAGIC: &[u8; 4] = b"NLK1";

/// Protects the device key at rest. Each platform provides one: Android Keystore wrapping on Android (injected by the app), the
/// desktop keyring on Linux (feature `secret-service`).
pub trait KeyProtector: Send + Sync {
    /// A stable one-byte identifier written into the key file.
    fn id(&self) -> u8;
    fn protect(&self, plaintext: &[u8]) -> io::Result<Vec<u8>>;
    fn unprotect(&self, ciphertext: &[u8]) -> io::Result<Vec<u8>>;
}

/// No protection. Only used where no platform protector exists (tests,
/// headless tools); a warning is logged when it is used for a real key.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlainKeyProtector;

impl KeyProtector for PlainKeyProtector {
    fn id(&self) -> u8 {
        0
    }
    fn protect(&self, plaintext: &[u8]) -> io::Result<Vec<u8>> {
        Ok(plaintext.to_vec())
    }
    fn unprotect(&self, ciphertext: &[u8]) -> io::Result<Vec<u8>> {
        Ok(ciphertext.to_vec())
    }
}

/// The best protector available on this platform.
pub fn default_protector() -> Arc<dyn KeyProtector> {
    #[cfg(all(target_os = "linux", feature = "secret-service"))]
    {
        match crate::keyring::KeyringProtector::open() {
            Ok(p) => Arc::new(p),
            Err(e) => {
                tracing::warn!("desktop keyring unavailable, keeping the owner-only key file: {e}");
                Arc::new(PlainKeyProtector)
            }
        }
    }
    #[cfg(not(all(target_os = "linux", feature = "secret-service")))]
    {
        Arc::new(PlainKeyProtector)
    }
}

/// Loads the device key from `dir`, creating and saving a new one on first
/// launch.
pub(crate) fn load_or_create(dir: &Path, protector: &dyn KeyProtector) -> Result<SecretKey> {
    let path = dir.join(FILE_NAME);
    restrict_permissions(dir, &path);
    match fs::read(&path) {
        // A key saved before this platform had a protector: seal it now, same identity.
        Ok(bytes) if protector.id() != 0 && bytes.get(MAGIC.len()) == Some(&0) => {
            let key = decode(&bytes, &PlainKeyProtector)?;
            write_atomically(&path, &encode(&key, protector)?)?;
            tracing::info!(method = protector.id(), "device key sealed by the platform protector");
            Ok(key)
        }
        Ok(bytes) => decode(&bytes, protector),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let key = SecretKey::generate();
            if protector.id() == 0 {
                tracing::warn!("device key is stored without platform protection");
            }
            write_atomically(&path, &encode(&key, protector)?)?;
            tracing::info!(device = %crate::device_id(&key.public()).short(), "created device identity");
            Ok(key)
        }
        Err(e) => Err(e.into()),
    }
}

fn encode(key: &SecretKey, protector: &dyn KeyProtector) -> Result<Vec<u8>> {
    let sealed = protector.protect(&key.to_bytes())?;
    let mut out = Vec::with_capacity(MAGIC.len() + 1 + sealed.len());
    out.extend_from_slice(MAGIC);
    out.push(protector.id());
    out.extend_from_slice(&sealed);
    Ok(out)
}

fn decode(bytes: &[u8], protector: &dyn KeyProtector) -> Result<SecretKey> {
    let corrupt = || Error::Storage("the device key file is corrupt".into());
    let rest = bytes.strip_prefix(MAGIC.as_slice()).ok_or_else(corrupt)?;
    let (&method, sealed) = rest.split_first().ok_or_else(corrupt)?;
    if method == crate::keyring::ID && protector.id() != method {
        return Err(Error::Storage(
            "the device key is sealed by the desktop keyring, which is locked or unavailable: \
             unlock it and start again"
                .into(),
        ));
    }
    if method != protector.id() {
        return Err(Error::Storage(format!(
            "the device key is protected with method {method}, but this build uses {}",
            protector.id()
        )));
    }
    let plain =
        protector.unprotect(sealed).map_err(|e| Error::Storage(format!("cannot unlock device key: {e}")))?;
    let bytes: [u8; 32] = plain.as_slice().try_into().map_err(|_| corrupt())?;
    Ok(SecretKey::from_bytes(&bytes))
}

/// Writes via a temporary file and rename, so a crash never leaves a
/// half-written key behind.
/// The file is created owner-only (0600 on Unix) so the key is never readable
/// by other users, not even briefly.
fn write_atomically(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        #[cfg(unix)]
        {
            // `mode` only applies to new files; a stale temp file keeps its old mode.
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Tightens an existing data folder and key file to owner-only (0700/0600).
/// Older builds created them 0755/0644. Failures are logged, not fatal: the
/// folder may be on a filesystem without Unix permissions.
#[cfg(unix)]
fn restrict_permissions(dir: &Path, key: &Path) {
    use std::os::unix::fs::PermissionsExt;
    for (path, mode) in [(dir, 0o700), (key, 0o600)] {
        let Ok(meta) = fs::metadata(path) else { continue };
        if meta.permissions().mode() & 0o077 != 0 {
            match fs::set_permissions(path, fs::Permissions::from_mode(mode)) {
                Ok(()) => {
                    tracing::info!(path = %path.display(), mode = format!("{mode:o}"), "restricted permissions")
                }
                Err(e) => tracing::warn!(path = %path.display(), "cannot restrict permissions: {e}"),
            }
        }
    }
}

#[cfg(not(unix))]
fn restrict_permissions(_dir: &Path, _key: &Path) {}

/// The install ID a device reports to its peers ([`DeviceInfo::install_id`]): a salted
/// BLAKE3 hash of a platform ID that survives app reinstalls (Android's `ANDROID_ID`,
/// Windows' `MachineGuid`). The raw ID never leaves the device; the salt keeps the hash
/// specific to Broche, so it can't be matched against other apps' identifiers.
///
/// [`DeviceInfo::install_id`]: broche_protocol::messages::DeviceInfo::install_id
pub fn install_id_from(platform_id: &str) -> Option<String> {
    let raw = platform_id.trim();
    if raw.is_empty() {
        return None;
    }
    let mut hasher = blake3::Hasher::new_derive_key("broche 2026 install id v1");
    hasher.update(raw.as_bytes());
    let hex = hasher.finalize().to_hex();
    Some(hex[..broche_protocol::messages::INSTALL_ID_LEN].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_then_reloads_the_same_key() {
        let dir = tempfile::tempdir().unwrap();
        let protector = default_protector();
        let first = load_or_create(dir.path(), protector.as_ref()).unwrap();
        let second = load_or_create(dir.path(), protector.as_ref()).unwrap();
        assert_eq!(first.public(), second.public());
    }

    #[test]
    fn key_file_does_not_contain_the_plain_key_when_protected() {
        let dir = tempfile::tempdir().unwrap();
        let protector = default_protector();
        let key = load_or_create(dir.path(), protector.as_ref()).unwrap();
        let file = fs::read(dir.path().join(FILE_NAME)).unwrap();
        if protector.id() != 0 {
            let raw = key.to_bytes();
            assert!(!file.windows(raw.len()).any(|w| w == raw), "plaintext key found on disk");
        }
    }

    #[cfg(unix)]
    #[test]
    fn key_and_folder_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join(FILE_NAME);

        load_or_create(dir.path(), &PlainKeyProtector).unwrap();
        assert_eq!(mode(&key), 0o600);

        // Files left by older builds (0755/0644) are tightened on the next start.
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
        let before = load_or_create(dir.path(), &PlainKeyProtector).unwrap();
        assert_eq!(mode(dir.path()), 0o700);
        assert_eq!(mode(&key), 0o600);
        assert_eq!(before.public(), load_or_create(dir.path(), &PlainKeyProtector).unwrap().public());
    }

    /// Flips every byte, so sealed bytes differ from the plaintext.
    struct Flip;
    impl KeyProtector for Flip {
        fn id(&self) -> u8 {
            9
        }
        fn protect(&self, plaintext: &[u8]) -> io::Result<Vec<u8>> {
            Ok(plaintext.iter().map(|b| !b).collect())
        }
        fn unprotect(&self, ciphertext: &[u8]) -> io::Result<Vec<u8>> {
            self.protect(ciphertext)
        }
    }

    #[test]
    fn an_unprotected_key_is_sealed_by_a_new_protector_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let plain = load_or_create(dir.path(), &PlainKeyProtector).unwrap();

        let migrated = load_or_create(dir.path(), &Flip).unwrap();
        assert_eq!(migrated.public(), plain.public(), "same identity, no re-pairing");
        let file = fs::read(dir.path().join(FILE_NAME)).unwrap();
        assert_eq!(file[MAGIC.len()], 9, "rewritten with the new protector");
        assert!(!file.windows(32).any(|w| w == plain.to_bytes()), "no plaintext key left in the file");
        assert_eq!(load_or_create(dir.path(), &Flip).unwrap().public(), plain.public());
    }

    #[test]
    fn rejects_corrupt_and_mismatched_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE_NAME), b"garbage").unwrap();
        assert!(matches!(load_or_create(dir.path(), &PlainKeyProtector), Err(Error::Storage(_))));

        let mut wrong_method = MAGIC.to_vec();
        wrong_method.push(42);
        wrong_method.extend_from_slice(&[0; 32]);
        fs::write(dir.path().join(FILE_NAME), wrong_method).unwrap();
        assert!(matches!(load_or_create(dir.path(), &PlainKeyProtector), Err(Error::Storage(_))));
    }

    #[test]
    fn install_ids_are_stable_salted_and_well_formed() {
        let a = install_id_from("9774d56d682e549c").unwrap();
        assert_eq!(a, install_id_from(" 9774d56d682e549c ").unwrap(), "stable for the same device");
        assert_ne!(a, install_id_from("9774d56d682e549d").unwrap(), "differs per device");
        assert!(broche_protocol::messages::is_valid_install_id(&a));
        assert!(!a.contains("9774d56d682e549c"), "the raw ID never appears");
        assert_eq!(install_id_from("  "), None);
    }
}
