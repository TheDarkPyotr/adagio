# Quickstart Validation: VFS On-demand Files (012)

---

## Prerequisites

```bash
cargo build -p adagio-daemon -p adagio-cli

# Verify FUSE3 is available (Linux)
ls /dev/fuse && fusermount3 --version

# Daemon running
./target/debug/adagio-daemon --config-dir ~/.config/ai.neuralagent.adagio/ &
sleep 1
```

---

## US1 — Browse all files without downloading

```bash
# Create/configure a VFS-mode pair
./target/debug/adagio pairs add --local ~/Nextcloud-VFS --remote / --vfs

# Check that the folder populates immediately
ls ~/Nextcloud-VFS | wc -l   # should equal remote file count

# Verify zero content bytes downloaded
./target/debug/adagio vfs status
# cloud_only_count = N (all files), cached_bytes ≈ 0

# Verify metadata size is small
du -sh ~/Nextcloud-VFS   # should be < 10 MB for any size library
```

**Pass**: All files visible, `cached_bytes ≈ 0`, `du -sh` < 10 MB.

---

## US2 — On-demand download

```bash
# Open a cloud-only file (any application or cat)
cat ~/Nextcloud-VFS/Documents/report.pdf > /dev/null

# Verify file is now locally available
./target/debug/adagio vfs status
# locally_available_count should be 1
# cached_bytes should equal file size

# Verify file can be read again offline (stop daemon)
pkill adagio-daemon
cat ~/Nextcloud-VFS/Documents/report.pdf > /dev/null  # should succeed (cached)
```

**Pass**: File readable after first open; locally-available count = 1.

---

## US3 — Pin for offline access

```bash
./target/debug/adagio-daemon ... &
./target/debug/adagio vfs pin ~/Nextcloud-VFS/Documents/

# Wait for background download
sleep 10
./target/debug/adagio vfs status   # pinned_count should = Documents/ file count

# Go offline
pkill adagio-daemon
ls ~/Nextcloud-VFS/Documents/   # should still list all files
cat ~/Nextcloud-VFS/Documents/notes.md   # should succeed
```

**Pass**: Files in Documents/ accessible offline after pinning.

---

## US4 — Evict cached files

```bash
./target/debug/adagio-daemon ... &

# Evict a single file
./target/debug/adagio vfs evict ~/Nextcloud-VFS/Documents/report.pdf
./target/debug/adagio vfs status   # cached_bytes decreases

# File still visible as placeholder
ls ~/Nextcloud-VFS/Documents/report.pdf   # should still show, size = original

# Re-open (re-downloads on demand)
cat ~/Nextcloud-VFS/Documents/report.pdf > /dev/null
./target/debug/adagio vfs status   # locally_available_count +1 again
```

**Pass**: Eviction reclaims storage; file remains visible and re-downloadable.

---

## US5 — Coexistence with copy-sync

```bash
# Existing copy-sync pair continues normally while VFS pair runs
./target/debug/adagio sync   # triggers both pairs
./target/debug/adagio status  # both pairs show in output, no errors

# Modify a file in copy-sync pair
echo "update" >> /home/luca/adagio-test/test.txt
./target/debug/adagio sync
# Only copy-sync pair uploads; VFS pair only updates metadata
```

**Pass**: Both sync modes complete independently; no interference.

---

## Quality gates

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
cargo test --workspace
npm run test  # 102+ UI tests pass
```
