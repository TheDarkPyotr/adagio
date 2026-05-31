/// Nextcloud E2EE OCS API client — v2.x endpoint wrappers.
///
/// All methods make a single authenticated HTTP call and return the raw JSON
/// response body. Higher-level logic (key parsing, metadata decryption) lives
/// in `adagio-e2ee`.
///
/// Base URL: `{server}/ocs/v2.php/apps/end_to_end_encryption/api/v2/`
use adagio_core::error::ClientError;
use reqwest::{header, Client, StatusCode};
use serde_json::Value;
use tracing::{debug, instrument};

/// OCS API version used for key-management endpoints.
const OCS_API_V2: &str = "v2";

/// OCS API version used for metadata and lock endpoints.
/// The V1 endpoint rejects V2-format metadata ("Use the v2 endpoint for this file").
const METADATA_API_V: &str = "v2";

fn map_status(status: StatusCode, url: &str) -> ClientError {
    match status.as_u16() {
        401 | 403 => ClientError::AuthRequired,
        404 => ClientError::Permanent(format!("not found: {url}")),
        409 => ClientError::Transient(format!("conflict (lock counter mismatch): {url}")),
        503 => ClientError::Maintenance,
        s if s >= 500 => ClientError::Transient(format!("server error {s}: {url}")),
        s => ClientError::Permanent(format!("HTTP {s}: {url}")),
    }
}

/// Thin HTTP wrapper for the Nextcloud E2EE OCS API.
pub struct E2eeOcsClient {
    http: Client,
    server_url: String,
    username: String,
    password: String,
}

impl E2eeOcsClient {
    /// Create a new client.
    pub fn new(
        server_url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        // Nextcloud's E2EE middleware only permits requests from known clients.
        // The official Desktop client includes "mirall/" in its user agent, which
        // is the string the Nextcloud E2EE app's ClientCheckMiddleware matches
        // against. Using this prefix allows adagio to access E2EE endpoints.
        let http = Client::builder()
            .user_agent("Mozilla/5.0 (Linux) mirall/3.12.0 (adagio; linux-openssl)")
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            server_url: server_url.into().trim_end_matches('/').to_string(),
            username: username.into(),
            password: password.into(),
        }
    }

    fn key_url(&self, path: &str) -> String {
        format!(
            "{}/ocs/v2.php/apps/end_to_end_encryption/api/{}/{}",
            self.server_url, OCS_API_V2, path
        )
    }

    fn meta_url(&self, path: &str) -> String {
        format!(
            "{}/ocs/v2.php/apps/end_to_end_encryption/api/{}/{}",
            self.server_url, METADATA_API_V, path
        )
    }

    /// Extract the Bearer token if the stored credential is an OAuth2 JSON blob
    /// (`{"access_token":"…"}`), otherwise return `None` and fall back to Basic auth.
    fn bearer_token(&self) -> Option<String> {
        if self.password.starts_with('{') {
            serde_json::from_str::<serde_json::Value>(&self.password)
                .ok()?
                .get("access_token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Attach Nextcloud authentication to a request builder.
    ///
    /// Prefers `Authorization: Bearer {token}` when the stored credential is
    /// an OAuth2 JSON blob — the OCS API requires it. Falls back to Basic auth
    /// for app-password credentials (plain strings).
    fn with_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = self.bearer_token() {
            debug!(
                auth_method = "bearer",
                "E2EE OCS: using Bearer token (OAuth2 credential detected)"
            );
            req.header(header::AUTHORIZATION, format!("Bearer {token}"))
        } else {
            debug!(
                auth_method = "basic",
                "E2EE OCS: using Basic auth (app-password credential)"
            );
            req.basic_auth(&self.username, Some(&self.password))
        }
    }

    /// Log the response body and return a rich error when the HTTP status is not success.
    async fn map_error_response(
        status: reqwest::StatusCode,
        url: &str,
        resp: reqwest::Response,
    ) -> ClientError {
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(
            status = status.as_u16(),
            url = %url,
            body = %body,
            "E2EE OCS request failed"
        );
        match status.as_u16() {
            401 => ClientError::Permanent(format!(
                "HTTP 401 Unauthorized on {url} — credentials invalid or token expired. Body: {body}"
            )),
            403 => ClientError::Permanent(format!(
                "HTTP 403 Forbidden on {url} — the account may lack E2EE permissions, \
                 or the Nextcloud E2EE app requires accepting terms in the web UI first. Body: {body}"
            )),
            _ => map_status(status, url),
        }
    }

    async fn ocs_get(&self, url: &str) -> Result<Value, ClientError> {
        debug!(url = %url, "E2EE OCS GET");
        let resp = self
            .with_auth(self.http.get(url))
            .header(header::ACCEPT, "application/json")
            .header("OCS-APIREQUEST", "true")
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::map_error_response(status, url, resp).await);
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))
    }

    async fn ocs_post(&self, url: &str, form: &[(&str, &str)]) -> Result<Value, ClientError> {
        debug!(url = %url, "E2EE OCS POST");
        let resp = self
            .with_auth(self.http.post(url))
            .header(header::ACCEPT, "application/json")
            .header("OCS-APIREQUEST", "true")
            .form(form)
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::map_error_response(status, url, resp).await);
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))
    }

    async fn ocs_put(
        &self,
        url: &str,
        form: &[(&str, &str)],
        extra_headers: &[(&str, &str)],
    ) -> Result<Value, ClientError> {
        debug!(url = %url, "E2EE OCS PUT");
        let mut req = self
            .with_auth(self.http.put(url))
            .header(header::ACCEPT, "application/json")
            .header("OCS-APIREQUEST", "true")
            .form(form);
        for (k, v) in extra_headers {
            req = req.header(*k, *v);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::map_error_response(status, url, resp).await);
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))
    }

    async fn ocs_delete(
        &self,
        url: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<(), ClientError> {
        debug!(url = %url, "E2EE OCS DELETE");
        let mut req = self
            .with_auth(self.http.delete(url))
            .header("OCS-APIREQUEST", "true");
        for (k, v) in extra_headers {
            req = req.header(*k, *v);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::map_error_response(status, url, resp).await);
        }
        Ok(())
    }

    // ── Key management ────────────────────────────────────────────────────────

    /// `GET …/server-key` — Nextcloud CA public key.
    #[instrument(skip(self))]
    pub async fn get_server_key(&self) -> Result<Value, ClientError> {
        self.ocs_get(&self.key_url("server-key")).await
    }

    /// `GET …/public-key?users[]=userId` — retrieve signed certificate(s).
    #[instrument(skip(self))]
    pub async fn get_public_key(&self, user_ids: &[&str]) -> Result<Value, ClientError> {
        let query: Vec<String> = user_ids
            .iter()
            .map(|u| format!("users[]={}", urlencoding::encode(u)))
            .collect();
        let url = format!("{}&{}", self.key_url("public-key"), query.join("&"));
        self.ocs_get(&url).await
    }

    /// `POST …/public-key` — submit a PKCS#10 CSR; server returns signed cert.
    #[instrument(skip(self, csr_pem))]
    pub async fn post_public_key(&self, csr_pem: &str) -> Result<Value, ClientError> {
        self.ocs_post(&self.key_url("public-key"), &[("csr", csr_pem)])
            .await
    }

    /// `PUT …/public-key` — replace existing public key certificate.
    #[instrument(skip(self, cert_pem))]
    pub async fn put_public_key(&self, cert_pem: &str) -> Result<Value, ClientError> {
        self.ocs_put(
            &self.key_url("public-key"),
            &[("certificate", cert_pem)],
            &[],
        )
        .await
    }

    /// `DELETE …/public-key` — remove public key from server.
    #[instrument(skip(self))]
    pub async fn delete_public_key(&self) -> Result<(), ClientError> {
        self.ocs_delete(&self.key_url("public-key"), &[]).await
    }

    /// `GET …/private-key` — download AES-GCM-encrypted private key blob.
    #[instrument(skip(self))]
    pub async fn get_private_key(&self) -> Result<Value, ClientError> {
        self.ocs_get(&self.key_url("private-key")).await
    }

    /// `POST …/private-key` — upload AES-GCM-encrypted private key blob.
    ///
    /// `encrypted_private_key` format: `{base64(ciphertext)}|{base64(nonce)}|{base64(salt)}`
    #[instrument(skip(self, encrypted_private_key))]
    pub async fn post_private_key(
        &self,
        encrypted_private_key: &str,
    ) -> Result<Value, ClientError> {
        self.ocs_post(
            &self.key_url("private-key"),
            &[("privateKey", encrypted_private_key)],
        )
        .await
    }

    /// `DELETE …/private-key` — remove private key from server.
    #[instrument(skip(self))]
    pub async fn delete_private_key(&self) -> Result<(), ClientError> {
        self.ocs_delete(&self.key_url("private-key"), &[]).await
    }

    // ── Metadata ──────────────────────────────────────────────────────────────

    /// `GET …/meta-data/{file_id}` — fetch encrypted metadata envelope.
    #[instrument(skip(self))]
    pub async fn get_metadata(&self, file_id: &str) -> Result<Value, ClientError> {
        self.ocs_get(&self.meta_url(&format!("meta-data/{file_id}")))
            .await
    }

    /// `POST …/meta-data/{file_id}` — create initial metadata for a new E2EE folder.
    ///
    /// The V2 API requires both a lock token (`e2e_token`) and a CMS signature
    /// (`signature`) — same contract as `put_metadata`.
    #[instrument(skip(self, metadata_json, e2e_token, signature))]
    pub async fn post_metadata(
        &self,
        file_id: &str,
        metadata_json: &str,
        e2e_token: &str,
        signature: &str,
    ) -> Result<Value, ClientError> {
        let url = self.meta_url(&format!("meta-data/{file_id}"));
        debug!(url = %url, "E2EE OCS POST metadata");
        let resp = self
            .with_auth(self.http.post(&url))
            .header(header::ACCEPT, "application/json")
            .header("OCS-APIREQUEST", "true")
            .header("e2e-token", e2e_token)
            .header("X-NC-E2EE-SIGNATURE", signature)
            .form(&[("metaData", metadata_json)])
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::map_error_response(status, &url, resp).await);
        }
        resp.json()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))
    }

    /// `PUT …/meta-data/{file_id}` — update metadata.
    ///
    /// Requires the `e2e-token` returned by `lock_folder` and the CMS signature.
    #[instrument(skip(self, metadata_json, e2e_token, signature))]
    pub async fn put_metadata(
        &self,
        file_id: &str,
        metadata_json: &str,
        e2e_token: &str,
        signature: &str,
    ) -> Result<Value, ClientError> {
        self.ocs_put(
            &self.meta_url(&format!("meta-data/{file_id}")),
            &[("metaData", metadata_json)],
            &[("e2e-token", e2e_token), ("X-NC-E2EE-SIGNATURE", signature)],
        )
        .await
    }

    /// `DELETE …/meta-data/{file_id}` — delete folder metadata (disables E2EE).
    #[instrument(skip(self))]
    pub async fn delete_metadata(&self, file_id: &str) -> Result<(), ClientError> {
        self.ocs_delete(&self.meta_url(&format!("meta-data/{file_id}")), &[])
            .await
    }

    // ── Lock / unlock ─────────────────────────────────────────────────────────

    /// `POST …/lock/{file_id}` — acquire exclusive write lock.
    ///
    /// Returns the `e2e-token` needed for subsequent metadata writes and unlock.
    /// `counter` must be exactly `current_counter + 1`.
    ///
    /// The V2 endpoint requires `X-NC-E2EE-COUNTER` as an HTTP **header**, not a
    /// form field, so this method builds the request directly.
    #[instrument(skip(self))]
    pub async fn lock_folder(&self, file_id: &str, counter: u64) -> Result<String, ClientError> {
        let url = self.meta_url(&format!("lock/{file_id}"));
        debug!(url = %url, counter = counter, "E2EE OCS POST lock");
        let resp = self
            .with_auth(self.http.post(&url))
            .header(header::ACCEPT, "application/json")
            .header("OCS-APIREQUEST", "true")
            .header("X-NC-E2EE-COUNTER", counter.to_string())
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::map_error_response(status, &url, resp).await);
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let token = body["ocs"]["data"]["e2e-token"]
            .as_str()
            .ok_or_else(|| ClientError::Permanent("lock response missing e2e-token".into()))?
            .to_string();
        Ok(token)
    }

    /// `DELETE …/lock/{file_id}` — release the exclusive write lock.
    #[instrument(skip(self, e2e_token))]
    pub async fn unlock_folder(&self, file_id: &str, e2e_token: &str) -> Result<(), ClientError> {
        self.ocs_delete(
            &self.meta_url(&format!("lock/{file_id}")),
            &[("e2e-token", e2e_token)],
        )
        .await
    }

    /// `PUT …/encrypted/{file_id}` — mark folder as E2EE-encrypted on the server.
    #[instrument(skip(self))]
    pub async fn mark_folder_encrypted(&self, file_id: &str) -> Result<(), ClientError> {
        self.ocs_put(&self.key_url(&format!("encrypted/{file_id}")), &[], &[])
            .await
            .map(|_| ())
    }

    // ── WebDAV (E2EE-locked upload) ───────────────────────────────────────────

    /// WebDAV PUT — upload ciphertext to an E2EE-locked folder.
    ///
    /// Must be called while the folder is locked (between `lock_folder` and
    /// `unlock_folder`). The `e2e_token` is included as a header; Nextcloud
    /// rejects PUT requests to E2EE-marked folders without it.
    ///
    /// `folder_dav_path` is the remote path relative to the user's WebDAV root,
    /// e.g. `"adagio-e2ee2"` — no leading slash.
    #[instrument(skip(self, data), fields(uuid = %uuid))]
    pub async fn upload_encrypted_file(
        &self,
        folder_dav_path: &str,
        uuid: &str,
        e2e_token: &str,
        data: Vec<u8>,
    ) -> Result<(), ClientError> {
        let url = format!(
            "{}/remote.php/dav/files/{}/{}/{}",
            self.server_url,
            urlencoding::encode(&self.username),
            folder_dav_path.trim_start_matches('/'),
            uuid,
        );
        debug!(url = %url, "E2EE WebDAV PUT encrypted file");
        let resp = self
            .with_auth(
                self.http
                    .put(&url)
                    .header("e2e-token", e2e_token)
                    .body(data),
            )
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Self::map_error_response(status, &url, resp).await);
        }
        Ok(())
    }
}

// ── Tests (T013) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn client(server_url: &str) -> E2eeOcsClient {
        E2eeOcsClient::new(server_url, "alice", "app-password")
    }

    // T013-a: get_server_key returns the response JSON.
    #[tokio::test]
    async fn get_server_key_returns_json() {
        let mut srv = mockito::Server::new_async().await;
        let _m = srv
            .mock(
                "GET",
                "/ocs/v2.php/apps/end_to_end_encryption/api/v2/server-key",
            )
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(r#"{"ocs":{"data":{"public-key":"PEM..."}}}"#)
            .create_async()
            .await;

        let resp = client(&srv.url()).get_server_key().await.unwrap();
        assert_eq!(resp["ocs"]["data"]["public-key"], "PEM...");
    }

    // T013-b: get_private_key returns the encrypted private key blob.
    #[tokio::test]
    async fn get_private_key_returns_blob() {
        let mut srv = mockito::Server::new_async().await;
        let _m = srv
            .mock(
                "GET",
                "/ocs/v2.php/apps/end_to_end_encryption/api/v2/private-key",
            )
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(r#"{"ocs":{"data":{"private-key":"ciphertext|nonce|salt"}}}"#)
            .create_async()
            .await;

        let resp = client(&srv.url()).get_private_key().await.unwrap();
        assert_eq!(resp["ocs"]["data"]["private-key"], "ciphertext|nonce|salt");
    }

    // T013-c: lock_folder returns the e2e-token.
    #[tokio::test]
    async fn lock_folder_returns_token() {
        let mut srv = mockito::Server::new_async().await;
        let _m = srv
            .mock(
                "POST",
                "/ocs/v2.php/apps/end_to_end_encryption/api/v1/lock/folder-42",
            )
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(r#"{"ocs":{"data":{"e2e-token":"tok-abc"}}}"#)
            .create_async()
            .await;

        let token = client(&srv.url())
            .lock_folder("folder-42", 1)
            .await
            .unwrap();
        assert_eq!(token, "tok-abc");
    }

    // T013-d: map_status returns Maintenance for 503.
    #[test]
    fn map_status_503_is_maintenance() {
        let err = map_status(StatusCode::SERVICE_UNAVAILABLE, "https://server/path");
        assert!(matches!(err, ClientError::Maintenance));
    }

    // T013-e: map_status returns Transient for 409 (lock conflict).
    #[test]
    fn map_status_409_is_transient() {
        let err = map_status(StatusCode::CONFLICT, "https://server/path");
        assert!(matches!(err, ClientError::Transient(_)));
    }

    // T013-f: map_status returns AuthRequired for 401.
    #[test]
    fn map_status_401_is_auth_required() {
        let err = map_status(StatusCode::UNAUTHORIZED, "https://server/path");
        assert!(matches!(err, ClientError::AuthRequired));
    }
}
