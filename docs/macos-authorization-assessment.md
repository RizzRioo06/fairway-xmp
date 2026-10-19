# macOS authorization assessment — 2026-10-19

Status: assessment plus an opt-in native HTTPS-trust prototype, prepared after RC 1 at the owner's request for a local Mac comparison build. Standard builds keep the existing authorization behavior. The owner will decide separately whether the proposed work warrants another RC.

## Findings in the current source

| Operation | Current implementation | Meaning for the prompts |
| --- | --- | --- |
| Add, rename or remove a site address | `hosts::ensure` compares the desired hosts block, then launches the existing short-lived Fairway helper through AppleScript `with administrator privileges` when a change is necessary. | This path requests administrator authorization. There is no Touch ID setting in Fairway's current implementation. Unchanged addresses already skip elevation. |
| First HTTPS trust setup | `tls::ensure_trust` generates certificates without elevation, checks existing trust, then invokes `security add-trusted-cert` with SSL trust in the current user's login keychain. | This is user certificate trust, not administrator access to the system keychain. Certificate insertion/keychain access and trust-setting authorization can involve separate system interactions. |
| Subsequent HTTPS sites | Existing verified CA trust is reused. | Per-site certificate generation does not itself require another trust approval. Removing the last HTTPS site removes trust; setting up HTTPS again can therefore require approval again. |

Sources: [hosts implementation](../src-tauri/src/hosts.rs), [TLS implementation](../src-tauri/src/tls.rs), and [desktop site commands](../src-tauri/src/desktop.rs). The user's report describes a password prompt followed by a Touch ID-capable prompt during HTTPS setup. That is consistent with separate keychain/trust interactions; the exact responsibility of each dialog is **not established** without its wording and a controlled reproduction. No fresh trust-store mutation was performed for this assessment.

Read-only probes on the available Mac (reported by `sw_vers` as macOS 27.2, build 26B5112a) found Touch ID available through `LAContext.canEvaluatePolicy`, and distinct authorization rules for `system.privilege.admin` and `com.apple.trust-settings.user`. The latter delegates to `entitled-session-owner-or-authenticate-session-owner`; the administrator right is a separate user authorization. These observations support treating the operations separately. They do not establish which authentication methods either actual operation will offer. The initial sandboxed probes could not contact the system services; the successful probes ran outside that restricted context. Neither probe evaluated an authentication policy, displayed a prompt, or modified authorization rules. This device snapshot does not replace the owner's separately reported RC test OS details.

## What the native APIs can and cannot improve

Apple's [Authorization Services](https://developer.apple.com/documentation/security/authorization-services?language=objc) delegates authentication to the system security agent and allows authentication mechanisms to evolve. It does not promise that every requested right can use Touch ID.

[Local Authentication](https://developer.apple.com/documentation/localauthentication/lapolicy/deviceownerauthentication) can offer Touch ID, Apple Watch and password fallback on supported Macs. A successful local authentication result does not itself give an ordinary process permission to modify `/etc/hosts`. Adding an independent biometric dialog before the existing command could add friction without replacing its authorization.

Apple documents a separate authentication interaction for [per-user certificate trust changes](https://developer.apple.com/documentation/security/sectrustsettingssettrustsettings%28_%3A_%3A_%3A%29). Replacing the `security` subprocess with Security framework calls may improve integration and control of the sequence, but fewer prompts or Touch ID availability must be demonstrated on the supported OS versions. User trust must remain user trust; moving the CA to system-wide trust would unnecessarily broaden its scope.

## Recommended next experiment

Investigate **native per-user HTTPS trust setup first**, independently of hosts elevation. The opt-in `native-macos-trust` Cargo feature now adds the same public CA certificate to the login keychain with `SecItemAdd`, then applies SSL-server-only trust in the user domain with `SecTrustSettingsSetTrustSettings`. It decodes the actual PEM used by verification rather than relying on a potentially stale DER sidecar. Existing trust verification and compatible certificate/trust removal are retained. It adds no preliminary Local Authentication dialog, stored-password keychain unlock, administrator helper or authorization-policy changes. A duplicate public certificate left after cancellation is accepted for retry; other insertion errors stop before requesting trust. The trusted marker is written only after the existing verification succeeds. Cancellation and other errors retain the failed operation's context.

The four optional Mac dependencies already existed in Cargo.lock; their versions and checksums are unchanged. Ordinary builds leave the feature off, and Windows/Linux keep their existing trust implementation. The local comparison build enables it explicitly and labels its window **Fairway XMP — Native HTTPS Prototype**. It uses the ordinary Fairway profile and therefore must be run after quitting the installed app; it does not replace the installed RC 1 bundle. A Developer ID signature identifies the local app, but this is a test build rather than a notarized release candidate.

For a meaningful prompt comparison, existing CA trust must first be absent. In the app, turn HTTPS off for all sites, choose **Remove certificate trust**, then enable HTTPS for one existing site. Merely enabling another site while the CA is already trusted intentionally skips setup. Compare prompt count, wording and authentication methods; fewer prompts are not yet established. Cancel a setup and retry, enable a second HTTPS site, then remove trust when unused. Address/hosts changes remain on the existing administrator-password path. These steps affect the normal profile's local CA trust; no automatic trust removal or setup is run during build/verification.

Use a disposable logged-in test account and a dedicated test CA. Record the exact dialog text, requesting process, prompt count and available authentication methods for the current implementation and prototype. Cover an unlocked and locked login keychain, clean and already-trusted CA, cancellation at each prompt, Touch ID unavailable/disabled, a standard user account, a second HTTPS site, trust removal and re-creation. Check the oldest supported macOS release as well as the owner's macOS 27 beta; the beta alone cannot establish shipping behavior. Remove only the dedicated test CA afterward.

Proceed only if the prototype gives a measured improvement while preserving trust scope, cancellation/retry behavior, verification and removal. The expected cost is a small native integration plus moderate device verification; prompt reduction remains an open result, not an implementation promise.

## Hosts changes: a larger follow-up

A signed helper registered through Apple's [SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice) is the modern direction if repeated hosts approvals become a significant annoyance. It adds helper registration/approval, authenticated app-to-helper communication, signing/package integration, update compatibility and complete uninstall cleanup. It should accept only validated Fairway domain updates, never arbitrary commands or paths. User approval of helper registration is still required; it should not be presented as a guaranteed single Touch ID prompt.

This is substantially more work than replacing the password dialog. Keep it outside a small RC polish pass unless the narrower experiment demonstrates that it is needed. Batching hosts changes and certificate trust into one root operation would mix separate permission scopes and is not the recommended shortcut.

## Release recommendation

Keep RC 1 and its published artifacts unchanged. The accompanying welcome-page proposal is independent visual/content polish. The native trust feature remains an opt-in test proposal and needs the user's real prompt comparison before any decision to enable it normally. Any welcome-page or authorization implementation chosen for another RC requires a fresh candidate build and acceptance; existing RC 1 hashes do not cover it.
