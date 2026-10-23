# Local HTTP, certificate and renderer boundaries

Fairway keeps coursework HTTP, registered `.test` sites, optional HTTPS, phpMyAdmin and the mail reader available while limiting which origins and certificates enter those workflows.

## HTTP routing

Every generated website and phpMyAdmin listener has an explicit default server that closes unknown-host requests with Nginx status 444. `localhost` and `127.0.0.1` retain the appropriate service root, including configured custom ports. Registered `.test` names route only on the website listener; arbitrary names, unregistered `.test` names and attacker-controlled suffixes do not reach either root. Nginx's normal case and trailing-dot normalization still works. HTTP/1.0 without a hostname also reaches the rejection server. Malformed requests may be rejected with 400 before routing.

When any project uses HTTPS, its listener has a default server that rejects unknown TLS SNI names during the handshake. A valid SNI name followed by an unknown HTTP Host is also rejected. Known HTTPS sites retain their leaf certificate and normal routing. No HTTPS listener is added when no project enables HTTPS. Health checks use an explicitly allowed localhost address.

## Certificate profile and privileged Linux helper

New roots are self-signed Fairway CAs with critical basic constraints, zero subordinate-CA depth, certificate-signing/CRL-signing key usage, TLS-server extended key usage, a critical permitted DNS subtree of `.test`, and explicit exclusions for every IPv4 and IPv6 address. These constraints follow the [RFC 5280 name-constraint and basic-constraint model](https://www.rfc-editor.org/rfc/rfc5280#section-4.2.1.10). Leaf certificates contain the exact enabled site names plus `fairway.test`; they remain ordinary TLS server certificates.

Before a privileged Linux install, the helper reads the input once through a bounded regular-file handle, rejecting symlinks, special files and inputs above 32 KiB. It parses a single PEM certificate, rejects extra content and duplicate/unexpected extensions, checks the self-signature and current validity, and requires the complete restricted profile. A filename or common name alone cannot authorize installation. Removal may recognize the previous exact `.test`-only Fairway profile, including an expired certificate, so an upgrade can retire it; that profile cannot be newly installed.

Debian-style anchors have certificate-fingerprint filenames. Installing a replacement therefore retains the previous anchor until retirement. Removal checks certificate contents, including at the old fixed filename, and leaves another profile's certificate intact. Public anchors are readable by clients. The p11-kit fallback operates on a root-owned, private copy in `/run`, rather than reopening a user-controlled source path or temporary directory. Privileged subprocesses use fixed system executable paths and a cleared environment with a system-only PATH. System directories must be root-owned and not writable by other users.

## Existing profiles and trust migration

Startup keeps the previous valid Fairway CA and enabled HTTPS selections. Sites offers **Update certificate trust** when that CA needs the new policy, or when retiring the previous trust remains pending. Linux can retry the same update with `fairway site https DOMAIN on`, including a site already enabled.

A per-profile OS file lock prevents simultaneous certificate updates from replacing keys during another approval. The update creates a separate root key, root certificate and matching leaf under `tls/upgrade/tls`, then asks the OS to trust that staged root. The staged files and directories are flushed before trust installation; retirement state is flushed before publishing one `tls/active-v2` pointer after approval succeeds. Original material remains in `tls/`; it is not overwritten or silently discarded. Nginx configuration uses the selected generation's paths. The caller applies/reloads the site configuration before retiring the old root, identifying the exact certificate rather than deleting all roots with the same common name. `tls/retired-pending` records unfinished retirement.

| Interruption | Result and retry |
| --- | --- |
| Trust approval fails or is cancelled | Original generation and enabled sites remain selected; retry the update. |
| Approval succeeds, configuration/reload fails | Existing configuration and old trust remain usable; the replacement stays staged/selected and retirement remains pending. Retry applies the new configuration before cleanup. |
| Process exits between publication and cleanup | Both roots remain trusted; the pending marker survives for retry. |
| Old trust removal fails or is cancelled | The new root remains active; Sites retains the update action to retry exact-certificate removal. |

Existing and staged CA keys must match their certificates. Cached leaves are reused only when their names, signature, validity and private key match the selected CA; interrupted leaf writes and changed issuers trigger regeneration without replacing the root. Turning off the last HTTPS site still applies its HTTP-only configuration before attempting trust removal. Cancelled cleanup leaves HTTPS off with the existing Remove certificate trust retry action. Explicit trust removal clears any remaining saved HTTPS choices after successful cleanup.

## Renderer and mail

The main window capability contains `core:event:default`, replacing the wider `core:default`. Frontend native traffic uses those event APIs and the existing explicit Rust commands. Native window/tray/menu management stays in Rust. [Tauri capabilities](https://v2.tauri.app/security/capabilities/) and its [CSP configuration](https://v2.tauri.app/security/csp/) remain separate boundaries.

The production CSP explicitly denies objects, base-URI overrides and browser form navigation. Frame sources stay local; scripts stay self-only except for the exact hash of the mail interaction bridge, while IPC connections, inline styles and self/data images remain. React form submission handlers still run, and external opens continue through native commands. The mail iframe uses `sandbox="allow-scripts"` without same-origin, forms, popups or top-navigation permission. Email HTML is processed in an inert template to remove scripts and metadata and resolve CID images. A per-frame CSP permits only the exact hash of Fairway’s trusted link/keyboard bridge, inline styles and data images; email event handlers, remote resources and nested frames stay blocked. The parent accepts messages only from the current opaque preview frame and validates HTTP/HTTPS destinations again in native code. Click, middle-click and Enter open web links in the default browser; Escape and boundary Tab actions use the shared modal focus manager. The bridge source hash is pinned in both the preview policy and app CSP. Text mode and the existing native Open in browser action remain available.

## Verification and platform behavior

Rust regression fixtures test malformed/broad roots, DNS/IP/depth/usage constraints, self-signatures, wrong keys, bounded privileged reads, independent Linux anchors, migration cancellation/retry and stale leaf caches. The real runtime smoke tests unknown HTTP Host and absolute-URI hosts, missing Host, both localhost spellings, custom service ports, unknown SNI/Host over TLS, registered sites, phpMyAdmin, every bundled PHP version and the complete database/mail flow. All runtime files and service ports are disposable; tests do not install host trust or edit hosts entries.

`security-boundaries.spec.ts` applies the exact production CSP to the built app in Chromium/WebKit. It verifies migration retries, readable sandboxed mail, embedded data and CID images, blocked external resources and scripts, native external-open requests for clicked HTTP/HTTPS email links, rejected non-web links, React forms, blocked objects/base/form navigation, and continued sidebar navigation. Broader visual, mail, workspace and notice suites cover existing behavior. Native OS pickers, permission prompts, WebViews and trust stores can be exercised in a disposable account using the [optional platform checks](testing.md#optional-platform-checks).

Modern OpenSSL on this host rejects public DNS names, IPv4/IPv6 names and a subordinate CA chain under the new root, while accepting `.test` leaves. Apple's bundled LibreSSL 3.3.6 rejected the DNS/IP cases but accepted the subordinate chain despite the root's path-length constraint. Trust-anchor extension enforcement varies by verifier; the profile is not a claim that every client enforces root depth. Repeat the negative chain checks in supported native browsers/WebViews and Windows/Linux trust stores. `FAIRWAY_TEST_OPENSSL` can select the verifier used by the macOS chain regression. No browser/system trust settings are changed to hide these differences.
