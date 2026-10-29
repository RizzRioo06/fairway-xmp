# Node creation wizard — Stage 4 verification

Based on Stage 3 `4370baa8fdc5243bf2d370665a3caaddc62ce250`, on
`feature/node-project-templates`. Verification used command-line fixtures on
macOS arm64. No Computer Use, installed-app modification, global Node changes or
personal-project execution was performed. Stage 5 has not begun.

## Integration

The six-template wizard uses the native catalog and immutable native review plans.
Check setup validates the destination and selected Node/npm pair without writes.
Explicit consent calls the existing combined native Install operation; it does not
publish through Generate first. Success requires publication and registration;
View project focuses the stopped card and preserves the existing Start confirmation.

Job discovery lives outside the dialog. Navigation/reload reconnect to native
journals without cancelling or restarting work. Revision/run/cursor checks reject
stale updates and mixed logs. Output is bounded and rendered as text. Recovery is
explicit and offered only for verified completion/registration or supported legacy
registration retry. Partial/uncertain data is preserved.

Rust changes add catalog descriptions and optional actual progress phases only.
No resolver, supervisor, runtime lease, filesystem publication or recovery mechanism
was rewritten. A legacy-journal regression confirms omitted descriptions/progress
keep original plan digests valid. Direct dependency pins, scripts, Node/npm ranges,
lockfiles, licenses and provenance remain unchanged. Review now includes the native
warnings and discovery limits; React does not replace native validation with its
own semver/path implementation.

## Completed checks

| Check | Observed result |
| --- | --- |
| Rust default desktop/library suite with fixture feature | 197 passed; 19 opt-in tests ignored in this run |
| Native installation completion and registration recovery | 1 opt-in passed |
| Native cancellation, stale revisions, duplicate requests, lease and Quit | 1 opt-in passed |
| Native npm-exit/descendant cleanup before verification/publication/lease release | 1 opt-in passed |
| Native installer with PHP Start All/Stop All and updater cleanup reservation | 1 opt-in passed |
| TypeScript unit and IPC/model contracts | 31 passed |
| Python regression/packaging suite | 87 passed |
| Headless Chromium/WebKit desktop wizard plus Node Projects, Node tools, PHP settings, data browser, frontend recovery and workspace | 112 passed |
| Headless Chromium/WebKit wizard at 820×650 | 30 passed |
| Final review-note/consent/card-handoff minimum-size follow-up | 2 passed |
| TypeScript noEmit and Vite production build | Passed |
| Production JS excludes test-only creation fixtures | Passed |
| Rust formatting and diff whitespace | Passed |
| Strict Clippy all targets/all features, -D warnings | Passed |
| macOS release app and CLI executables | Built successfully; not installed/launched |
| Non-desktop CLI configuration | cargo check --no-default-features --features cli passed |
| Secret/generated-file and catalog-policy audit | No high-signal credential patterns or unintended generated files; only catalog descriptions changed |

Counts overlap; do not add repeated browser checks into a unique-test total.
The initial browser run found an ambiguous nested runtime label in the test/UI
contract. An explicit accessible name corrected it; all completed final browser
runs passed. The final output contains no known failing test.

Browser fixtures cover all six variants, both supported runtime selections, native
validation errors, approval gating, duplicate clicks, expired reviews, installation
failures, cancellation, interrupted jobs, registration retry, navigation/reload,
Unicode/log flooding, keyboard focus and stopped-card handoff. They simulate the
backend and do not prove native folder-picker or real network behavior.

The four native opt-ins use disposable Unicode/spaced paths and selected Node/npm,
plus offline/controlled npm fixtures. Packaged PHP binaries are read-only tools;
all service data is disposable. Existing Stage 3 genuine six-template/two-runtime
npm/build/HTTP qualification was not repeated: no locks, dependencies, scripts or
runtime requirements changed. Other opt-ins were left skipped where their existing
coverage was unaffected. The ordinary Rust suite still exercises filesystem races,
registry durability, legacy journals and existing PHP/Node safety rules.

Temporary evidence logs: `/tmp/fw-stage4-rust-final.log`,
`fw-stage4-native-recovery.log`, `fw-stage4-native-regression.log`,
`fw-stage4-unit-final.log`, `fw-stage4-python.log`,
`fw-stage4-browser-regression.log`, `fw-stage4-browser-minimum.log`,
`fw-stage4-review-notes.log`, `fw-stage4-clippy.log`, `fw-stage4-release.log`.
Logs/build/browser artifacts are disposable and are not committed.

## Remaining limitations

Real Mac acceptance is performed by the user using
[the isolated manual guide](node-creation-manual-testing.md). The user has now
reported successful generation and execution of Hono + TypeScript and React + Vite
TypeScript with Node 24.20.0; this is partial acceptance, not qualification of all
six variants or every cancellation/recovery/shutdown path. See the findings below.
Windows/Linux native generation/install and other CPU architectures are not
qualified. Signed installers, actual updater replacement, OS power loss and GUI
interaction were not tested. No Stage 5 release verification or new services were
started. The documented one-install-at-a-time, retained partial stages and explicit
fresh-plan retry policy from Stage 3 remain in force.

## Manual feedback for Stage 5

User-reported MacBook results, recorded 2026-10-29. These observations have not
been independently reproduced or investigated in this documentation-only update.
Stage 5 remains unstarted; fixes require subsequent authorization.

1. **Automatic Local URL.** Generated projects should receive a correct Local URL
   and a clickable Open action on their Node Projects cards. Derive it from the
   actual configured or detected host and port, not a universal assumed port.
   Preserve manual URL editing while stopped. A Running process does not establish
   application readiness; an Open action must not imply that readiness was verified.
2. **Concurrent Vite projects.** With `my-vite-typescript` running, starting
   `my-react-vite` exited with code 1. The user suspects a port conflict; logs have
   not confirmed the cause. When Stage 5 is authorized, inspect the generated Vite
   configuration, dev scripts, `server.port`, `server.strictPort` and failure logs.
   Reproduce with disposable fixtures before attributing the failure. Verify two
   generated Vite projects run concurrently on different available ports and each
   displayed Local URL matches its actual listener. Prefer clear conflict errors
   and safe configuration; never kill another application to free a port. Add
   focused regression tests for the confirmed cause and URL/port behavior.
3. **Node 26 compatibility.** Node 26 was rejected by the current template range
   `>=22.12.0 <23 || >=24.0.0 <25`; Node 24.20.0 successfully generated and ran the
   two tested variants. Rejection matches the current support policy and is not
   evidence that Node 26 is incompatible. Review each template separately and
   qualify dependency installation, builds and execution before relaxing engine
   restrictions. Keep compatibility errors understandable; do not advertise
   unqualified support or weaken dependency/runtime validation.

Priority once authorized: reproduce reported failures, distinguish confirmed bugs
from hypotheses and requested improvements, then implement minimal tested fixes.
The user retains real Mac GUI acceptance. No Computer Use, installed-app changes,
global runtime changes or personal-project execution is authorized by this record.

## Changed files

### Created

- `docs/node-creation-manual-testing.md`
- `docs/node-creation-wizard-verification.md`
- `src/node-project-creation/CompletionRecovery.tsx`
- `src/node-project-creation/CreationReview.tsx`
- `src/node-project-creation/CreationWizard.tsx`
- `src/node-project-creation/CreationWorkspace.tsx`
- `src/node-project-creation/ProjectConfiguration.tsx`
- `src/node-project-creation/SetupProgress.tsx`
- `src/node-project-creation/TemplateSelection.tsx`
- `src/node-project-creation/creation.css`
- `src/node-project-creation/model.ts`
- `src/node-project-creation/useCreationJobs.ts`
- `src/ui-test/node-creation.ts`
- `tests/node-creation-model.test.ts`
- `tests/ui/node-creation.spec.ts`

### Modified

- `docs/node-project-creation.md`
- `src-tauri/src/node_project_creation.rs`
- `src-tauri/src/node_project_creation/catalog.json`
- `src-tauri/src/node_project_creation/catalog.rs`
- `src-tauri/src/node_project_creation/installation.rs`
- `src-tauri/src/node_project_creation/installation_native.rs`
- `src-tauri/src/node_project_creation/installation_tests.rs`
- `src-tauri/src/node_project_creation/tests.rs`
- `src/node-project-creation/types.ts`
- `src/node-projects/NodeProjectsPage.tsx`
- `src/ui-test/backend.ts`
- `src/ui-test/node-projects.ts`
