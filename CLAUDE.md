<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan at
`specs/017-release-packaging/plan.md`.

Key artifacts for this feature:
- Spec: `specs/017-release-packaging/spec.md`
- Research & decisions: `specs/017-release-packaging/research.md`
- Pipeline contract: `specs/017-release-packaging/contracts/release-workflow.md`
<!-- SPECKIT END -->

## Current status (branch `013-e2ee-encryption`)

Feature 013 (E2EE) is substantially complete. The full init sequence, metadata sync, and upload/download cycle are implemented. One server-side blocker remains: Nextcloud's SSE (server-side encryption) must be disabled before WebDAV uploads to E2EE folders work.

### What works
- E2EE init: 6-step sequence (RSA-2048 key gen, PKCS#10 CSR, BIP-39 mnemonic, folder ID resolution, V2 metadata POST with lock token, mark encrypted)
- Metadata sync: GET `/api/v2/meta-data/{id}` parses double-encoded JSON correctly
- Upload cycle: `upload_locked` — lock → encrypt → WebDAV PUT with `e2e-token` → PUT metadata → unlock
- Download cycle: WebDAV GET uuid → AES-128-GCM decrypt → write local file
- E2EE runner: 30 s interval + manual trigger via Sync Now button
- UI: pair row shows E2EE badge, mnemonic displayed after init

### Remaining to validate (T062)
1. `docker exec --user www-data nextcloud php occ encryption:disable` — disables SSE
2. Re-run E2EE init from the UI
3. Copy a file into the local E2EE folder; verify upload log: `INFO E2EE: locked upload cycle complete uploaded=1`
4. Check Nextcloud web via `curl` with `mirall/3.12.0` UA that UUID-named ciphertext is present

### Critical protocol notes
- All metadata/lock calls use `/api/v2/` endpoint (not v1)
- `X-NC-E2EE-COUNTER` is an HTTP **header** on lock POST
- POST metadata requires both `e2e-token` + `X-NC-E2EE-SIGNATURE` headers
- V2 `meta-data` response field is a JSON string — needs `serde_json::from_str` before struct deserialisation
- `userId` in `build_outer`/`parse_outer` is the Nextcloud **username** (e.g. `"luca"`), not the adagio account UUID

### Server patches (re-apply after `docker-compose up` recreates the container)
```bash
docker cp /tmp/patch_e2ee.php nextcloud:/tmp/patch_e2ee.php
docker exec nextcloud php /tmp/patch_e2ee.php
# Expected: patch1=1 patch2=1 patch3=1
```
The patch file lives at `/tmp/patch_e2ee.php` on the dev machine.

## Next feature: 014 — LAN-peer protocol

Direct device-to-device sync over LAN without going through the Nextcloud server. Start with `speckit-specify` once 013 is validated.
