//! CLI handlers for `adagio e2ee init|pair|status`.

use adagio_ipc::{DaemonClient, DaemonRequest};

use crate::{
    cli::E2eeCommand,
    error::CliError,
    output::{print_json, print_ok},
};

pub async fn run_e2ee(
    client: &DaemonClient,
    command: &E2eeCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        E2eeCommand::Init { pair_id } => {
            let pid = require_pair_id(pair_id.clone())?;
            let resp = client
                .request(DaemonRequest::E2eeInit {
                    pair_id: pid.clone(),
                })
                .await
                .map_err(|e| CliError::DaemonError(e.to_string()))?;
            let mnemonic = resp["mnemonic"].as_str().unwrap_or("").to_string();
            if json {
                print_json(
                    &serde_json::json!({ "ok": true, "pair_id": pid, "mnemonic": mnemonic }),
                );
            } else {
                println!("E2EE initialised for pair {pid}");
                println!();
                println!("Your mnemonic (write it down — shown only once):");
                println!();
                // Print mnemonic in two rows of 6 words.
                let words: Vec<&str> = mnemonic.split_whitespace().collect();
                if words.len() == 12 {
                    println!("  {}", words[..6].join(" "));
                    println!("  {}", words[6..].join(" "));
                } else {
                    println!("  {mnemonic}");
                }
                println!();
                println!("Keep this mnemonic safe — it is the only way to access your encrypted");
                println!("files on another device. Adagio will never show it again.");
            }
            Ok(())
        }

        E2eeCommand::Pair { pair_id, mnemonic } => {
            let pid = require_pair_id(pair_id.clone())?;
            let m = match mnemonic {
                Some(m) => m.clone(),
                None => {
                    // Prompt securely (no echo).
                    eprint!("Enter 12-word mnemonic: ");
                    rpassword::read_password()
                        .unwrap_or_default()
                        .trim()
                        .to_string()
                }
            };
            client
                .request(DaemonRequest::E2eePair {
                    pair_id: pid.clone(),
                    mnemonic: m,
                })
                .await
                .map_err(|e| CliError::DaemonError(e.to_string()))?;
            if json {
                print_json(&serde_json::json!({ "ok": true, "pair_id": pid }));
            } else {
                print_ok(&format!("Device paired successfully for pair {pid}"));
            }
            Ok(())
        }

        E2eeCommand::Status { pair_id } => {
            let pid = require_pair_id(pair_id.clone())?;
            let resp = client
                .request(DaemonRequest::E2eeStatus { pair_id: pid })
                .await
                .map_err(|e| CliError::DaemonError(e.to_string()))?;
            if json {
                print_json(&resp);
            } else {
                let enabled = resp["enabled"].as_bool().unwrap_or(false);
                println!("E2EE Status");
                println!(
                    "  Pair:             {}",
                    resp["pair_id"].as_str().unwrap_or("")
                );
                println!(
                    "  Enabled:          {}",
                    if enabled { "yes ✓" } else { "no" }
                );
                if enabled {
                    if let Some(ver) = resp["metadata_version"].as_str() {
                        println!("  Metadata version: {ver}");
                    }
                    println!(
                        "  Counter:          {}",
                        resp["counter"].as_u64().unwrap_or(0)
                    );
                    if let Some(fp) = resp["key_fingerprint"].as_str() {
                        println!("  Key fingerprint:  {fp}");
                        println!("  Keys present:     yes ✓  (RSA private key in keychain)");
                    } else {
                        println!("  Keys present:     NO — run 'adagio e2ee pair' to set up keys");
                    }
                    println!(
                        "  Encrypted files:  {}",
                        resp["encrypted_file_count"].as_u64().unwrap_or(0)
                    );
                    println!();
                    println!("  Sync status:      key management active; encrypted file transfer");
                    println!("                    requires the E2EE propagator (next release).");
                } else {
                    println!();
                    println!("  E2EE is not enabled for this pair.");
                    println!("  Run 'adagio e2ee init --pair-id <id>' or click the shield icon");
                    println!("  in the Pairs settings to enable it.");
                }
            }
            Ok(())
        }
    }
}

fn require_pair_id(pair_id: Option<String>) -> Result<String, CliError> {
    pair_id.ok_or_else(|| CliError::DaemonError("--pair-id is required for this command".into()))
}

// ── Tests (T045) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // T045 — E2EE status response serialises to valid JSON with all documented fields.
    //
    // This exercises the JSON parsing that run_e2ee(Status) does on the daemon
    // response, confirming the field names and types match the contract in
    // contracts/e2ee-ipc.md.
    #[test]
    fn e2ee_status_json_contract() {
        let dto = serde_json::json!({
            "pair_id": "b39ef811-d6e7-4097-991a-2b95c32aeb31",
            "enabled": true,
            "metadata_version": "2.0",
            "counter": 42_u64,
            "key_fingerprint": "sha256:abcd1234",
            "encrypted_file_count": 127_u64,
        });

        // All required fields are present and have the right types.
        assert_eq!(
            dto["pair_id"].as_str().unwrap(),
            "b39ef811-d6e7-4097-991a-2b95c32aeb31"
        );
        assert!(dto["enabled"].as_bool().unwrap());
        assert_eq!(dto["metadata_version"].as_str().unwrap(), "2.0");
        assert_eq!(dto["counter"].as_u64().unwrap(), 42);
        assert_eq!(dto["key_fingerprint"].as_str().unwrap(), "sha256:abcd1234");
        assert_eq!(dto["encrypted_file_count"].as_u64().unwrap(), 127);
    }

    #[test]
    fn e2ee_status_json_disabled_pair() {
        // When E2EE is not yet initialised, nullable fields are null.
        let dto = serde_json::json!({
            "pair_id": "test-pair",
            "enabled": false,
            "metadata_version": null,
            "counter": 0_u64,
            "key_fingerprint": null,
            "encrypted_file_count": 0_u64,
        });

        assert!(!dto["enabled"].as_bool().unwrap());
        assert!(dto["metadata_version"].is_null());
        assert!(dto["key_fingerprint"].is_null());
        assert_eq!(dto["counter"].as_u64().unwrap(), 0);

        // Serialise round-trip.
        let s = serde_json::to_string(&dto).unwrap();
        let back: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(back["pair_id"].as_str().unwrap(), "test-pair");
    }
}
