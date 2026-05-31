# Tasks: End-to-End Encryption (E2EE)

**Input**: Design documents from `specs/013-e2ee-encryption/`

**Prerequisites**: plan.md ✅ spec.md ✅ research.md ✅ data-model.md ✅ contracts/ ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY. Tests MUST be written first and MUST FAIL before implementation begins.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story ([US1]–[US5])

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Crate skeleton, workspace wiring, ADR.

- [x] T001 Create `crates/adagio-e2ee/` workspace crate with `Cargo.toml`, `src/lib.rs` stub, and add it to `Cargo.toml` workspace members
- [x] T002 [P] Add workspace dependencies `rsa`, `x509-cert`, `pkcs8`, `bip39`, `pbkdf2`, `hmac`, `sha2`, `cms`, `flate2`, `zeroize` to root `Cargo.toml`
- [x] T003 [P] Write `docs/adr/017-e2ee-protocol.md` documenting the protocol choices (RSA-4096 + AES-128-GCM + CMS, not Ed25519/AES-256; see research.md)
- [x] T004 Create `crates/adagio-nextcloud/src/e2ee_ocs.rs` stub with all OCS endpoint URL constants and an empty `E2eeOcsClient` struct

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared types, traits, migration, IPC variants, and crypto primitives that all user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T005 Write migration `crates/adagio-core/migrations/004_e2ee.sql` creating `e2ee_account_keys` and `e2ee_folder_state` tables per `data-model.md`
- [x] T006 [P] Add `e2ee_enabled: bool` and `e2ee_account_id: Option<String>` to `SyncPair` in `crates/adagio-core/src/types.rs` with `#[serde(default)]`
- [x] T007 [P] Add `e2ee_enabled: bool` and `e2ee_account_id: Option<String>` to `SavedPair` in `crates/adagio-desktop/src/config/mod.rs` with `#[serde(default)]`
- [x] T008 Define `E2eeError` enum in `crates/adagio-e2ee/src/lib.rs` covering: `KeychainUnavailable`, `InvalidMnemonic`, `MnemonicMismatch`, `ServerError(String)`, `UnsupportedMetadataVersion(String)`, `LegacyMetadataReadOnly`, `LockConflict`, `Io(std::io::Error)`
- [x] T009 Define `E2eeProvider` trait in `crates/adagio-e2ee/src/lib.rs` exactly as specified in `plan.md`; define `E2eeMetadata`, `E2eeFileEntry`, `E2eeStatusDto` structs
- [x] T010 Add `E2eeInit { pair_id }`, `E2eePair { pair_id, mnemonic }`, `E2eeStatus { pair_id }` variants to `DaemonRequest` and matching response variants to `DaemonResponse` in `crates/adagio-ipc/src/types.rs`
- [x] T011 [P] Write unit test `T005-migration` in `crates/adagio-core/src/journal/corruption.rs` (or a new `e2ee_journal_test.rs`) confirming migration 004 runs without error on a fresh database
- [x] T012 Implement `E2eeOcsClient` in `crates/adagio-nextcloud/src/e2ee_ocs.rs`: methods `get_server_key`, `get_public_key`, `post_public_key`, `get_private_key`, `post_private_key`, `get_metadata`, `post_metadata`, `put_metadata`, `delete_metadata`, `lock_folder`, `unlock_folder`, `mark_folder_encrypted` — each makes the documented OCS HTTP call with Basic auth; return `Result<serde_json::Value, ClientError>`
- [x] T013 [P] Write unit tests for `E2eeOcsClient` in `crates/adagio-nextcloud/src/e2ee_ocs.rs` using `MockRemoteClient` or a `mockito` HTTP mock for each endpoint method

**Checkpoint**: Foundation complete — all user story phases can now begin.

---

## Phase 3: User Story 1 — Enable E2EE on a sync pair (Priority: P1) 🎯 MVP

**Goal**: User can enable E2EE on a pair from the desktop UI or CLI. A mnemonic is generated, keys uploaded, and subsequent file uploads are encrypted.

**Independent Test**: Create a pair with E2EE enabled, upload one file, inspect raw WebDAV — file content must be unrecognisable ciphertext; original filename must be absent.

### Tests for User Story 1 (MANDATORY — write first, confirm FAIL)

- [x] T014 [P] [US1] Unit test `cipher_roundtrip` in `crates/adagio-e2ee/src/cipher.rs`: encrypt then decrypt a known plaintext with `AES-128-GCM` and assert recovered bytes match
- [x] T015 [P] [US1] Unit test `key_gen_and_wrap` in `crates/adagio-e2ee/src/keys.rs`: generate RSA-4096 key, derive AES-256 key from a test mnemonic via PBKDF2, wrap and unwrap, assert private key round-trips
- [x] T016 [P] [US1] Unit test `metadata_roundtrip` in `crates/adagio-e2ee/src/metadata.rs`: construct `E2eeMetadata`, encrypt to outer envelope, decrypt back, assert all file entries match
- [x] T017 [P] [US1] Unit test `metadata_cms_sign_verify` in `crates/adagio-e2ee/src/metadata.rs`: sign inner JSON with a test RSA key, verify CMS signature succeeds; tamper with JSON and assert verification fails

### Implementation for User Story 1

- [x] T018 [US1] Implement `crates/adagio-e2ee/src/cipher.rs`: `encrypt_file(key, nonce, plaintext) -> (ciphertext, auth_tag)` and `decrypt_file(key, nonce, auth_tag, ciphertext) -> plaintext` using `AES-128-GCM`
- [x] T019 [US1] Implement `crates/adagio-e2ee/src/keys.rs`: `generate_rsa_keypair() -> RsaPrivateKey`, `build_csr(privkey, user_id) -> Vec<u8>`, `wrap_private_key(privkey, mnemonic) -> (ciphertext, nonce, salt)`, `unwrap_private_key(blob, mnemonic) -> RsaPrivateKey`, `wrap_metadata_key(metadata_key, cert) -> Vec<u8>`, `unwrap_metadata_key(encrypted, privkey) -> [u8;16]` — all per research.md KDF/crypto spec
- [x] T020 [US1] Implement `crates/adagio-e2ee/src/metadata.rs`: `E2eeMetadata` ↔ inner-JSON ser/de (gzip + AES-128-GCM encrypt/decrypt), outer-envelope ser/de, CMS SignedData sign/verify over canonical inner JSON using `cms` crate
- [x] T021 [US1] Implement `NcE2eeClient::init()` in `crates/adagio-e2ee/src/provider.rs`: generate keys → build + submit CSR via `E2eeOcsClient` → wrap private key with mnemonic → upload via `post_private_key` → store certificate in `e2ee_account_keys` table → mark folder encrypted via `mark_folder_encrypted` → generate fresh metadata file → return mnemonic
- [x] T022 [US1] Implement `NcE2eeClient::encrypt_file()` in `crates/adagio-e2ee/src/provider.rs`: generate per-file key + nonce → encrypt with `cipher.rs` → return `(ciphertext, uuid, E2eeFileEntry)`
- [x] T023 [US1] Implement `NcE2eeClient::commit_metadata()` in `crates/adagio-e2ee/src/provider.rs`: lock folder (counter+1) → encrypt inner metadata → CMS sign → PUT metadata with token + signature → unlock; retry once on `LockConflict` (409)
- [x] T024 [US1] Wire E2EE encrypt path in `crates/adagio-core/src/cycle/propagator.rs`: add `with_e2ee(provider, pair_id)` builder; in `upload_single`, if E2EE active call `provider.encrypt_file()` before PUT and use UUID as remote filename; call `commit_metadata()` after all uploads in a cycle
- [x] T025 [US1] Add `E2eeInit` handler to `crates/adagio-daemon/src/dispatcher.rs`: call `provider.init(pair_id)` → update `e2ee_enabled` on pair in config → return `{ mnemonic }` response; key must not appear in any log
- [x] T026 [US1] Create `crates/adagio-desktop/src/commands/e2ee.rs` with `e2ee_init(pair_id)` Tauri command forwarding to `DaemonRequest::E2eeInit`; register in `lib.rs`
- [x] T027 [US1] Add TypeScript binding `e2eeInit(pairId)` to `crates/adagio-desktop/src-ui/src/tauri.ts`
- [x] T028 [US1] Add E2EE toggle to `crates/adagio-desktop/src-ui/src/components/PairsScene.tsx`: switch on pair card enabling E2EE; show mnemonic-display modal on init (12 words, copy button, "I have saved this" confirmation before dismiss)
- [x] T029 [US1] Implement `crates/adagio-cli/src/handlers/e2ee.rs` with `adagio e2ee init [--pair <id>] [--json]`; print mnemonic to stdout (never to log); register handler in `crates/adagio-cli/src/run.rs`

**Checkpoint**: Enable E2EE on a pair, upload a file, confirm server shows UUID-named ciphertext. `adagio e2ee init` works headlessly.

---

## Phase 4: User Story 2 — Decrypt and browse an E2EE folder (Priority: P1)

**Goal**: Files in an E2EE pair are listed with their real names and can be downloaded and decrypted transparently.

**Independent Test**: After daemon restart (keys cleared from memory), navigate to E2EE folder in UI, confirm original filenames appear and file content is correct after opening.

### Tests for User Story 2 (MANDATORY — write first, confirm FAIL)

- [x] T030 [P] [US2] Unit test `sync_metadata_roundtrip` in `crates/adagio-e2ee/src/provider.rs`: build an outer envelope, store in mock OCS client, call `sync_metadata()`, assert returned `E2eeMetadata.files` matches the input
- [x] T031 [P] [US2] Unit test `v1x_gate_blocks_upload` in `crates/adagio-e2ee/src/provider.rs`: set `metadata_version = "1.0"` in `e2ee_folder_state`, call `encrypt_file()`, assert `E2eeError::LegacyMetadataReadOnly` is returned

### Implementation for User Story 2

- [x] T032 [US2] Implement `NcE2eeClient::sync_metadata()` in `crates/adagio-e2ee/src/provider.rs`: `GET .../meta-data/{folderId}` → decrypt outer envelope → verify CMS signature → assert counter ≥ stored counter → update `e2ee_folder_state.counter`; return `E2eeError::UnsupportedMetadataVersion` for unknown versions; return `E2eeError::LegacyMetadataReadOnly` for v1.x
- [x] T033 [US2] Implement `NcE2eeClient::decrypt_file()` in `crates/adagio-e2ee/src/provider.rs`: look up `E2eeFileEntry` by uuid from cached/fetched metadata → call `cipher::decrypt_file()` → return plaintext; load key from `e2ee_account_keys` + keychain if not cached
- [x] T034 [US2] Wire E2EE decrypt path in `crates/adagio-core/src/cycle/propagator.rs`: in `download_file`, if E2EE active call `sync_metadata()` once per cycle → download UUID-named ciphertext → `decrypt_file()` → write plaintext to `local_root/original_filename`
- [x] T035 [US2] Make `DaemonRequest::ListSyncedFiles` E2EE-aware in `crates/adagio-daemon/src/dispatcher.rs`: for E2EE pairs, read `E2eeMetadata.files` (using `provider.sync_metadata()`) to resolve UUID→filename mapping; return real filenames in `FileStatusDto`
- [x] T036 [US2] Add read-only badge in `crates/adagio-desktop/src-ui/src/components/FilesScene.tsx` when `syncStatus` has a v1.x E2EE pair: show inline "Read-only (legacy encryption)" label on folder rows

**Checkpoint**: Files in E2EE folder are listed and opened with original names and content.

---

## Phase 5: User Story 3 — Pair a second device (Priority: P2)

**Goal**: A second device gains access to existing E2EE folders by providing the 12-word mnemonic.

**Independent Test**: On a clean config dir, run `adagio e2ee pair --mnemonic "..."`, sync, confirm file is decrypted correctly.

### Tests for User Story 3 (MANDATORY — write first, confirm FAIL)

- [x] T037 [P] [US3] Unit test `pair_correct_mnemonic` in `crates/adagio-e2ee/src/keys.rs`: wrap a test RSA key with mnemonic A, call `unwrap_private_key(blob, mnemonic_A)`, assert private key round-trips
- [x] T038 [P] [US3] Unit test `pair_wrong_mnemonic_fails` in `crates/adagio-e2ee/src/keys.rs`: wrap with mnemonic A, unwrap with mnemonic B, assert `E2eeError::MnemonicMismatch`

### Implementation for User Story 3

- [x] T039 [US3] Implement `NcE2eeClient::pair()` in `crates/adagio-e2ee/src/provider.rs`: `GET .../private-key` → PBKDF2 from mnemonic → AES-256-GCM decrypt private key blob → `GET .../public-key?users=[userId]` → verify private/public match → store private key in keychain under `"adagio-e2ee/{account_id}"` → update `e2ee_account_keys` row
- [x] T040 [US3] Add `E2eePair` handler to `crates/adagio-daemon/src/dispatcher.rs`: call `provider.pair(pair_id, mnemonic)` → return `Unit`; mnemonic MUST NOT appear in any log or response beyond this handler
- [x] T041 [US3] Add `e2ee_pair(pair_id, mnemonic)` Tauri command to `crates/adagio-desktop/src/commands/e2ee.rs`; add TypeScript binding `e2eePair(pairId, mnemonic)` to `tauri.ts`
- [x] T042 [US3] Add mnemonic-entry dialog to `crates/adagio-desktop/src-ui/src/components/PairsScene.tsx`: "Pair this device" button → modal with 12-word input field + secure paste + "Pair" button; show success/error
- [x] T043 [US3] Add `adagio e2ee pair [--mnemonic <words>] [--pair <id>] [--json]` to `crates/adagio-cli/src/handlers/e2ee.rs`; interactive secure prompt when `--mnemonic` is omitted

**Checkpoint**: Device B can pair with mnemonic and decrypt files uploaded by Device A.

---

## Phase 6: User Story 4 — CLI status and management (Priority: P2)

**Goal**: `adagio e2ee status` reports E2EE state; all three subcommands produce valid JSON.

**Independent Test**: Run all three subcommands with `--json` on a headless machine with no GUI; confirm valid JSON output and exit codes 0/1.

### Tests for User Story 4 (MANDATORY — write first, confirm FAIL)

- [x] T044 [P] [US4] Unit test `e2ee_status_serialises` in `crates/adagio-cli/src/handlers/e2ee.rs` (or adjacent test module): construct an `E2eeStatusDto`, serialize to JSON, assert all documented fields present with correct types
- [x] T045 [P] [US4] CLI integration test in `crates/adagio-cli/tests/` (or `crates/adagio-daemon/tests/`): mock daemon returning a fixed `E2eeStatusDto`; run `adagio e2ee status --json`; assert output parses as valid `E2eeStatusDto` JSON

### Implementation for User Story 4

- [x] T046 [US4] Implement `NcE2eeClient::status()` in `crates/adagio-e2ee/src/provider.rs`: read `e2ee_account_keys` + `e2ee_folder_state` from journal → count encrypted files in `E2eeMetadata.files` → return `E2eeStatusDto`
- [x] T047 [US4] Add `E2eeStatus` handler to `crates/adagio-daemon/src/dispatcher.rs`: call `provider.status(pair_id)` → return serialised `E2eeStatusDto` as `DaemonResponse::Status(value)`
- [x] T048 [US4] Add `e2ee_status(pair_id)` Tauri command to `crates/adagio-desktop/src/commands/e2ee.rs`; add TypeScript binding `e2eeStatus(pairId)` to `tauri.ts`
- [x] T049 [US4] Implement `adagio e2ee status [--pair <id>] [--json]` in `crates/adagio-cli/src/handlers/e2ee.rs`: print tabular human output or JSON matching `E2eeStatusDto`

**Checkpoint**: `adagio e2ee init`, `adagio e2ee pair`, and `adagio e2ee status` all work headlessly with `--json`.

---

## Phase 7: User Story 5 — Lock icon in file browser (Priority: P3)

**Goal**: E2EE-enabled pairs and folders show a lock icon in the UI; non-E2EE pairs are unaffected.

**Independent Test**: Enable E2EE on one pair; confirm lock icon appears only on that pair in the sidebar and file browser; non-E2EE pair shows no lock.

### Tests for User Story 5 (MANDATORY — write first, confirm FAIL)

- [x] T050 [P] [US5] UI test in `crates/adagio-desktop/src-ui/src/__tests__/FilesScene.test.tsx`: render `FilesScene` with `isVfsPair=false, isE2eePair=true`, assert lock icon element is present; with `isE2eePair=false` assert lock icon is absent

### Implementation for User Story 5

- [x] T051 [US5] Add `is_e2ee` field to `FileStatusDto` and `PairDto` in `crates/adagio-desktop/src/commands/pair.rs`; populate from `pair.e2ee_enabled` in the existing list commands
- [x] T052 [US5] Add `isE2eePair?: boolean` prop to `FilesScene` in `crates/adagio-desktop/src-ui/src/components/FilesScene.tsx`; show a lock icon (use existing `Icon` component, name `"shield"`) next to the breadcrumb root when `isE2eePair` is true
- [x] T053 [US5] Add lock indicator to `crates/adagio-desktop/src-ui/src/components/Sidebar.tsx` pair rows: when `pair.e2ee_enabled`, render a small lock icon beside the pair name
- [x] T054 [US5] Update `crates/adagio-desktop/src-ui/src/App.tsx` to pass `isE2eePair={activePair?.e2ee_enabled ?? false}` to `FilesScene`
- [x] T055 [US5] Extend `GetStatus` response in `crates/adagio-daemon/src/dispatcher.rs` to include `"e2ee_pairs": [...]` (list of pair IDs with `e2ee_enabled=true`) per the IPC contract in `contracts/e2ee-ipc.md`

**Checkpoint**: Lock icon visible only on E2EE pairs in both sidebar and file browser.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [x] T056 [P] Add `ZeroizeOnDrop` to `E2eeKeySet` and all intermediate key buffers in `crates/adagio-e2ee/src/keys.rs` and `provider.rs` using the `zeroize` crate
- [x] T057 [P] Add `criterion` benchmark `crates/adagio-e2ee/benches/crypto.rs` for AES-128-GCM encrypt/decrypt of a 10 MB buffer; assert ≤ 100 ms on `cargo bench`
- [x] T058 [P] Add structured `tracing` spans to all `E2eeProvider` methods and all `E2eeOcsClient` HTTP calls in `crates/adagio-e2ee/src/provider.rs` and `crates/adagio-nextcloud/src/e2ee_ocs.rs`
- [x] T059 Implement `E2eeProvider::disable()` (optional for v1.0): mark pair as non-E2EE, re-upload all files in plaintext, delete server metadata — call from UI "Disable E2EE" button in `PairsScene.tsx`
- [x] T060 [P] Run `cargo clippy --workspace -- -D warnings` and fix all E2EE-related warnings
- [x] T061 [P] Run `cargo fmt --all -- --check` and fix formatting
- [ ] T062 Run quickstart.md validation steps (US1–US6) against a live Nextcloud instance with E2EE app ≥ 2.0

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user story phases
- **Phase 3 (US1)**: Depends on Phase 2
- **Phase 4 (US2)**: Depends on Phase 3 (needs `encrypt_file` + `commit_metadata` infrastructure)
- **Phase 5 (US3)**: Depends on Phase 2; can overlap with Phase 4
- **Phase 6 (US4)**: Depends on Phase 5 (`pair` must be wired before `status` is meaningful end-to-end)
- **Phase 7 (US5)**: Depends on Phase 2 only — UI only, can run in parallel with Phase 3–6
- **Phase 8 (Polish)**: Depends on all preceding phases

### User Story Dependencies

| Story | Depends on | Can run in parallel with |
|-------|-----------|--------------------------|
| US1 (P1) | Phase 2 | — |
| US2 (P1) | US1 (needs encrypt path in propagator) | — |
| US3 (P2) | Phase 2 | US2 |
| US4 (P2) | US3 | US2 |
| US5 (P3) | Phase 2 | US1, US2, US3, US4 |

### Within Each Phase

- Tests MUST be written and FAIL before any implementation task in that phase begins
- Crypto primitives (`cipher.rs`, `keys.rs`) before `metadata.rs` before `provider.rs`
- `provider.rs` before dispatcher handlers
- Dispatcher handlers before CLI/Tauri commands
- CLI/Tauri commands before UI changes

---

## Parallel Opportunities

```bash
# Phase 1 — all four tasks in parallel
T001  T002  T003  T004

# Phase 2 — types can be written in parallel
T006  T007  T008  T009   # all types/traits
T011                      # migration test
T012  T013               # OCS client + its tests

# Phase 3 — tests in parallel, then crypto primitives in parallel
T014  T015  T016  T017   # all tests
T018  T019  T020         # cipher / keys / metadata in parallel
# T021 onwards sequentially (provider depends on all three)

# Phase 7 — fully parallel with Phases 3–6
T050  T051  T052  T053  T054  T055
```

---

## Implementation Strategy

### MVP First (US1 + US2 only)

1. Complete Phase 1 + Phase 2
2. Complete Phase 3 (US1 — encrypt + upload)
3. Complete Phase 4 (US2 — decrypt + browse)
4. **STOP AND VALIDATE**: upload and download one file end-to-end in E2EE mode
5. Demo: file on server is ciphertext; file on disk is plaintext

### Incremental Delivery

1. Phase 1 + 2 → foundation ready
2. Phase 3 (US1) → encrypt and upload works
3. Phase 4 (US2) → full read/write cycle works — **MVP shippable**
4. Phase 5 (US3) → second-device pairing works
5. Phase 6 (US4) → full CLI management works
6. Phase 7 (US5) → lock icon UX polish
7. Phase 8 → hardening + benchmarks

---

## Notes

- Mnemonic MUST NEVER appear in `tracing` logs or daemon responses other than the single `E2eeInit` response
- `unsafe` is not expected; if added, `// SAFETY:` comment is mandatory
- The `cms` crate may need to be evaluated for maturity; if insufficient, construct DER manually via `der` crate as fallback (document in ADR-017)
- All three platform CI targets (Linux, macOS, Windows) must pass — RSA/AES/PBKDF2 crates are pure-Rust and platform-independent
- Total task count: **62 tasks** across 8 phases
