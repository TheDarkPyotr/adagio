# Quickstart Validation: E2EE (013)

---

## Prerequisites

```bash
cargo build -p adagio-daemon -p adagio-cli

# Daemon running with a configured Nextcloud account
./target/debug/adagio-daemon --config-dir ~/.config/adagio/ &
sleep 1

# Confirm the Nextcloud server has end_to_end_encryption ≥ 2.0 enabled
./target/debug/adagio status
```

### Server requirements

The Nextcloud server must have:
1. **End-to-End Encryption app ≥ 2.0** — provides the `/api/v2/` metadata endpoints
2. **Server-side encryption (SSE) disabled** — SSE blocks WebDAV PUT to E2EE folders with 503

```bash
# Disable SSE (run once; survives container restarts)
docker exec --user www-data nextcloud php occ encryption:disable
# Expected output: Encryption is already disabled  — or: Encryption disabled

# Apply PHP patches (re-run after docker-compose up recreates the container)
docker cp /tmp/patch_e2ee.php nextcloud:/tmp/patch_e2ee.php
docker exec nextcloud php /tmp/patch_e2ee.php
# Expected: patch1=1 patch2=1 patch3=1
```

---

## US1 — Enable E2EE and upload an encrypted file

### Via the desktop UI (recommended)

1. Open the app → Sync Pairs → select an empty pair pointing at a new Nextcloud folder
2. Click **Init E2EE** — the mnemonic is displayed once; save it
3. Watch the daemon logs for: `INFO E2EE: locked upload cycle complete uploaded=N counter=2`
4. Copy a file into the local folder and click **Sync Now**

### Via the CLI

```bash
PAIR_ID=$(./target/debug/adagio pairs list --json | jq -r '.[0].id')

# Initialise E2EE (save the printed mnemonic!)
./target/debug/adagio e2ee init --pair "$PAIR_ID"
# Expected: mnemonic displayed once

# Write a test file; the E2EE runner picks it up within 30 s
echo "secret content" > ~/E2EE-Test/secret.txt

# Verify server stores only ciphertext (UUID-named blob, not "secret content")
curl -s -u luca:APP_PASSWORD \
  -H "User-Agent: Mozilla/5.0 (Linux) mirall/3.12.0 (adagio; linux-openssl)" \
  -X PROPFIND -H "Depth: 1" \
  "https://YOUR_SERVER/remote.php/dav/files/luca/e2ee-test-folder/" \
  | grep -o '<d:href>[^<]*</d:href>'
# Expected: UUID filenames (e.g. a3f8c2d1865246e3…), NOT "secret.txt"
```

**Pass**: File on server is UUID-named ciphertext; original name/content not visible in raw WebDAV.

---

## US2 — Read back the encrypted file

```bash
# Delete local copy, let the runner download + decrypt (or trigger manually)
rm ~/E2EE-Test/secret.txt
./target/debug/adagio sync --pair "$PAIR_ID"

cat ~/E2EE-Test/secret.txt
# Expected: "secret content"
```

**Pass**: Local file decrypted correctly after fresh download.

---

## US3 — Pair a second device

```bash
# On the second device (or a clean config dir):
MNEMONIC="word1 word2 ... word12"   # from US1

./target/debug/adagio e2ee pair --pair "$PAIR_ID" --mnemonic "$MNEMONIC"
# Expected: exit 0, key fingerprint printed

# Trigger a sync on the second device
./target/debug/adagio sync --pair "$PAIR_ID"
cat ~/E2EE-Test/secret.txt
# Expected: "secret content"
```

**Pass**: Second device decrypts correctly using only the mnemonic.

---

## US4 — CLI status and JSON output

```bash
./target/debug/adagio e2ee status --json
# Expected: valid JSON with enabled=true, metadata_version="2.0", counter>0

./target/debug/adagio e2ee status --json | jq '.encrypted_file_count'
# Expected: 1 (or current file count)
```

**Pass**: All CLI subcommands produce valid JSON and correct exit codes.

---

## US5 — Lock icon in UI

Open the desktop app → navigate to the E2EE pair in the file browser.

- Expected: lock icon visible next to pair name in sidebar and folder row
- Expected: files shown with real names (not UUIDs)

**Pass**: Lock icon visible; files shown with real names.

---

## US6 — Metadata version gate

```bash
# Simulate unknown metadata version by manually patching the DB
sqlite3 ~/.config/adagio/adagio.db \
  "UPDATE e2ee_folder_state SET metadata_version='99.0' WHERE pair_id='$PAIR_ID';"

# Attempt upload; the runner should abort immediately
echo "new content" > ~/E2EE-Test/new.txt
# Wait 30 s or trigger sync

# Expected daemon log: "unsupported metadata_ver 99.0" — no data written to server
```

**Pass**: Sync blocked with version error; no server mutation.

---

## Quality gates

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
cargo test --workspace
npm run test   # UI tests (from crates/adagio-desktop/src-ui)
```
