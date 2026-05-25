use adagio_core::error::ClientError;
use adagio_core::types::{Checksum, ChecksumAlgorithm, RemoteItem};
use chrono::{DateTime, Utc};
use quick_xml::events::Event;
use quick_xml::Reader;

/// A single item extracted from a WebDAV PROPFIND multi-status response.
struct PropfindItem {
    href: String,
    is_dir: bool,
    size: u64,
    etag: String,
    file_id: Option<String>,
    mtime: DateTime<Utc>,
    checksum: Option<Checksum>,
    status_ok: bool,
}

impl Default for PropfindItem {
    fn default() -> Self {
        Self {
            href: String::new(),
            is_dir: false,
            size: 0,
            etag: String::new(),
            file_id: None,
            mtime: Utc::now(),
            checksum: None,
            status_ok: false,
        }
    }
}

/// Parse a WebDAV `207 Multi-Status` XML response body into `RemoteItem`s.
///
/// `prefix_to_strip` is the WebDAV root prefix to remove from hrefs to produce
/// relative paths, e.g. `/remote.php/dav/files/username/sync-root`.
pub fn parse_multistatus(
    xml_bytes: &[u8],
    prefix_to_strip: &str,
) -> Result<Vec<RemoteItem>, ClientError> {
    let mut reader = Reader::from_reader(xml_bytes);
    reader.config_mut().trim_text(true);

    let mut items: Vec<RemoteItem> = Vec::new();
    let mut current: Option<PropfindItem> = None;
    let mut current_tag = String::new();
    let mut in_propstat = false;
    let mut in_prop = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().as_ref()).to_string();
                if local == "collection" {
                    if let Some(ref mut item) = current {
                        item.is_dir = true;
                    }
                }
            }
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref()).to_string();
                match local.as_str() {
                    "response" => {
                        current = Some(PropfindItem::default());
                        in_propstat = false;
                        in_prop = false;
                    }
                    "propstat" => in_propstat = true,
                    "prop" if in_propstat => in_prop = true,
                    "collection" if in_prop => {
                        // <D:collection> with child content (non-empty form)
                        if let Some(ref mut item) = current {
                            item.is_dir = true;
                        }
                    }
                    _ => {}
                }
                current_tag = local;
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let local = local_name(name.as_ref()).to_string();
                match local.as_str() {
                    "response" => {
                        if let Some(item) = current.take() {
                            if item.status_ok {
                                items.push(item_to_remote(item, prefix_to_strip));
                            }
                        }
                    }
                    "propstat" => in_propstat = false,
                    "prop" => in_prop = false,
                    _ => {}
                }
                current_tag = String::new();
            }
            Ok(Event::Text(ref e)) => {
                let text = e
                    .unescape()
                    .map_err(|err| ClientError::Permanent(format!("xml decode: {err}")))?;
                let text = text.trim();
                if let Some(ref mut item) = current {
                    match current_tag.as_str() {
                        "href" => item.href = text.to_string(),
                        "getcontentlength" => {
                            item.size = text.parse().unwrap_or(0);
                        }
                        "getetag" => {
                            item.etag = text.trim_matches('"').to_string();
                        }
                        "fileid" => item.file_id = Some(text.to_string()),
                        "getlastmodified" => {
                            if let Ok(dt) = DateTime::parse_from_rfc2822(text) {
                                item.mtime = dt.to_utc();
                            }
                        }
                        "checksum" => {
                            item.checksum = parse_oc_checksum(text);
                        }
                        "status" => {
                            item.status_ok |= text.contains("200 OK");
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ClientError::Permanent(format!("xml parse error: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}

fn local_name(qualified: &[u8]) -> &str {
    let s = std::str::from_utf8(qualified).unwrap_or("");
    if let Some(pos) = s.rfind(':') {
        &s[pos + 1..]
    } else {
        s
    }
}

fn item_to_remote(item: PropfindItem, prefix: &str) -> RemoteItem {
    use adagio_core::types::RelativePath;
    let path = item.href.trim_start_matches(prefix).trim_start_matches('/');
    RemoteItem {
        path: RelativePath::new(path),
        file_id: item.file_id.unwrap_or_default(),
        etag: item.etag,
        size: item.size,
        mtime: item.mtime,
        checksum: item.checksum,
        is_dir: item.is_dir,
    }
}

/// Parse an OC-Checksum header value like `SHA256:abc123` or `MD5:abc123`.
pub fn parse_oc_checksum(s: &str) -> Option<Checksum> {
    let (algo_str, value) = s.split_once(':')?;
    let algorithm = match algo_str.to_uppercase().as_str() {
        "SHA256" => ChecksumAlgorithm::Sha256,
        "MD5" => ChecksumAlgorithm::Md5,
        _ => return None,
    };
    Some(Checksum {
        algorithm,
        value: value.to_string(),
    })
}

/// Build a PROPFIND request body requesting all sync-relevant properties.
pub fn propfind_body() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:prop>
    <d:getcontentlength/>
    <d:getetag/>
    <d:getlastmodified/>
    <d:resourcetype/>
    <oc:fileid/>
    <oc:checksum/>
  </d:prop>
</d:propfind>"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_oc_checksum_sha256() {
        let c = parse_oc_checksum("SHA256:abc123").unwrap();
        assert_eq!(c.algorithm, ChecksumAlgorithm::Sha256);
        assert_eq!(c.value, "abc123");
    }

    #[test]
    fn parse_oc_checksum_unknown_algo() {
        assert!(parse_oc_checksum("BLAKE2:xyz").is_none());
    }

    #[test]
    fn parse_multistatus_single_file() {
        let xml = br#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:response>
    <d:href>/remote.php/dav/files/user/docs/hello.txt</d:href>
    <d:propstat>
      <d:prop>
        <d:getcontentlength>42</d:getcontentlength>
        <d:getetag>"abc123"</d:getetag>
        <d:getlastmodified>Mon, 01 Jan 2024 00:00:00 GMT</d:getlastmodified>
        <d:resourcetype/>
        <oc:fileid>999</oc:fileid>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let items = parse_multistatus(xml, "/remote.php/dav/files/user/docs").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path.as_str(), "hello.txt");
        assert_eq!(items[0].size, 42);
        assert_eq!(items[0].etag, "abc123");
        assert!(!items[0].is_dir);
    }

    #[test]
    fn parse_multistatus_directory_is_dir() {
        let xml = br#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:response>
    <d:href>/remote.php/dav/files/user/docs/subdir/</d:href>
    <d:propstat>
      <d:prop>
        <d:getcontentlength>0</d:getcontentlength>
        <d:getetag>"dir123"</d:getetag>
        <d:getlastmodified>Mon, 01 Jan 2024 00:00:00 GMT</d:getlastmodified>
        <d:resourcetype><d:collection/></d:resourcetype>
        <oc:fileid>42</oc:fileid>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let items = parse_multistatus(xml, "/remote.php/dav/files/user/docs").unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_dir, "directory entry must have is_dir=true");
    }
}
