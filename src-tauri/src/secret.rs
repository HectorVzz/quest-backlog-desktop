//! The device token lives only in the OS keychain (Windows Credential
//! Manager, Secret Service on Linux). Never in a plain file, never in the
//! webview. These calls block; run them on a blocking thread.

use keyring::{Entry, Error};

const SERVICE: &str = "com.questbacklog.QuestBacklog";
const ACCOUNT: &str = "device-token";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|error| error.to_string())
}

pub fn load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

pub fn save(token: &str) -> Result<(), String> {
    entry()?.set_password(token).map_err(|error| error.to_string())
}

pub fn clear() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
