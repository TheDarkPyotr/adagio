use crate::xml::{parse_multistatus, propfind_body};
use adagio_core::error::ClientError;
use adagio_core::remote::{ByteStream, RemoteClient};
use adagio_core::types::{ByteRange, Checksum, RemoteItem, RemotePath, ServerCapabilities};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{header, Client, StatusCode};
use tracing::instrument;

/// HTTP client for a single Nextcloud account over WebDAV + OCS APIs.
pub struct NextcloudClient {
    http: Client,
    /// Base URL of the Nextcloud server, e.g. `https://cloud.example.com`.
    server_url: String,
    /// Nextcloud username.
    username: String,
    /// App-password or OAuth2 access token (never logged).
    password: String,
}

impl NextcloudClient {
    pub fn new(
        server_url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let http = Client::builder()
            .user_agent("adagio-nextcloud/0.1")
            .build()
            .expect("failed to build reqwest client");
        Self {
            http,
            server_url: server_url.into().trim_end_matches('/').to_string(),
            username: username.into(),
            password: password.into(),
        }
    }

    /// WebDAV root for this user: `{server}/remote.php/dav/files/{username}`.
    fn dav_root(&self) -> String {
        format!("{}/remote.php/dav/files/{}", self.server_url, self.username)
    }

    /// Absolute WebDAV URL for a remote path.
    fn dav_url(&self, path: &RemotePath) -> String {
        let p = path.0.trim_start_matches('/');
        format!("{}/{}", self.dav_root(), p)
    }

    /// URL-path prefix to strip from PROPFIND hrefs to produce user-relative paths.
    /// PROPFIND hrefs contain only the path component (no scheme/host), so we
    /// extract just the path from the constructed DAV URL.
    fn dav_href_prefix(&self, path: &RemotePath) -> String {
        let full_url = self.dav_url(path);
        reqwest::Url::parse(&full_url)
            .map(|u| u.path().trim_end_matches('/').to_string())
            .unwrap_or(full_url)
    }

    fn auth(&self) -> (String, String) {
        (self.username.clone(), self.password.clone())
    }

    fn map_status(status: StatusCode, url: &str) -> ClientError {
        match status.as_u16() {
            401 | 403 => ClientError::AuthRequired,
            404 => ClientError::Permanent(format!("not found: {url}")),
            s if s >= 500 => ClientError::Transient(format!("server error {s}: {url}")),
            s => ClientError::Permanent(format!("HTTP {s}: {url}")),
        }
    }
}

#[async_trait]
impl RemoteClient for NextcloudClient {
    #[instrument(err, skip(self), fields(path = %path))]
    async fn list(&self, path: &RemotePath) -> Result<Vec<RemoteItem>, ClientError> {
        let url = self.dav_url(path);
        let (user, pass) = self.auth();
        let resp = self
            .http
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .basic_auth(&user, Some(&pass))
            .header("Depth", "1")
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(propfind_body())
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() && resp.status() != StatusCode::MULTI_STATUS {
            return Err(Self::map_status(resp.status(), &url));
        }

        let body = resp
            .bytes()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let remote_prefix = self.dav_href_prefix(path);
        let mut items = parse_multistatus(&body, &remote_prefix)?;
        // Remove the directory itself from the listing.
        items.retain(|it| !it.path.as_str().is_empty());
        Ok(items)
    }

    #[instrument(err, skip(self), fields(path = %path))]
    async fn list_recursive(&self, path: &RemotePath) -> Result<Vec<RemoteItem>, ClientError> {
        let url = self.dav_url(path);
        let (user, pass) = self.auth();
        let resp = self
            .http
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .basic_auth(&user, Some(&pass))
            .header("Depth", "infinity")
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(propfind_body())
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() && resp.status() != StatusCode::MULTI_STATUS {
            return Err(Self::map_status(resp.status(), &url));
        }

        let body = resp
            .bytes()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        let remote_prefix = self.dav_href_prefix(path);
        let mut items = parse_multistatus(&body, &remote_prefix)?;
        items.retain(|it| !it.path.as_str().is_empty());
        Ok(items)
    }

    #[instrument(err, skip(self, data), fields(path = %path, size))]
    async fn upload(
        &self,
        path: &RemotePath,
        data: ByteStream,
        size: u64,
        checksum: &Checksum,
    ) -> Result<String, ClientError> {
        let url = self.dav_url(path);
        let (user, pass) = self.auth();
        let oc_checksum = format!(
            "{}:{}",
            match checksum.algorithm {
                adagio_core::types::ChecksumAlgorithm::Sha256 => "SHA256",
                adagio_core::types::ChecksumAlgorithm::Md5 => "MD5",
            },
            checksum.value
        );

        let stream_body = reqwest::Body::wrap_stream(data);
        let resp = self
            .http
            .put(&url)
            .basic_auth(&user, Some(&pass))
            .header("OC-Checksum", &oc_checksum)
            .header(header::CONTENT_LENGTH, size)
            .body(stream_body)
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), &url));
        }

        let etag = resp
            .headers()
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        Ok(etag)
    }

    async fn begin_chunked_upload(&self) -> Result<String, ClientError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let url = format!(
            "{}/remote.php/dav/uploads/{}/{}",
            self.server_url, self.username, session_id
        );
        let (user, pass) = self.auth();
        let resp = self
            .http
            .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
            .basic_auth(&user, Some(&pass))
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), &url));
        }
        Ok(url)
    }

    async fn upload_chunk(
        &self,
        session_url: &str,
        chunk_index: u32,
        data: ByteStream,
        size: u64,
    ) -> Result<(), ClientError> {
        let chunk_url = format!("{}/{:08}", session_url, chunk_index);
        let (user, pass) = self.auth();
        let resp = self
            .http
            .put(&chunk_url)
            .basic_auth(&user, Some(&pass))
            .header(header::CONTENT_LENGTH, size)
            .body(reqwest::Body::wrap_stream(data))
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), &chunk_url));
        }
        Ok(())
    }

    async fn finalize_chunked_upload(
        &self,
        session_url: &str,
        dest: &RemotePath,
        _total_size: u64,
        checksum: &Checksum,
    ) -> Result<String, ClientError> {
        let dest_url = self.dav_url(dest);
        let (user, pass) = self.auth();
        let oc_checksum = format!(
            "{}:{}",
            match checksum.algorithm {
                adagio_core::types::ChecksumAlgorithm::Sha256 => "SHA256",
                adagio_core::types::ChecksumAlgorithm::Md5 => "MD5",
            },
            checksum.value
        );
        let assembly_url = format!("{}/.file", session_url);
        let resp = self
            .http
            .request(reqwest::Method::from_bytes(b"MOVE").unwrap(), &assembly_url)
            .basic_auth(&user, Some(&pass))
            .header("Destination", &dest_url)
            .header("OC-Checksum", &oc_checksum)
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), &assembly_url));
        }
        let etag = resp
            .headers()
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();
        Ok(etag)
    }

    async fn list_uploaded_chunks(&self, session_url: &str) -> Result<Vec<u32>, ClientError> {
        let (user, pass) = self.auth();
        let resp = self
            .http
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                session_url,
            )
            .basic_auth(&user, Some(&pass))
            .header("Depth", "1")
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() && resp.status() != StatusCode::MULTI_STATUS {
            return Err(Self::map_status(resp.status(), session_url));
        }

        let body = resp
            .bytes()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;
        // Parse chunk indices from PROPFIND hrefs: each chunk href ends in /XXXXXXXX.
        // Extract the path component of the session URL so prefix-stripping matches.
        let session_path = reqwest::Url::parse(session_url)
            .map(|u| u.path().trim_end_matches('/').to_string())
            .unwrap_or_else(|_| session_url.to_string());
        let items = parse_multistatus(&body, &session_path).unwrap_or_default();
        let mut indices = Vec::new();
        for item in items {
            if let Ok(idx) = item.path.as_str().parse::<u32>() {
                indices.push(idx);
            }
        }
        Ok(indices)
    }

    #[instrument(err, skip(self), fields(path = %path))]
    async fn download(
        &self,
        path: &RemotePath,
        range: Option<ByteRange>,
    ) -> Result<ByteStream, ClientError> {
        let url = self.dav_url(path);
        let (user, pass) = self.auth();
        let mut req = self.http.get(&url).basic_auth(&user, Some(&pass));

        if let Some(r) = range {
            let range_header = match r.end {
                Some(end) => format!("bytes={}-{}", r.start, end),
                None => format!("bytes={}-", r.start),
            };
            req = req.header(header::RANGE, range_header);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), &url));
        }

        let stream = resp
            .bytes_stream()
            .map(|r| r.map_err(std::io::Error::other));
        Ok(Box::pin(stream))
    }

    #[instrument(err, skip(self), fields(path = %path))]
    async fn delete(&self, path: &RemotePath) -> Result<(), ClientError> {
        let url = self.dav_url(path);
        let (user, pass) = self.auth();
        let resp = self
            .http
            .delete(&url)
            .basic_auth(&user, Some(&pass))
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        match resp.status() {
            StatusCode::NO_CONTENT | StatusCode::OK | StatusCode::NOT_FOUND => Ok(()),
            s => Err(Self::map_status(s, &url)),
        }
    }

    #[instrument(err, skip(self), fields(from = %from, to = %to))]
    async fn move_item(&self, from: &RemotePath, to: &RemotePath) -> Result<(), ClientError> {
        let from_url = self.dav_url(from);
        let to_url = self.dav_url(to);
        let (user, pass) = self.auth();
        let resp = self
            .http
            .request(reqwest::Method::from_bytes(b"MOVE").unwrap(), &from_url)
            .basic_auth(&user, Some(&pass))
            .header("Destination", &to_url)
            .header("Overwrite", "T")
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), &from_url));
        }
        Ok(())
    }

    #[instrument(err, skip(self), fields(path = %path))]
    async fn create_dir(&self, path: &RemotePath) -> Result<(), ClientError> {
        let url = self.dav_url(path);
        let (user, pass) = self.auth();
        let resp = self
            .http
            .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
            .basic_auth(&user, Some(&pass))
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        match resp.status() {
            StatusCode::CREATED | StatusCode::OK | StatusCode::METHOD_NOT_ALLOWED => Ok(()),
            s => Err(Self::map_status(s, &url)),
        }
    }

    async fn capabilities(&self) -> Result<ServerCapabilities, ClientError> {
        let url = format!(
            "{}/ocs/v2.php/cloud/capabilities?format=json",
            self.server_url
        );
        let (user, pass) = self.auth();
        let resp = self
            .http
            .get(&url)
            .basic_auth(&user, Some(&pass))
            .header("OCS-ApiRequest", "true")
            .send()
            .await
            .map_err(|e| ClientError::Transient(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(Self::map_status(resp.status(), &url));
        }

        let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);

        let supports_chunked = json
            .pointer("/ocs/data/capabilities/dav/chunking")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);

        Ok(ServerCapabilities {
            max_chunk_size: 10 * 1024 * 1024, // 10 MiB default
            supports_chunked_upload: supports_chunked,
            supports_dav_checksum: true,
            server_version: json
                .pointer("/ocs/data/version/string")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
        })
    }
}
