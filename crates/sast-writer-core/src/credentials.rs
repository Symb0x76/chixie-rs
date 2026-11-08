//! "Remember me" for the ChiXie account: the password lives only in the OS credential store
//! (Windows Credential Manager, via `keyring`). Settings keep the account (phone number) but
//! never the password. ChiXie3 itself stores the password in plaintext in
//! `%USERPROFILE%\ChiXie3\conf\config.ini [LoginConfig]`; we do not copy that.

use crate::{Error, Result};

/// Credential store service name; entries show up as `SAST Writer` / `<account>`.
const SERVICE: &str = "SAST Writer";
/// Service name used before the rename; read once and moved to [`SERVICE`].
const LEGACY_SERVICE: &str = "chixie-rs";

fn entry(account: &str) -> Result<keyring::Entry> {
    entry_in(SERVICE, account)
}

fn entry_in(service: &str, account: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(service, account).map_err(|e| Error::Credential(e.to_string()))
}

/// Store (or replace) the password for `account`.
pub fn save(account: &str, password: &str) -> Result<()> {
    entry(account)?.set_password(password).map_err(|e| Error::Credential(e.to_string()))
}

/// The stored password, or `None` if nothing was saved for `account`. A password saved under the
/// app's former name is moved to the new one on first read.
pub fn load(account: &str) -> Result<Option<String>> {
    match entry(account)?.get_password() {
        Ok(p) => return Ok(Some(p)),
        Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(Error::Credential(e.to_string())),
    }
    let legacy = entry_in(LEGACY_SERVICE, account)?;
    match legacy.get_password() {
        Ok(p) => {
            save(account, &p)?;
            let _ = legacy.delete_credential();
            Ok(Some(p))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::Credential(e.to_string())),
    }
}

/// Remove the stored password; succeeds if there was none.
pub fn forget(account: &str) -> Result<()> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Credential(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real OS credential store, so it is opt-in:
    /// `cargo test -p sast-writer-core credentials -- --ignored`.
    #[test]
    #[ignore = "writes to the OS credential store"]
    fn roundtrip_in_os_store() {
        let account = format!("test-{}", std::process::id());
        forget(&account).unwrap();
        assert_eq!(load(&account).unwrap(), None);
        save(&account, "s3cret-测试").unwrap();
        assert_eq!(load(&account).unwrap().as_deref(), Some("s3cret-测试"));
        forget(&account).unwrap();
        assert_eq!(load(&account).unwrap(), None);
        forget(&account).unwrap(); // idempotent
    }
}
