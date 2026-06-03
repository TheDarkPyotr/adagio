use crate::error::SyncError;
use crate::types::{Account, AccountId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Manages Nextcloud accounts in memory; credentials live in the OS keychain.
pub struct AccountManager {
    accounts: Arc<RwLock<HashMap<AccountId, Account>>>,
}

impl AccountManager {
    pub fn new() -> Self {
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new account. Credentials must already be stored in the keychain by the caller.
    pub fn add(&self, account: Account) -> Result<(), SyncError> {
        let mut map = self
            .accounts
            .write()
            .map_err(|_| SyncError::Fatal("account manager lock poisoned".into()))?;
        map.insert(account.id.clone(), account);
        Ok(())
    }

    /// Remove an account by ID. Returns the removed account, or None if not found.
    pub fn remove(&self, id: &AccountId) -> Result<Option<Account>, SyncError> {
        let mut map = self
            .accounts
            .write()
            .map_err(|_| SyncError::Fatal("account manager lock poisoned".into()))?;
        Ok(map.remove(id))
    }

    /// List all registered accounts (cloned).
    pub fn list(&self) -> Result<Vec<Account>, SyncError> {
        let map = self
            .accounts
            .read()
            .map_err(|_| SyncError::Fatal("account manager lock poisoned".into()))?;
        Ok(map.values().cloned().collect())
    }

    /// Get a single account by ID.
    pub fn get(&self, id: &AccountId) -> Result<Option<Account>, SyncError> {
        let map = self
            .accounts
            .read()
            .map_err(|_| SyncError::Fatal("account manager lock poisoned".into()))?;
        Ok(map.get(id).cloned())
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Account;
    use chrono::Utc;

    fn make_account() -> Account {
        let id = AccountId::new();
        Account {
            id,
            display_name: "Test User".into(),
            server_url: "https://cloud.example.com".into(),
            username: "testuser".into(),
            keychain_service_key: "adagio/test".into(),
            created_at: Utc::now(),
            upload_limit_kbps: 0,
            download_limit_kbps: 0,
        }
    }

    #[test]
    fn add_and_get_round_trips() {
        let mgr = AccountManager::new();
        let acct = make_account();
        let id = acct.id.clone();
        mgr.add(acct.clone()).unwrap();
        let found = mgr.get(&id).unwrap().expect("should find added account");
        assert_eq!(found.id, id);
        assert_eq!(found.username, "testuser");
    }

    #[test]
    fn list_returns_all_accounts() {
        let mgr = AccountManager::new();
        mgr.add(make_account()).unwrap();
        mgr.add(make_account()).unwrap();
        assert_eq!(mgr.list().unwrap().len(), 2);
    }

    #[test]
    fn remove_returns_account_and_makes_it_absent() {
        let mgr = AccountManager::new();
        let acct = make_account();
        let id = acct.id.clone();
        mgr.add(acct).unwrap();
        let removed = mgr.remove(&id).unwrap();
        assert!(removed.is_some());
        assert!(mgr.get(&id).unwrap().is_none());
    }

    #[test]
    fn get_returns_none_for_unknown_id() {
        let mgr = AccountManager::new();
        let result = mgr.get(&AccountId::new()).unwrap();
        assert!(result.is_none());
    }
}
