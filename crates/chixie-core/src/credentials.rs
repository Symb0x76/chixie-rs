//! "Remember me" for the ChiXie account: the password lives only in the OS credential store
//! (Windows Credential Manager, via `keyring`). Settings keep the account (phone number) but
//! never the password. ChiXie3 itself stores the password in plaintext in
//! `%USERPROFILE%\ChiXie3\conf\config.ini [LoginConfig]`; we do not copy that.

use crate::{Error, Result};

/// Credential Manager target name prefix; entries show up as `chixie-rs` / `<account>`.
const SERVICE: &str = "chixie-rs";

fn entry(account: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, account).map_err(|e| Error::Credential(e.to_string()))
}

/// Store (or replace) the password for `account`.
pub fn save(account: &str, password: &str) -> Result<()> {
    entry(account)?.set_password(password).map_err(|e| Error::Credential(e.to_string()))
}

/// The stored password, or `None` if nothing was saved for `account`.
pub fn load(account: &str) -> Result<Option<String>> {
    match entry(account)?.get_password() {
        Ok(p) => Ok(Some(p)),
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
    /// `cargo test -p chixie-core credentials -- --ignored`.
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
