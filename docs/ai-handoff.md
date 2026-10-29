# AI handoff

- Repository: `/Users/rioo/projects/fairway-xmp-personal`; personal fork
  `RizzRioo06/fairway-xmp`.
- Current branch: `feature/node-project-templates`.
- Latest implementation commit: `822c54334eaca48af137136c26d59d8f7afaf2ca`
  (Stage 4). This documentation-only handoff follows it; use `git log -1` for
  the latest checkout commit.
- **Automatic Node Project Creation Stages 1–4 are complete. Stage 5 has not started.**
  This is separate from the earlier completed Node Projects integration.

The six-template catalog supports plain Node JavaScript/TypeScript, Hono
TypeScript, Express JavaScript, and React + Vite JavaScript/TypeScript. Native
plans validate names, destinations and paired Node/npm without creating files.
Confirmed jobs generate privately, install pinned dependencies with lifecycle
scripts disabled, verify, publish without overwrite and register. Journals support
safe recovery. The wizard includes consent, actual progress, bounded logs,
cancellation, reload continuity and stopped-card handoff; servers never auto-start.

Relevant locations:

- Backend: `src-tauri/src/node_project_creation.rs` and `node_project_creation/`;
  assets/locks: `src-tauri/templates/node-projects/`.
- IPC: `src-tauri/src/desktop/node_project_creation_commands.rs`.
- Wizard/contracts: `src/node-project-creation/`; existing import/runtime/execution:
  `src/node-projects/` and `src-tauri/src/node_projects/`.
- [Architecture and safety](node-project-creation.md),
  [Stage 3 evidence](node-installation-verification.md),
  [Stage 4 evidence and file inventory](node-creation-wizard-verification.md).

Native creation/installation is macOS-first. Windows/Linux native behavior, other
CPU architectures, signed installers, actual updater replacement and power-loss
behavior remain unverified. One installation runs at a time; uncertain partial
staging is retained and retries require a fresh reviewed workspace. Automated
browser fixtures do not establish real native-dialog behavior.

**Real Mac wizard acceptance is still pending.** Next: collect the user's results
from [the isolated six-template manual guide](node-creation-manual-testing.md),
then perform Stage 5 verification when requested. Do not start Stage 5, merge main,
or add features as part of this handoff. Read [AGENTS.md](../AGENTS.md) first.
