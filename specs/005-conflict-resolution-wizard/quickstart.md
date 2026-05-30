# T052 Validation: Conflict Resolution Wizard (005)

End-to-end manual validation covering all three conflict types, all three
resolutions, the multi-conflict queue, dismiss-and-return, and the no-duplicate
invariant.

---

## Prerequisites

| Item | Command / check |
|------|----------------|
| App builds | `cd crates/adagio-desktop && cargo build -p adagio-desktop 2>&1 \| grep -c error` → 0 |
| Rust tests pass | `cargo test --lib -p adagio-core -p adagio-desktop` → all green |
| React tests pass | `cd crates/adagio-desktop/src-ui && npm run test` → all green |
| One sync pair configured | Nextcloud instance reachable; local root exists; at least one file synced |

Set two shell variables you will reuse throughout:

```bash
LOCAL="$HOME/Adagio"          # replace with your actual pair local_root
NC="https://cloud.example.com" # replace with your Nextcloud URL
NC_USER="alice"                # replace with your Nextcloud username
```

Start the app in dev mode and leave it open for all scenarios:

```bash
cd crates/adagio-desktop
npm run tauri:dev
```

---

## How to trigger a sync cycle

The default scan interval is 2 h. Use the in-app shortcut instead:

**Settings → Sync Pairs → [your pair] → Sync Now**

Or via the keyboard shortcut **Ctrl+P** (toggles pause/resume — unpause while
syncing triggers an immediate cycle).

Alternatively, shorten `scan_interval_secs` to `10` in `~/.config/adagio/config.json`
before running validation, then restore it afterwards.

---

## Scenario A — File conflict, Keep Local Version

### Setup

```bash
# 1. Confirm the file is already synced.
cat "$LOCAL/conflict-test.txt"   # should exist

# 2. Edit the local copy.
echo "local version $(date)" > "$LOCAL/conflict-test.txt"

# 3. Immediately (before the next cycle) edit the same file on the server
#    via the Nextcloud web UI, adding text so the size/mtime differ visibly.
#    Use: Files → conflict-test.txt → … → Details → Edit in text editor
#    (or the Nextcloud Files app on another device)
```

### Trigger sync

Click **Sync Now** in Settings → Sync Pairs.

### Expected UI state

- [ ] A numbered badge (e.g. **1**) appears in the top-right of the Chrome toolbar.
- [ ] Badge uses `data-testid="conflict-badge"`.
- [ ] Clicking the badge opens the Conflict Wizard overlay (≤ 100 ms).

### Wizard content checks

- [ ] Header reads **"Conflict 1 of 1"**.
- [ ] File name shown: **conflict-test.txt**.
- [ ] Path shown below filename when there is a directory prefix (not shown for root-level files).
- [ ] **"This device"** panel shows local file size and local mtime.
- [ ] **"Cloud copy"** panel shows server file size and server mtime — values differ from local.
- [ ] Three buttons present: **Keep Local Version**, **Keep Server Version**, **Keep Both**.
- [ ] No folder-conflict kind label is shown (this is a file conflict).
- [ ] No impact warning is shown.

### Resolution

Click **Keep Local Version**.

- [ ] Spinner overlay appears immediately.
- [ ] Wizard closes when the transfer completes.
- [ ] Badge disappears (count → 0).

### Post-resolution file checks

```bash
# Local file is unchanged.
cat "$LOCAL/conflict-test.txt"   # must contain "local version …"

# Server now matches local — download and compare.
curl -s -u "$NC_USER:$PASSWORD" \
  "$NC/remote.php/dav/files/$NC_USER/conflict-test.txt" \
  | grep "local version"         # must match

# No timestamp-suffix duplicates anywhere.
find "$LOCAL" -name "conflict-test*" | sort
# Expected: exactly one file — conflict-test.txt
```

---

## Scenario B — File conflict, Keep Server Version

### Setup

```bash
# 1. Edit local copy.
echo "local again $(date)" > "$LOCAL/conflict-test.txt"

# 2. Edit server copy via Nextcloud web UI.
#    Write something distinct, e.g. "server version B".
```

Trigger sync and open the wizard.

### Wizard checks (delta from Scenario A)

- [ ] Both panels show updated mtime/size values reflecting the new edits.

### Resolution

Click **Keep Server Version**.

- [ ] Spinner, then wizard closes, badge disappears.

### Post-resolution checks

```bash
# Local file now holds the server content.
cat "$LOCAL/conflict-test.txt"   # must contain "server version B"

# No duplicate files.
find "$LOCAL" -name "conflict-test*" | sort
# Expected: exactly one file.
```

---

## Scenario C — File conflict, Keep Both (no-duplicate invariant)

### Setup

```bash
# 1. Edit local copy.
echo "local C $(date)" > "$LOCAL/conflict-test.txt"

# 2. Edit server copy (write "server C") via Nextcloud web UI.
```

Trigger sync and open the wizard.

### Resolution

Click **Keep Both**.

- [ ] Spinner, then wizard closes, badge disappears.

### Post-resolution checks

```bash
find "$LOCAL" -name "conflict-test*" | sort
```

Expected output — exactly **two** files:

```
/path/to/Adagio/conflict-test.txt
/path/to/Adagio/conflict-test (conflicted copy from <HOSTNAME> YYYY-MM-DD HH-MM-SS).txt
```

Verify each invariant:

```bash
# 1. Original path holds the server copy.
cat "$LOCAL/conflict-test.txt"
# → "server C"

# 2. Conflict copy holds the local copy.
COPY=$(find "$LOCAL" -name "conflict-test (conflicted copy*" | head -1)
cat "$COPY"
# → "local C …"

# 3. Naming format — no timestamp suffix with underscores or random digits.
echo "$COPY" | grep -P '\(conflicted copy from .+ \d{4}-\d{2}-\d{2} \d{2}-\d{2}-\d{2}\)'
# → must match

# 4. No third file was created.
find "$LOCAL" -name "conflict-test*" | wc -l
# → 2

# 5. Conflict copy also exists on the server (uploaded back).
curl -s -o /dev/null -w "%{http_code}" -u "$NC_USER:$PASSWORD" \
  "$NC/remote.php/dav/files/$NC_USER/$(basename "$COPY")"
# → 200
```

---

## Scenario D — Multiple conflicts (queue + step counter)

### Setup

Create three simultaneous file conflicts:

```bash
for name in alpha.txt beta.txt gamma.txt; do
  echo "local $name" > "$LOCAL/$name"
done
# Then in the Nextcloud web UI, edit all three files before triggering sync.
```

Trigger sync.

### Expected UI state

- [ ] Badge shows **3**.

### Open wizard and step through

1. Click badge.
   - [ ] Header: **"Conflict 1 of 3"**, filename: **alpha.txt** (or whichever is first).
2. Click **Keep Local Version**.
   - [ ] Wizard auto-advances. Header: **"Conflict 2 of 3"**, filename: **beta.txt**.
   - [ ] No flicker or full re-render of the overlay.
3. Click **Keep Server Version**.
   - [ ] Wizard auto-advances. Header: **"Conflict 3 of 3"**, filename: **gamma.txt**.
4. Click **Keep Both**.
   - [ ] Wizard closes.
   - [ ] Badge disappears.

### Post-resolution

```bash
find "$LOCAL" -name "gamma*"
# → gamma.txt + gamma (conflicted copy from …).txt
```

---

## Scenario E — Dismiss and return (badge persistence)

### Setup

Create one file conflict (as in Scenario A).

Trigger sync. Badge shows **1**.

### Dismiss

1. Click badge to open wizard.
2. Click the **×** dismiss button (top-right of dialog).
   - [ ] Wizard closes immediately.
   - [ ] `resolveConflict` was **not** called (no spinner appeared).
   - [ ] Badge still shows **1** — count did not reset to 0.

### Re-open

3. Click the badge again.
   - [ ] Wizard reopens with **the same conflict** — same filename, same metadata.
   - [ ] Header: **"Conflict 1 of 1"**.

### Clean up

4. Resolve the conflict via any resolution button.
   - [ ] Badge disappears after resolution completes.

---

## Scenario F — Folder rename conflict (renamed_both_sides)

> **Note**: This scenario requires a Nextcloud instance where you can rename
> folders on the server, and the propagator must detect the rename.

### Setup

```bash
# Create and sync a folder first.
mkdir "$LOCAL/Projects"
touch "$LOCAL/Projects/readme.txt"
# Trigger sync, confirm folder appears on server.

# Then simultaneously:
mv "$LOCAL/Projects" "$LOCAL/Work"          # rename locally
# AND rename the same folder to "Projects 2026" on the server via web UI.
```

Trigger sync.

### Expected UI state

- [ ] Badge shows **1**.
- [ ] Wizard opens: conflict kind label reads **"Renamed on both sides"**.
- [ ] Folder icon or `is_dir` label visible.
- [ ] Header path shows folder name, not a filename with extension.

### Resolution options

- **Keep Local Version** → folder on server renamed to "Work".
- **Keep Server Version** → local folder renamed to "Projects 2026".
- **Keep Both** → both "Work" and "Projects 2026" exist as siblings.

Verify your chosen resolution:

```bash
ls "$LOCAL"   # confirm which folder name(s) are present
```

---

## Scenario G — Delete-vs-content conflict (deleted_with_content) + impact warning

> This scenario verifies FR-013 and FR-014 (impact warning before confirm).

### Setup

```bash
# Sync a folder with content.
mkdir -p "$LOCAL/Archive"
echo "old file" > "$LOCAL/Archive/old.txt"
# Trigger sync, confirm it appears on server.

# Then simultaneously:
rm -rf "$LOCAL/Archive"              # delete locally
# AND add a new file to Archive/ on the server via Nextcloud web UI.
```

Trigger sync.

### Expected UI state

- [ ] Badge shows **1**.
- [ ] Wizard conflict kind label: **"Deleted locally / New content on server"**.
- [ ] `data-testid="impact-warning"` element is present in the dialog.
- [ ] Impact warning text mentions files will be restored to disk.

### Resolution — Keep Server Version

Click **Keep Server Version**.

```bash
ls "$LOCAL/Archive/"
# → new file added on server must be present locally
```

### Resolution — Keep Local Version

(Re-create the conflict and choose Keep Local instead.)

```bash
ls "$LOCAL/Archive/"
# → directory must not exist (or be empty, depending on your server state)
```

---

## Scenario H — "Keep Both" collision guard

If a conflict copy file already exists (e.g., from Scenario C), triggering
**another** "Keep Both" resolution on the same file path must **not** silently
overwrite the existing copy.

```bash
# Verify after a second "Keep Both" on conflict-test.txt:
find "$LOCAL" -name "conflict-test*" | wc -l
# → 3 (original + first copy + second copy with a later timestamp or extra suffix)
# Specifically: no two files with identical names.
find "$LOCAL" -name "conflict-test*" | sort | uniq -d
# → (empty — no duplicates)
```

---

## Scenario I — App restart preserves pending conflicts

### Setup

Leave one conflict unresolved (e.g., dismiss after Scenario E, step 2).

### Restart

```bash
# Stop the dev server (Ctrl+C) and restart.
npm run tauri:dev
```

- [ ] Badge still shows the pending count on startup (loaded from journal via `list_conflicts`).
- [ ] Clicking badge shows the same unresolved conflict.

---

## Quality gate checklist (automated)

Run before declaring T052 complete:

```bash
# Rust
cargo clippy -- -D warnings        # 0 warnings
cargo fmt --all --check            # passes
cargo test --lib -p adagio-core    # 125 tests pass
cargo test --lib -p adagio-desktop # 37 tests pass

# Frontend
cd crates/adagio-desktop/src-ui
npm run check                      # tsc --noEmit → 0 errors
npm run test                       # 91 tests pass
```

---

## Pass criteria

T052 passes when all boxes in Scenarios A–I are checked and all automated
quality gates are green. If any box fails, note the failing scenario and step
number in the PR description.
