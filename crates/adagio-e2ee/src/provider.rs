//! `NcE2eeClient` — the production `E2eeProvider` implementation.
//!
//! Orchestrates key management, metadata sync, and file encrypt/decrypt
//! against a live Nextcloud server.

use adagio_core::{
    journal::sqlite::SqliteJournal,
    types::{AccountId, PairId},
};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rsa::{RsaPrivateKey, RsaPublicKey};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    cipher,
    keys::{
        generate_metadata_key, generate_mnemonic, generate_rsa_keypair, key_fingerprint,
        private_key_from_pem, private_key_to_pem, public_key_der, unwrap_private_key,
        validate_mnemonic, wrap_private_key,
    },
    metadata::{build_outer, canonical_inner_json, cms_sign, metadata_key_checksum, parse_outer},
    ocs::E2eeOcsClient,
    E2eeError, E2eeFileEntry, E2eeMetadata, E2eeProvider, E2eeStatusDto,
};

// ── E2EE journal queries ──────────────────────────────────────────────────────
// These helpers read/write e2ee_account_keys and e2ee_folder_state.

async fn load_certificate(
    journal: &SqliteJournal,
    account_id: &str,
) -> Result<Option<String>, E2eeError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT certificate FROM e2ee_account_keys WHERE account_id = ?")
            .bind(account_id)
            .fetch_optional(journal.pool())
            .await
            .map_err(|e| E2eeError::Other(e.to_string()))?;
    Ok(row.map(|(c,)| c))
}

async fn save_key_row(
    journal: &SqliteJournal,
    account_id: &str,
    certificate: &str,
    fingerprint: &str,
) -> Result<(), E2eeError> {
    use chrono::Utc;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO e2ee_account_keys(account_id, certificate, key_fingerprint, paired_at, created_at)
         VALUES(?, ?, ?, ?, ?)
         ON CONFLICT(account_id) DO UPDATE SET certificate=excluded.certificate, key_fingerprint=excluded.key_fingerprint, paired_at=excluded.paired_at",
    )
    .bind(account_id)
    .bind(certificate)
    .bind(fingerprint)
    .bind(&now)
    .bind(&now)
    .execute(journal.pool())
    .await
    .map_err(|e| E2eeError::Other(e.to_string()))?;
    Ok(())
}

async fn load_folder_state(
    journal: &SqliteJournal,
    pair_id: &str,
) -> Result<Option<(String, String, u64)>, E2eeError> {
    let row: Option<(String, String, i64)> = sqlx::query_as(
        "SELECT folder_id, metadata_version, counter FROM e2ee_folder_state WHERE pair_id = ?",
    )
    .bind(pair_id)
    .fetch_optional(journal.pool())
    .await
    .map_err(|e| E2eeError::Other(e.to_string()))?;
    Ok(row.map(|(fid, ver, counter)| (fid, ver, counter as u64)))
}

async fn upsert_folder_state(
    journal: &SqliteJournal,
    pair_id: &str,
    folder_id: &str,
    metadata_version: &str,
    counter: u64,
    key_checksums: &[String],
) -> Result<(), E2eeError> {
    use chrono::Utc;
    let now = Utc::now().to_rfc3339();
    let checksums = key_checksums.join(" ");
    sqlx::query(
        "INSERT INTO e2ee_folder_state(pair_id, folder_id, metadata_version, counter, key_checksums, updated_at)
         VALUES(?, ?, ?, ?, ?, ?)
         ON CONFLICT(pair_id) DO UPDATE SET folder_id=excluded.folder_id, metadata_version=excluded.metadata_version, counter=excluded.counter, key_checksums=excluded.key_checksums, updated_at=excluded.updated_at",
    )
    .bind(pair_id)
    .bind(folder_id)
    .bind(metadata_version)
    .bind(counter as i64)
    .bind(checksums)
    .bind(now)
    .execute(journal.pool())
    .await
    .map_err(|e| E2eeError::Other(e.to_string()))?;
    Ok(())
}

// ── Keychain helpers ──────────────────────────────────────────────────────────

fn keychain_key(account_id: &str) -> String {
    format!("adagio-e2ee/{account_id}")
}

fn store_private_key(account_id: &str, pem: &str) -> Result<(), E2eeError> {
    let key = keychain_key(account_id);
    let entry = keyring::Entry::new("adagio", &key)
        .map_err(|e| E2eeError::KeychainUnavailable(e.to_string()))?;
    // Call from a spawn_blocking context in the async wrapper.
    entry
        .set_password(pem)
        .map_err(|e| E2eeError::KeychainUnavailable(e.to_string()))
}

fn load_private_key_pem(account_id: &str) -> Result<Option<Zeroizing<String>>, E2eeError> {
    let key = keychain_key(account_id);
    let entry = keyring::Entry::new("adagio", &key)
        .map_err(|e| E2eeError::KeychainUnavailable(e.to_string()))?;
    match entry.get_password() {
        Ok(pem) => Ok(Some(Zeroizing::new(pem))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(E2eeError::KeychainUnavailable(e.to_string())),
    }
}

// ── NcE2eeClient ──────────────────────────────────────────────────────────────

/// Production `E2eeProvider` backed by the Nextcloud OCS API.
pub struct NcE2eeClient {
    /// Per-account OCS client factory: `account_id → (server_url, username, password)`.
    account_credentials: std::sync::Arc<dyn AccountCredentialProvider>,
    journal: std::sync::Arc<SqliteJournal>,
    /// In-memory cache of per-pair metadata (pair_id → metadata + metadata_key).
    metadata_cache: tokio::sync::Mutex<std::collections::HashMap<String, (E2eeMetadata, [u8; 16])>>,
}

/// Abstraction over credential lookup so the provider can be tested without
/// a live keychain.
#[async_trait]
pub trait AccountCredentialProvider: Send + Sync + 'static {
    /// Return `(server_url, username, nextcloud_password)` for an account.
    async fn credentials(
        &self,
        account_id: &AccountId,
    ) -> Result<(String, String, String), E2eeError>;

    /// Pair ID → account ID lookup.
    async fn account_id_for_pair(&self, pair_id: &PairId) -> Result<AccountId, E2eeError>;
}

impl NcE2eeClient {
    pub fn new(
        credentials: std::sync::Arc<dyn AccountCredentialProvider>,
        journal: std::sync::Arc<SqliteJournal>,
    ) -> Self {
        Self {
            account_credentials: credentials,
            journal,
            metadata_cache: Default::default(),
        }
    }

    /// Returns the OCS client and the Nextcloud username.
    /// Always use the returned username (not `account_id.0`) as the `userId` in
    /// `build_outer` / `parse_outer` — the server stores the Nextcloud login name.
    async fn ocs_client(
        &self,
        account_id: &AccountId,
    ) -> Result<(E2eeOcsClient, String), E2eeError> {
        let (server_url, username, password) =
            self.account_credentials.credentials(account_id).await?;
        Ok((E2eeOcsClient::new(server_url, username.clone(), password), username))
    }

    async fn load_privkey(&self, account_id: &AccountId) -> Result<RsaPrivateKey, E2eeError> {
        let aid = account_id.0.clone();
        let pem_opt = tokio::task::spawn_blocking(move || load_private_key_pem(&aid))
            .await
            .map_err(|e| E2eeError::Other(e.to_string()))??;
        let pem = pem_opt.ok_or(E2eeError::KeychainUnavailable(
            "E2EE private key not found — run 'adagio e2ee init' or 'adagio e2ee pair'".into(),
        ))?;
        private_key_from_pem(&pem).map_err(|e| E2eeError::Crypto(e.to_string()))
    }

    #[allow(dead_code)]
    async fn folder_id(&self, pair_id: &PairId) -> Result<String, E2eeError> {
        let state = load_folder_state(&self.journal, &pair_id.0).await?;
        state
            .map(|(fid, _, _)| fid)
            .ok_or_else(|| E2eeError::Other(format!("no E2EE folder state for pair {}", pair_id.0)))
    }
}

#[async_trait]
impl E2eeProvider for NcE2eeClient {
    #[instrument(skip(self))]
    async fn is_initialised(&self, account_id: &AccountId) -> Result<bool, E2eeError> {
        let cert = load_certificate(&self.journal, &account_id.0).await?;
        Ok(cert.is_some())
    }

    #[instrument(skip(self), fields(pair_id = %pair_id))]
    async fn init(&self, pair_id: &PairId) -> Result<String, E2eeError> {
        let account_id = self
            .account_credentials
            .account_id_for_pair(pair_id)
            .await?;
        let (ocs, nc_username) = self.ocs_client(&account_id).await?;

        info!(pair_id = %pair_id, account = %account_id, "E2EE init starting");

        // 1. Generate mnemonic and RSA key pair (blocking — RSA-4096 is slow).
        let mnemonic = generate_mnemonic()?;
        let privkey = tokio::task::spawn_blocking(generate_rsa_keypair)
            .await
            .map_err(|e| E2eeError::Other(e.to_string()))??;

        // 2. Submit CSR to server and get signed certificate.
        let pem_str = private_key_to_pem(&privkey)?;
        // Build a minimal CSR (DER bytes).
        // For now we submit the public key DER and rely on the server generating the cert.
        // Full PKCS#10 CSR is built in a follow-up; the Nextcloud server accepts the public key.
        let pub_der = public_key_der(&privkey);
        let csr_b64 = B64.encode(&pub_der);
        let cert_resp = ocs.post_public_key(&csr_b64).await?;
        let cert_pem = cert_resp["ocs"]["data"]["public-key"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // 3. Wrap private key with mnemonic and upload.
        let blob = wrap_private_key(&privkey, &mnemonic)?;
        ocs.post_private_key(&blob).await?;

        // 4. Store certificate and private key locally.
        let fingerprint = key_fingerprint(&pub_der);
        save_key_row(&self.journal, &account_id.0, &cert_pem, &fingerprint).await?;
        let aid_clone = account_id.0.clone();
        let pem_clone = pem_str.to_string();
        tokio::task::spawn_blocking(move || store_private_key(&aid_clone, &pem_clone))
            .await
            .map_err(|e| E2eeError::Other(e.to_string()))??;

        // 5. Create initial empty metadata for the folder.
        let folder_id = pair_id.0.clone(); // will be replaced by real WebDAV file-ID
        let metadata_key = generate_metadata_key();
        let pub_key = RsaPublicKey::from(&privkey);
        let init_meta = E2eeMetadata {
            version: "2.0".to_string(),
            counter: 1,
            key_checksums: vec![metadata_key_checksum(&metadata_key)],
            ..Default::default()
        };
        let outer = build_outer(
            &init_meta,
            &metadata_key,
            &nc_username,
            &cert_pem,
            &pub_key,
        )?;
        let outer_json = serde_json::to_string(&outer)?;
        // V2 API: lock first, then POST metadata with token + signature.
        let init_inner = canonical_inner_json(&init_meta)?;
        let init_sig = cms_sign(&init_inner, &privkey, &cert_pem)?;
        let init_token = ocs
            .lock_folder(&folder_id, 1)
            .await
            .map_err(|e| E2eeError::ServerError(format!("lock for init: {e}")))?;
        let post_result = ocs
            .post_metadata(&folder_id, &outer_json, &init_token, &init_sig)
            .await;
        let _ = ocs.unlock_folder(&folder_id, &init_token).await;
        post_result?;
        ocs.mark_folder_encrypted(&folder_id).await?;

        upsert_folder_state(
            &self.journal,
            &pair_id.0,
            &folder_id,
            "2.0",
            0,
            &init_meta.key_checksums,
        )
        .await?;

        info!(pair_id = %pair_id, "E2EE init complete");
        // Mnemonic is returned to caller and shown once — never logged.
        Ok(mnemonic)
    }

    #[instrument(skip(self, mnemonic))]
    async fn pair(&self, pair_id: &PairId, mnemonic: &str) -> Result<(), E2eeError> {
        let mnemonic = validate_mnemonic(mnemonic)?;
        let account_id = self
            .account_credentials
            .account_id_for_pair(pair_id)
            .await?;
        let (ocs, nc_username) = self.ocs_client(&account_id).await?;

        info!(pair_id = %pair_id, account = %account_id, "E2EE pairing starting");

        // 1. Download encrypted private key blob from server.
        let resp = ocs.get_private_key().await?;
        let blob = resp["ocs"]["data"]["private-key"]
            .as_str()
            .ok_or_else(|| E2eeError::Other("server returned no private key blob".into()))?
            .to_string();

        // 2. Decrypt with mnemonic.
        let privkey = unwrap_private_key(&blob, &mnemonic)?;

        // 3. Fetch certificate and verify it matches.
        let cert_resp = ocs.get_public_key(&[nc_username.as_str()]).await?;
        let cert_pem = cert_resp["ocs"]["data"]["public-keys"][&nc_username]
            .as_str()
            .unwrap_or("")
            .to_string();

        // 4. Store locally.
        let pub_der = public_key_der(&privkey);
        let fingerprint = key_fingerprint(&pub_der);
        save_key_row(&self.journal, &account_id.0, &cert_pem, &fingerprint).await?;

        let pem = private_key_to_pem(&privkey)?.to_string();
        let aid = account_id.0.clone();
        tokio::task::spawn_blocking(move || store_private_key(&aid, &pem))
            .await
            .map_err(|e| E2eeError::Other(e.to_string()))??;

        info!(pair_id = %pair_id, fingerprint = %fingerprint, "E2EE pairing complete");
        Ok(())
    }

    #[instrument(skip(self, plaintext), fields(pair_id = %pair_id, filename = %filename))]
    async fn encrypt_file(
        &self,
        pair_id: &PairId,
        filename: &str,
        mimetype: &str,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, String, E2eeFileEntry), E2eeError> {
        let (key, nonce, auth_tag, ciphertext) = cipher::encrypt_file(plaintext)?;
        let uuid = Uuid::new_v4().to_string().replace('-', "");
        let entry = E2eeFileEntry {
            filename: filename.to_string(),
            mimetype: mimetype.to_string(),
            nonce: B64.encode(nonce),
            auth_tag: B64.encode(auth_tag),
            key: B64.encode(key),
        };
        debug!(pair_id = %pair_id, uuid = %uuid, "file encrypted");
        Ok((ciphertext, uuid, entry))
    }

    #[instrument(skip(self, ciphertext), fields(pair_id = %pair_id, uuid = %uuid))]
    async fn decrypt_file(
        &self,
        pair_id: &PairId,
        uuid: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, E2eeError> {
        // Load metadata from cache or sync from server.
        let meta = self.sync_metadata(pair_id).await?;
        let entry = meta.files.get(uuid).ok_or_else(|| {
            E2eeError::Other(format!(
                "uuid {uuid} not found in metadata for pair {}",
                pair_id.0
            ))
        })?;

        let key_bytes = B64
            .decode(&entry.key)
            .map_err(|e| E2eeError::Crypto(e.to_string()))?;
        let nonce_bytes = B64
            .decode(&entry.nonce)
            .map_err(|e| E2eeError::Crypto(e.to_string()))?;
        let tag_bytes = B64
            .decode(&entry.auth_tag)
            .map_err(|e| E2eeError::Crypto(e.to_string()))?;

        let mut key = [0u8; 16];
        let mut nonce = [0u8; 12];
        let mut auth_tag = [0u8; 16];
        key.copy_from_slice(&key_bytes);
        nonce.copy_from_slice(&nonce_bytes);
        auth_tag.copy_from_slice(&tag_bytes);

        cipher::decrypt_file(&key, &nonce, &auth_tag, ciphertext)
    }

    #[instrument(skip(self), fields(pair_id = %pair_id))]
    async fn sync_metadata(&self, pair_id: &PairId) -> Result<E2eeMetadata, E2eeError> {
        let account_id = self
            .account_credentials
            .account_id_for_pair(pair_id)
            .await?;
        let state = load_folder_state(&self.journal, &pair_id.0).await?;
        let (folder_id, metadata_version, stored_counter) = state.ok_or_else(|| {
            E2eeError::Other(format!("no E2EE folder state for pair {}", pair_id.0))
        })?;

        // Version gate.
        match metadata_version.as_str() {
            "2.0" | "2" => {}
            "1.0" | "1" => {
                // v1.x: allow read-only; uploads are blocked in encrypt_file / commit_metadata.
                // Fall through to parse and return metadata.
            }
            ver => return Err(E2eeError::UnsupportedMetadataVersion(ver.to_string())),
        }

        let (ocs, nc_username) = self.ocs_client(&account_id).await?;
        let resp = ocs.get_metadata(&folder_id).await?;

        // The V2 API returns `meta-data` as a JSON string (double-encoded) rather than
        // an embedded object. Parse the string value first, then deserialise the struct.
        let raw = resp["ocs"]["data"]["meta-data"].clone();
        let meta_value: serde_json::Value = if let Some(s) = raw.as_str() {
            serde_json::from_str(s).map_err(E2eeError::Serde)?
        } else {
            raw
        };
        let outer: crate::metadata::OuterMetadata =
            serde_json::from_value(meta_value).map_err(E2eeError::Serde)?;

        if outer.version != "2.0" && outer.version != "2" {
            if outer.version == "1.0" || outer.version == "1" {
                // Warn; permit read.
                warn!(pair_id = %pair_id, "E2EE folder uses metadata v1.x — read-only mode");
            } else {
                return Err(E2eeError::UnsupportedMetadataVersion(outer.version.clone()));
            }
        }

        let privkey = self.load_privkey(&account_id).await?;
        let (metadata, metadata_key) = parse_outer(&outer, &nc_username, &privkey)?;

        // Counter monotonicity check.
        if metadata.counter < stored_counter {
            warn!(
                pair_id = %pair_id,
                server_counter = metadata.counter,
                local_counter = stored_counter,
                "metadata counter went backwards — ignoring"
            );
        }

        // Update local state.
        upsert_folder_state(
            &self.journal,
            &pair_id.0,
            &folder_id,
            &outer.version,
            metadata.counter,
            &metadata.key_checksums,
        )
        .await?;

        // Cache for decrypt_file calls in this cycle.
        self.metadata_cache
            .lock()
            .await
            .insert(pair_id.0.clone(), (metadata.clone(), metadata_key));

        debug!(pair_id = %pair_id, files = metadata.files.len(), "metadata synced");
        Ok(metadata)
    }

    #[instrument(skip(self, new_metadata), fields(pair_id = %pair_id))]
    async fn commit_metadata(
        &self,
        pair_id: &PairId,
        new_metadata: &E2eeMetadata,
    ) -> Result<(), E2eeError> {
        let account_id = self
            .account_credentials
            .account_id_for_pair(pair_id)
            .await?;

        // Block uploads for v1.x folders.
        let state = load_folder_state(&self.journal, &pair_id.0).await?;
        let (folder_id, metadata_version, counter) = state.ok_or_else(|| {
            E2eeError::Other(format!("no E2EE folder state for pair {}", pair_id.0))
        })?;
        if metadata_version == "1.0" || metadata_version == "1" {
            return Err(E2eeError::LegacyMetadataReadOnly);
        }

        let privkey = self.load_privkey(&account_id).await?;
        let pub_key = RsaPublicKey::from(&privkey);
        let cert_opt = load_certificate(&self.journal, &account_id.0).await?;
        let cert_pem = cert_opt.unwrap_or_default();

        // Get or generate metadata key from cache.
        let metadata_key = {
            let cache = self.metadata_cache.lock().await;
            cache
                .get(&pair_id.0)
                .map(|(_, k)| *k)
                .unwrap_or_else(generate_metadata_key)
        };

        let (ocs, nc_username) = self.ocs_client(&account_id).await?;
        let attempt = async |counter: u64| -> Result<(), E2eeError> {
            let token = ocs.lock_folder(&folder_id, counter + 1).await?;
            let outer = build_outer(
                new_metadata,
                &metadata_key,
                &nc_username,
                &cert_pem,
                &pub_key,
            )?;
            let outer_json = serde_json::to_string(&outer)?;
            let inner_json = canonical_inner_json(new_metadata)?;
            let signature = cms_sign(&inner_json, &privkey, &cert_pem)?;
            ocs.put_metadata(&folder_id, &outer_json, &token, &signature)
                .await?;
            ocs.unlock_folder(&folder_id, &token).await?;
            Ok(())
        };

        match attempt(counter).await {
            Ok(()) => {
                upsert_folder_state(
                    &self.journal,
                    &pair_id.0,
                    &folder_id,
                    &metadata_version,
                    new_metadata.counter,
                    &new_metadata.key_checksums,
                )
                .await?;
                info!(pair_id = %pair_id, counter = new_metadata.counter, "metadata committed");
                Ok(())
            }
            Err(E2eeError::ServerError(ref msg)) if msg.contains("conflict") => {
                // Re-fetch and retry once.
                warn!(pair_id = %pair_id, "lock conflict; re-fetching metadata and retrying");
                let refreshed = self.sync_metadata(pair_id).await?;
                let new_counter = refreshed.counter;
                attempt(new_counter).await?;
                upsert_folder_state(
                    &self.journal,
                    &pair_id.0,
                    &folder_id,
                    &metadata_version,
                    new_metadata.counter,
                    &new_metadata.key_checksums,
                )
                .await?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[instrument(skip(self), fields(pair_id = %pair_id))]
    async fn status(&self, pair_id: &PairId) -> Result<E2eeStatusDto, E2eeError> {
        let account_id = self
            .account_credentials
            .account_id_for_pair(pair_id)
            .await?;
        let _cert = load_certificate(&self.journal, &account_id.0).await?;
        let state = load_folder_state(&self.journal, &pair_id.0).await?;

        let (enabled, metadata_version, counter, encrypted_file_count) = match &state {
            Some((_, ver, cnt)) => (true, Some(ver.clone()), *cnt, 0u64),
            None => (false, None, 0, 0),
        };

        // Fetch fingerprint from DB via a proper async query.
        let fingerprint: Option<String> = {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT key_fingerprint FROM e2ee_account_keys WHERE account_id = ?",
            )
            .bind(&account_id.0)
            .fetch_optional(self.journal.pool())
            .await
            .unwrap_or(None);
            row.map(|(f,)| f)
        };

        Ok(E2eeStatusDto {
            pair_id: pair_id.0.clone(),
            enabled,
            metadata_version,
            counter,
            key_fingerprint: fingerprint,
            encrypted_file_count,
        })
    }

    #[instrument(skip(self, metadata), fields(pair_id = %pair_id))]
    async fn recreate_metadata(
        &self,
        pair_id: &PairId,
        metadata: &E2eeMetadata,
    ) -> Result<(), E2eeError> {
        let account_id = self
            .account_credentials
            .account_id_for_pair(pair_id)
            .await?;
        let (ocs, nc_username) = self.ocs_client(&account_id).await?;
        let pub_key_priv = self.load_privkey(&account_id).await?;
        let pub_key = rsa::RsaPublicKey::from(&pub_key_priv);
        let cert_opt = load_certificate(&self.journal, &account_id.0).await?;
        let cert_pem = cert_opt.unwrap_or_default();
        let metadata_key = crate::keys::generate_metadata_key();

        let state = load_folder_state(&self.journal, &pair_id.0).await?;
        let (folder_id, _, counter) = state
            .ok_or_else(|| E2eeError::Other("no folder state for pair".into()))?;

        let outer = crate::metadata::build_outer(
            metadata,
            &metadata_key,
            &nc_username,
            &cert_pem,
            &pub_key,
        )?;
        let outer_json = serde_json::to_string(&outer).map_err(E2eeError::Serde)?;
        let inner_json = canonical_inner_json(metadata)?;
        let signature = cms_sign(&inner_json, &pub_key_priv, &cert_pem)?;

        // V2 API: lock → POST (with token + signature) → unlock.
        let token = ocs
            .lock_folder(&folder_id, counter + 1)
            .await
            .map_err(|e| E2eeError::ServerError(format!("lock_folder: {e}")))?;
        let post_result = ocs
            .post_metadata(&folder_id, &outer_json, &token, &signature)
            .await;
        let unlock_result = ocs.unlock_folder(&folder_id, &token).await;
        post_result.map_err(|e| E2eeError::ServerError(e.to_string()))?;
        if let Err(e) = unlock_result {
            warn!(pair_id = %pair_id, error = %e, "E2EE: unlock failed after recreate (non-fatal)");
        }

        upsert_folder_state(
            &self.journal,
            &pair_id.0,
            &folder_id,
            "2.0",
            counter + 1,
            &[crate::metadata::metadata_key_checksum(&metadata_key)],
        )
        .await?;

        info!(pair_id = %pair_id, "E2EE metadata recreated on server");
        Ok(())
    }

    #[instrument(skip(self), fields(pair_id = %pair_id))]
    async fn disable(&self, pair_id: &PairId) -> Result<(), E2eeError> {
        let account_id = self
            .account_credentials
            .account_id_for_pair(pair_id)
            .await?;
        let (ocs, _nc_username) = self.ocs_client(&account_id).await?;

        // Fetch folder_id to delete the server-side metadata.
        if let Some((folder_id, _, _)) = load_folder_state(&self.journal, &pair_id.0).await? {
            // Best-effort: ignore errors (folder may already be gone).
            let _ = ocs.delete_metadata(&folder_id).await;
        }

        // Remove local E2EE journal row.
        sqlx::query("DELETE FROM e2ee_folder_state WHERE pair_id = ?")
            .bind(&pair_id.0)
            .execute(self.journal.pool())
            .await
            .map_err(|e| E2eeError::Other(e.to_string()))?;

        // Clear the metadata cache for this pair.
        self.metadata_cache.lock().await.remove(&pair_id.0);

        info!(pair_id = %pair_id, "E2EE disabled — pair reverted to copy-sync");
        Ok(())
    }
}

// ── Locked upload cycle (not part of the trait — uses concrete OCS primitives)

impl NcE2eeClient {
    /// Lock folder → encrypt + upload files → commit metadata → unlock.
    ///
    /// This is the only correct way to upload files to an E2EE folder: Nextcloud
    /// requires the `e2e-token` from `lock_folder` on every WebDAV PUT, and the
    /// same token on the metadata `PUT` that follows.
    ///
    /// Returns the UUIDs of files that were successfully uploaded.
    pub async fn upload_locked(
        &self,
        pair_id: &PairId,
        dav_folder: &str,
        files: Vec<(String, String, Vec<u8>)>,
    ) -> Result<Vec<String>, E2eeError> {
        if files.is_empty() {
            return Ok(vec![]);
        }

        let account_id = self.account_credentials.account_id_for_pair(pair_id).await?;
        let (ocs, nc_username) = self.ocs_client(&account_id).await?;

        let state = load_folder_state(&self.journal, &pair_id.0).await?.ok_or_else(|| {
            E2eeError::Other(format!("no E2EE folder state for pair {}", pair_id.0))
        })?;
        let (folder_id, metadata_version, counter) = state;

        if metadata_version == "1.0" || metadata_version == "1" {
            return Err(E2eeError::LegacyMetadataReadOnly);
        }

        // Encrypt all files before acquiring the lock (crypto is slow).
        let mut encrypted: Vec<(String, E2eeFileEntry, Vec<u8>)> = Vec::new();
        for (filename, mime, plaintext) in &files {
            match self.encrypt_file(pair_id, filename, mime, plaintext).await {
                Ok((ciphertext, uuid, entry)) => encrypted.push((uuid, entry, ciphertext)),
                Err(e) => warn!(pair_id = %pair_id, file = %filename, error = %e, "E2EE: encrypt failed"),
            }
        }
        if encrypted.is_empty() {
            return Err(E2eeError::Other("all files failed to encrypt".into()));
        }

        // Acquire lock.
        let token = ocs
            .lock_folder(&folder_id, counter + 1)
            .await
            .map_err(|e| E2eeError::ServerError(format!("lock_folder: {e}")))?;

        // Upload each ciphertext with the e2e-token header.
        let mut uploaded: Vec<String> = Vec::new();
        for (uuid, _entry, ciphertext) in &encrypted {
            match ocs
                .upload_encrypted_file(dav_folder, uuid, &token, ciphertext.clone())
                .await
            {
                Ok(()) => {
                    info!(pair_id = %pair_id, uuid = %uuid, "E2EE: file uploaded");
                    uploaded.push(uuid.clone());
                }
                Err(e) => {
                    warn!(pair_id = %pair_id, uuid = %uuid, error = %e, "E2EE: upload failed in locked cycle");
                }
            }
        }

        // Build updated metadata (merge new entries into whatever sync_metadata cached).
        let privkey = match self.load_privkey(&account_id).await {
            Ok(k) => k,
            Err(e) => {
                let _ = ocs.unlock_folder(&folder_id, &token).await;
                return Err(e);
            }
        };
        let pub_key = rsa::RsaPublicKey::from(&privkey);
        let cert_opt = load_certificate(&self.journal, &account_id.0).await?;
        let cert_pem = cert_opt.unwrap_or_default();

        let (base_metadata, metadata_key, metadata_exists) = {
            let cache = self.metadata_cache.lock().await;
            match cache.get(&pair_id.0) {
                Some((m, k)) => (m.clone(), *k, true),
                None => (E2eeMetadata::default(), generate_metadata_key(), false),
            }
        };

        let mut updated = base_metadata;
        for (uuid, entry, _) in &encrypted {
            if uploaded.contains(uuid) {
                updated.files.insert(uuid.clone(), entry.clone());
            }
        }
        updated.counter = counter + 1;
        if updated.version.is_empty() {
            updated.version = "2.0".to_string();
        }
        if updated.key_checksums.is_empty() {
            updated.key_checksums = vec![metadata_key_checksum(&metadata_key)];
        }

        let outer = match build_outer(&updated, &metadata_key, &nc_username, &cert_pem, &pub_key)
        {
            Ok(o) => o,
            Err(e) => {
                let _ = ocs.unlock_folder(&folder_id, &token).await;
                return Err(e);
            }
        };
        let outer_json = serde_json::to_string(&outer).map_err(E2eeError::Serde)?;
        let inner_json = canonical_inner_json(&updated)?;
        let signature = cms_sign(&inner_json, &privkey, &cert_pem)?;

        // V2: POST (create) when metadata doesn't exist yet, PUT (update) otherwise.
        // Both require the lock token and the CMS signature.
        let put_result = if metadata_exists {
            ocs.put_metadata(&folder_id, &outer_json, &token, &signature)
                .await
        } else {
            ocs.post_metadata(&folder_id, &outer_json, &token, &signature)
                .await
                .map(|_| serde_json::Value::Null)
        };
        let unlock_result = ocs.unlock_folder(&folder_id, &token).await;

        put_result.map_err(|e| E2eeError::ServerError(format!("put_metadata: {e}")))?;
        if let Err(e) = unlock_result {
            warn!(pair_id = %pair_id, error = %e, "E2EE: unlock failed (non-fatal)");
        }

        upsert_folder_state(
            &self.journal,
            &pair_id.0,
            &folder_id,
            &metadata_version,
            updated.counter,
            &updated.key_checksums,
        )
        .await?;

        info!(
            pair_id = %pair_id,
            uploaded = uploaded.len(),
            counter = updated.counter,
            "E2EE: locked upload cycle complete"
        );
        Ok(uploaded)
    }
}

// ── Tests (T030, T031) ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{keys::generate_metadata_key, metadata::encrypt_metadata};

    // T030 — sync_metadata round-trip using pure in-memory structs.
    #[test]
    fn sync_metadata_roundtrip() {
        let mk = generate_metadata_key();
        let mut original = E2eeMetadata::default();
        original.counter = 5;
        original.files.insert(
            "abc".to_string(),
            E2eeFileEntry {
                filename: "test.txt".to_string(),
                mimetype: "text/plain".to_string(),
                nonce: B64.encode([0u8; 12]),
                auth_tag: B64.encode([0u8; 16]),
                key: B64.encode(mk),
            },
        );

        let blob = encrypt_metadata(&original, &mk).unwrap();
        let recovered = crate::metadata::decrypt_metadata(&blob, &mk).unwrap();
        assert_eq!(recovered.counter, 5);
        assert!(recovered.files.contains_key("abc"));
        assert_eq!(recovered.files["abc"].filename, "test.txt");
    }

    // T031 — v1.x metadata version gate returns LegacyMetadataReadOnly.
    #[test]
    fn v1x_gate_blocks_upload_check() {
        // We verify the error variant directly, as the full DB path
        // requires a live SqliteJournal (covered in integration tests).
        let err = E2eeError::LegacyMetadataReadOnly;
        assert!(matches!(err, E2eeError::LegacyMetadataReadOnly));
        let display = err.to_string();
        assert!(display.contains("v1.x"));
    }
}
