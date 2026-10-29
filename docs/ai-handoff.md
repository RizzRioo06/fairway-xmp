# AI handoff

- Repository: `/Users/rioo/projects/fairway-xmp-personal`, personal fork
  `RizzRioo06/fairway-xmp`; branch `feature/node-project-templates`.
- Stage 5 follows checkpoint `c9e614c`. Use `git log -1` for the latest commit.
- **Automatic Node Project Creation Stages 1–5 implementation and automated
  verification are complete; real Mac acceptance remains partial.**

Actual retained manual logs confirmed `my-react-vite` failed because port 5173
was already occupied by the other Vite app. Revision 4 templates allow Vite
available-port fallback. Native run metadata reports the emitted loopback URL;
cards open that current address, label readiness unchecked, and retain stopped
manual URL editing. API starters report actual socket ports, including port zero.
Existing projects/configs are never rewritten. All six variants qualified with
Node 22.23.4/npm 10.9.9, 24.20.0/npm 11.19.0 and 26.12.1/npm 11.20.0 on macOS arm64.
Pinned dependency graphs and security/filesystem/process ownership rules remain.

- Backend: `src-tauri/src/node_project_creation/`, `node_projects/local_url.rs`,
  `node_projects/execution.rs`; assets: `src-tauri/templates/node-projects/`.
- UI: `src/node-projects/`; wizard: `src/node-project-creation/`.
- [Stage 5 evidence/limits](node-creation-stage5-verification.md),
  [architecture/safety](node-project-creation.md),
  [manual acceptance](node-creation-manual-testing.md).

One install runs at a time. Uncertain partial stages remain; retry needs a fresh
reviewed workspace. No automatic script execution, project migration or readiness
claim was added. Windows/Linux native behavior, other architectures, signing,
installer/updater replacement, power loss and remaining Mac GUI cases are unverified.
No GUI automation or installed-app/personal-project mutation was performed.

Next: user performs remaining isolated Mac acceptance and reports observations.
Do not merge main, push upstream, start PostgreSQL integration or advance scope
without a new request. Read [AGENTS.md](../AGENTS.md) first.
