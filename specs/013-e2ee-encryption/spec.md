# Feature Specification: End-to-End Encryption (E2EE)

**Feature Branch**: `013-e2ee-encryption`

**Created**: 2026-05-31

**Status**: Draft

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Enable E2EE on a sync pair (Priority: P1)

A user wants to protect a sensitive Nextcloud folder so that no one — not the server administrator, not a network observer — can read its contents. They enable E2EE on a sync pair from the desktop UI. The app generates a mnemonic, uploads an encrypted metadata file to the server, and from that point on all uploads for that pair encrypt content and filenames client-side before they leave the device.

**Why this priority**: Core value proposition of the entire feature. Every other story builds on it.

**Independent Test**: Create a new sync pair pointing at an empty Nextcloud folder, enable E2EE, upload one file, then inspect the raw WebDAV content via a different HTTP client — the file content and filename must be unrecognisable ciphertext.

**Acceptance Scenarios**:

1. **Given** a sync pair that has E2EE disabled, **When** the user enables E2EE from the pair settings UI, **Then** the app generates a 12-word BIP-39 mnemonic, displays it for the user to record, and marks the pair as E2EE-enabled.
2. **Given** E2EE is enabled on a pair, **When** a file is uploaded, **Then** the server receives an encrypted blob with an opaque filename; the original filename and content are unrecoverable without the mnemonic.
3. **Given** a server running a metadata format version the client does not recognise, **When** the user attempts to enable or upload to an E2EE folder, **Then** the operation is refused with a clear error message and no data is written.
4. **Given** a pair in a folder that already has Nextcloud E2EE initialised by another client, **When** the user adds that folder without first pairing, **Then** sync is blocked and the user is prompted to complete the pairing flow.

---

### User Story 2 — Decrypt and browse an E2EE folder (Priority: P1)

A user who has previously enabled E2EE opens the file browser and navigates into an encrypted folder. The app decrypts filenames from the metadata file and shows the original names and sizes. Opening or syncing a file downloads the ciphertext and decrypts it locally before writing to disk.

**Why this priority**: Useless to encrypt without being able to read back. Parity with non-E2EE pairs is required for day-to-day use.

**Independent Test**: Upload an encrypted file via the E2EE init flow, then restart the daemon (clearing any in-memory keys), navigate to the folder in the UI, and confirm the file is listed with its real name and its content can be opened.

**Acceptance Scenarios**:

1. **Given** an E2EE-enabled pair with files on the server, **When** the file browser opens that folder, **Then** decrypted filenames and sizes are shown; the lock icon is visible on the folder.
2. **Given** an E2EE-enabled pair, **When** a file is synced to disk, **Then** the local file contains the original plaintext content.
3. **Given** a legacy metadata v1.x folder, **When** the user opens it, **Then** files are shown read-only (download and decrypt work; upload is blocked with an explanatory message).

---

### User Story 3 — Pair a second device using the mnemonic (Priority: P2)

A user has E2EE running on their desktop. They want to access the same encrypted folder from a second device (laptop). They open the pairing flow on the second device, enter the 12-word mnemonic (or scan a QR code shown on the first device), and from that point the second device can encrypt and decrypt the same folder.

**Why this priority**: Without cross-device pairing, E2EE is only useful on one device — severely limiting utility.

**Independent Test**: Enable E2EE on device A. On device B (fresh install, no prior state), run `adagio e2ee pair` with the mnemonic. Verify device B can download and decrypt a file previously uploaded by device A.

**Acceptance Scenarios**:

1. **Given** a second device with the same Nextcloud account configured, **When** the user runs `adagio e2ee pair` and provides the correct mnemonic, **Then** the device can decrypt existing E2EE files and upload new ones.
2. **Given** the first device displays a QR code via `adagio e2ee pair --qr`, **When** the second device scans it, **Then** the mnemonic is transferred and pairing completes without typing.
3. **Given** an incorrect mnemonic is entered, **When** the user attempts to pair, **Then** pairing fails with a clear error; no keys are stored and no server state is modified.
4. **Given** a successful pairing, **When** the user runs `adagio e2ee status`, **Then** output shows the pair as "paired", the E2EE folder path, and the metadata format version in use.

---

### User Story 4 — View E2EE status and manage keys from the CLI (Priority: P2)

A power user or sysadmin wants to inspect E2EE state, rotate keys after a mnemonic compromise, or initialise E2EE on a headless server without a GUI.

**Why this priority**: CLI-first is a core project principle; headless deployments must be fully supported.

**Independent Test**: On a machine with no GUI, run `adagio e2ee init`, `adagio e2ee status`, and `adagio e2ee pair` entirely from the terminal and confirm all produce correct JSON output and exit codes.

**Acceptance Scenarios**:

1. **Given** a configured account, **When** `adagio e2ee init --pair <id>` is run, **Then** E2EE is initialised for that pair, the mnemonic is printed once, and the command exits 0.
2. **Given** an E2EE-enabled pair, **When** `adagio e2ee status` is run, **Then** output includes pair ID, folder path, metadata version, key fingerprint, and number of encrypted files.
3. **Given** an E2EE-enabled pair, **When** `adagio e2ee status --json` is run, **Then** output is valid JSON matching the documented schema.

---

### User Story 5 — E2EE folder visible with lock icon in UI (Priority: P3)

A non-technical user glancing at the file browser can immediately see which folders are E2EE-protected without hunting through settings.

**Why this priority**: UX polish; core encryption still works without this.

**Independent Test**: Enable E2EE on a folder, open the file browser, and verify a lock icon is displayed on the folder row and the folder's detail panel shows "End-to-end encrypted".

**Acceptance Scenarios**:

1. **Given** a sync pair with E2EE enabled, **When** the file browser renders the folder list, **Then** a lock icon appears next to the pair name in the sidebar and on the folder row.
2. **Given** a mix of E2EE and non-E2EE pairs, **When** the user views the sidebar, **Then** only E2EE pairs show the lock icon; non-E2EE pairs are unchanged.

---

### Edge Cases

- What happens when the keychain is unavailable (locked screen, headless without secret service)? → Sync for E2EE pairs is suspended with an error; non-E2EE pairs are unaffected.
- What happens if the metadata file on the server is corrupted or missing? → The pair enters an error state; the user is prompted to re-initialise or restore from mnemonic.
- What happens if disk is full during decryption to a temp file? → Partial temp file is deleted; error is reported per file; other files continue.
- What happens if a file is added to an E2EE folder from a different Nextcloud client that does not follow the E2EE protocol? → File appears as an "unknown encrypted entry" in the browser; sync skips it with a warning.
- What happens if the user enables E2EE on a pair that already contains unencrypted files? → Existing files are re-uploaded encrypted; original plaintext server copies are deleted after the encrypted version is confirmed uploaded.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST encrypt file content using AES-256-GCM with a unique random key and nonce per file before uploading to the server.
- **FR-002**: The system MUST encrypt filenames in the metadata file so that the server never stores original filenames in plaintext.
- **FR-003**: The system MUST store E2EE keys in the OS keychain; keys MUST NOT be persisted to disk in plaintext.
- **FR-004**: The system MUST generate a 12-word BIP-39 mnemonic as the human-readable key backup when E2EE is first initialised for a pair.
- **FR-005**: The system MUST display the mnemonic exactly once during initialisation and confirm the user has recorded it before proceeding.
- **FR-006**: The system MUST produce and consume Nextcloud metadata v2.x format (one `metadata.json` per E2EE root folder).
- **FR-007**: The system MUST support read-only access to Nextcloud metadata v1.x folders (decrypt and download only; uploads always use v2.x).
- **FR-008**: The system MUST refuse all write operations to an E2EE folder when the server advertises a `metadata_ver` value the client does not recognise, surfacing a clear error.
- **FR-009**: The system MUST disable bulk-upload for E2EE-enabled pairs in this version.
- **FR-010**: The system MUST support pairing a second device by entering the 12-word mnemonic or scanning a QR code.
- **FR-011**: The system MUST provide `adagio e2ee init`, `adagio e2ee status`, and `adagio e2ee pair` CLI subcommands with `--json` output.
- **FR-012**: The system MUST show a lock icon on E2EE folders in the desktop file browser.
- **FR-013**: The system MUST allow E2EE to be enabled or disabled per sync pair independently; disabling E2EE re-uploads affected files in plaintext and removes the metadata file.
- **FR-014**: When E2EE is enabled on a pair that already contains unencrypted server files, the system MUST re-encrypt them before deleting the plaintext server copies.
- **FR-015**: E2EE sync MUST be suspended (not crash) when the OS keychain is unavailable; non-E2EE pairs MUST continue syncing normally.

### Key Entities

- **E2EE Key Set**: The master key material derived from the mnemonic. Has a fingerprint, creation timestamp, and associated pair IDs. Stored in keychain only.
- **E2EE Metadata File** (`metadata.json`): Server-side per-root JSON file. Contains encrypted filename mapping, per-file AES keys, nonces, auth tags, mime types, mtime, size, and `metadata_ver` field.
- **Encrypted File Entry**: A server object consisting of an opaque ciphertext blob. The real filename and content are stored only in the metadata file and in the decrypted local copy.
- **Mnemonic**: 12-word BIP-39 phrase. Single source of truth for key recovery. Shown once at init; never stored by the app.
- **Pairing Token**: Short-lived proof of possession used to register a new device against an existing E2EE key set via the Nextcloud E2EE OCS API.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A file uploaded through an E2EE-enabled pair cannot be identified or read by inspecting raw server storage — confirmed by examining the WebDAV file content directly.
- **SC-002**: A second device can access an E2EE folder using only the mnemonic within 2 minutes of beginning the pairing flow.
- **SC-003**: Enabling E2EE on a pair and completing the first encrypted sync takes under 60 seconds for a folder containing up to 100 files of average size 1 MB on a 10 Mbps connection.
- **SC-004**: Decryption overhead adds no more than 100 ms per file to sync time for files up to 10 MB on the target hardware.
- **SC-005**: 100% of files in a v1.x legacy folder can be downloaded and decrypted successfully; 0% can be uploaded (enforced by the hard gate).
- **SC-006**: All three CLI subcommands (`init`, `status`, `pair`) produce valid JSON output and correct exit codes in headless mode without a GUI.
- **SC-007**: An incorrect mnemonic during pairing is rejected in under 1 second with no server-side state change.

## Assumptions

- The Nextcloud server has the `end_to_end_encryption` app ≥ 2.0 installed and enabled for the target account.
- The Nextcloud E2EE OCS API endpoints (`/ocs/v2.php/apps/end_to_end_encryption/api/v2/…`) are stable and follow the published RFC for v2.x.
- The OS keychain (secret-service on Linux, Keychain on macOS, DPAPI on Windows) is accessible whenever the daemon is running; headless Linux deployments without a secret service will see E2EE pairs suspended, consistent with existing credential handling.
- A single mnemonic covers all E2EE pairs for a given account on a given device; per-pair mnemonics are not required.
- QR code pairing is in-scope for the desktop UI but not for the headless CLI (which uses typed mnemonic only).
- Key rotation (changing the mnemonic without re-encrypting all files) is out of scope for v1.0.
- Sharing encrypted files with other Nextcloud users (shared E2EE) is out of scope; the server's sharing API for E2EE folders is not implemented.
