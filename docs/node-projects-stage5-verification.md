# Node Projects: Stage 5 integration verification

Date: 2026-10-28 (Asia/Bangkok). Branch: `feature/node-projects`.
Reviewed baseline: Stage 4 `ae34704254a8e31e6885af0f74fd8670a0c81f18`.

## Outcome and scope

Stages 1–4 were reviewed together: bounded inspection, durable registrations,
selected Node/npm preparation, owned foreground execution, logs and workspace.
Stage 4 MacBook acceptance was reported successful by the owner: import/runtime
selection, Start/Stop/Restart, live logs, two HTTP applications on 43101/43102,
independent control, server cleanup after Quit, intentional failure, and
simultaneous Nginx/PHP/MySQL and Node operation. That is owner-reported device
evidence, separate from the command-line checks below.

Stage 5 adds a narrow metadata-publication fix and regression coverage. It adds
no project-creation wizard, service, dependency, IPC contract or registry schema.
PHP controllers, global Node settings, licenses and copyright notices are unchanged.
No graphical automation or installed-app/settings changes were used for this stage.
Packaged PHP tools were accessed read-only with temporary homes/data and free ports.

## Confirmed defect and fix

Run history used the shared `config::atomic_write` helper, whose predictable
`last-run.fairway-tmp` staging filename could follow a pre-existing symlink.
A disposable regression test reproduced an imported fixture file being overwritten.

Only Node run metadata now uses a private, unpredictable, exclusively created
staging file in the same directory. The write is flushed before atomic replacement;
the directory is synced afterward. A drop guard removes staging files after failure.
An immediate linked metadata directory is rejected. Replacing an existing metadata
symlink replaces the link itself, preserving its target and permissions. The
existing registry replacement operation is reused, including Windows replacement
semantics; the shared PHP/configuration writer is unchanged.

Regression tests cover the old staging link, a destination link, a linked metadata
directory, private permissions, repeat publication, failed replacement and staging
cleanup. The reproducer failed before the fix and passed afterward. Run metadata
remains advisory history: a persisted PID is never adopted or signalled. If metadata
publication fails, active process ownership remains in memory; registry durability
is independent of this best-effort run history.

## Runtime-discovery investigation

A new opt-in test uses a disposable `.zshrc` containing a twenty-second sleep and
an isolated fnm inventory referencing a provisioned real Node installation. It
restores all process-local discovery variables afterward and executes no imported
package script.

Both existing eight-second shell probes time out: shell-environment discovery and
login-shell Node detection. Inspection completes in about 16 seconds with a timeout
warning and still lists the installed Node runtime. Replacing only the scratch
profile with a healthy one makes the next inspection recover. No defaults or
activation markers are created. This proves bounded failure and recovery under a
controlled slow profile; it does not identify the original MacBook timeout's cause.
Healthy controlled Node 22/24 discovery and preparation also pass. Inventory probes
and fnm metadata checks have their own bounds, so 16 seconds is not a universal
maximum for every installation. Personal startup files were not executed to
investigate the historical incident.

## Integration review

- Registration and status use separate app-owned storage. Corrupt Node configuration
  does not block PHP initialization, and Stop does not require a healthy registry.
  Existing tests cover malformed/oversized manifests, Unicode roots, duplicate
  directories, missing runtimes, stale revisions and failed-save recovery.
- Each child begins with a fresh environment. The selected absolute Node executable
  runs its paired npm CLI; inherited PATH, Node options, loader hooks and npm settings
  cannot silently replace it. No default, profile or activation marker is changed.
- A private inherited socket binds each supervisor to its owner. The supervisor
  remains process-group leader while terminating ordinary descendants, preventing
  group-ID reuse during cleanup. Crash EOF triggers cleanup; no name/port/PID search
  grants ownership. Runtime leases remain held until the helper is reaped.
- Start/Stop/Restart and runtime mutations share the native lifecycle reservation.
  Quit attempts both Node and PHP cleanup. Update installation stops Node before
  replacement and aborts if cleanup fails. Window/view visibility owns no process.
  PHP Start All/Stop All stays independent. These paths were reviewed against the
  production desktop wiring and exercised through native lifecycle/controller tests.
- Stream draining, memory bounds, rotating private logs, UTF-8 decoding, cursor gaps
  and persisted history remain covered. Project output is text, not renderer HTML.
  A running process is not an application-readiness promise.

## Automated evidence

All checks use this Stage 5 source unless explicitly noted. Provisioned runtimes:
Node `v22.23.4` and `v24.20.0`, each using its own npm. No runtime was installed or
selected globally by these checks.

| Check | Observed result |
| --- | --- |
| Rust all-features library | 178 passed; 7 opt-in cases excluded here and run separately |
| Opt-in real Node/runtime/lifecycle tests, serial | 7 passed, including crash cleanup, forced cleanup, rotation, persistence, PHP independence and discovery recovery |
| Optimized production executable as `--node-project-runner` | 4 native suites passed; no GUI initialization; fixture-owner crash case excluded because that transport exists only in the example |
| Rust terminal/no-default features | 154 passed; same 7 opt-in cases excluded; 0 doc-tests |
| Final metadata regression assertions | 2 passed after adding destination/directory-link checks |
| TypeScript units | 22 passed |
| Python regressions and packaging tests | 87 passed |
| Browser/native bridge regression | 530 distinct cases passed, 2 WebKit clipboard-permission cases skipped; all 4 native Node UI cases passed against final backend |
| `cargo fmt --check` | Passed |
| Clippy, all targets/all features, warnings denied | Passed |
| Unix CLI feature check | Passed |
| TypeScript and production Vite build | Passed |
| Optimized macOS desktop build | Passed |
| Release version consistency | Passed: 0.1.0 |

The full browser run was interrupted after 206 successful cases without a final
suite result. Rather than repeat it, 204 unaffected completed cases are excluded
from the resumed run; its two completed native Node cases are deliberately rerun
against the final backend. The resumed run completed with 326 passes and 2 skips. Together with the 204
preserved unaffected passes, this accounts for all 532 distinct cases: 530 passed,
2 skipped. The four final native Node cases cover Chromium/WebKit at both sizes. No interrupted suite is labelled a full-suite pass.

Normal regression commands and fixture setup are documented in
[Testing](testing.md) and [Node Projects](node-projects.md). To reproduce timeout
recovery independently, supply `FAIRWAY_TEST_NODE_RUNTIMES` and run:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib --all-features --locked \
  real_discovery_timeout_preserves_inventory_and_recovers -- --ignored --test-threads=1
```

Run opt-in discovery tests serially: they temporarily own their test process's
environment variables, never the terminal's/global environment.

## Packaging, limitations and merge decision

The production supervisor is a mode of the existing Fairway executable, dispatched
before single-instance/Tauri setup. No extra helper binary or bundled Node/npm is
required. The optimized executable was used directly in controlled supervisor tests.
Existing packaging/signing/version and updater boundary tests passed; no installer,
notarization or signed bundle replacement was performed on the owner's MacBook.

**Source integration is ready for review and merge into the personal fork's main
branch, subject to the owner's approval. A signed personal release is a separate
step.** The inherited release tooling defaults to `eimg/fairway-xmp` update feeds,
its public updater key, Apple team and app/package identity. Development configuration
has no updater feed. Do not publish a personal updater with those inherited defaults:
choose the personal feed/signing credentials and review identity/coexistence first.
This stage intentionally does not change release trust or installed-app identity.

Windows/Linux native builds and device behavior were not verified: only the
`aarch64-apple-darwin` target is installed. Platform guards were reviewed; Node
Projects remains explicitly macOS-only. Signed installers, notarization, actual
subsequent-version updater replacement/restart, native tray behavior, sleep/power-loss
and adversarially daemonized descendants remain outside this automated evidence.
The foreground contract excludes code that deliberately changes process group/session
or kills both app and supervisor; runtime isolation is not a sandbox for project code.
There are no remaining confirmed Node defects from this review. Historical timeout
causation remains unresolved.

For a small optional post-fix device check, use the existing [isolated manual
guide](node-projects-manual-testing.md): save a disposable registration, start/stop
it, Quit and reopen the same isolated profile, then confirm the selected runtime,
script, URL and retained output survive and nothing auto-starts. No repeat of the
whole accepted Stage 4 matrix is requested. If discovery stalls again, share the
elapsed time, exact redacted error and Node versions; do not send shell profiles,
whole environment dumps, credentials or personal configuration. Signed release
checks should use an owner-controlled disposable account after the fork's release
identity and trust configuration are deliberately set.
