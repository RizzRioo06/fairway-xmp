# Node creation wizard — isolated Mac acceptance

Use only a disposable debug profile. Do not replace or update installed Fairway,
change a global Node default, or select a normal project folder. These checks
complete real Mac acceptance after Stage 5 automated verification.

## Prepare and launch

1. Manually quit your installed Fairway application. Open Terminal in
   `/Users/rioo/projects/fairway-xmp-personal`. Confirm the branch is
   `feature/node-project-templates` and the working tree is clean.
2. Use the build prerequisites in [Contributing](../CONTRIBUTING.md). Node 24 is
   the frontend build tool; the wizard separately supports installed Node
   22.12+ in major 22, Node 24 or Node 26 with paired npm 10/11. Do not change
   fnm defaults
   to make a missing runtime appear. If Cargo is missing, the optional temporary
   toolchain instructions in [the earlier manual guide](node-projects-manual-testing.md)
   can be used if those directories still exist.
3. Run the following in the same Terminal. The copy reads installed resources;
   all development writes use the new directory.

```sh
cd /Users/rioo/projects/fairway-xmp-personal
export FAIRWAY_CREATION_TEST="$(mktemp -d /tmp/fairway-creation-manual.XXXXXX)"
mkdir -p "$FAIRWAY_CREATION_TEST/home" "$FAIRWAY_CREATION_TEST/data" \
  "$FAIRWAY_CREATION_TEST/Projects café 日本"
cp -cR "/Applications/Fairway XMP.app/Contents/Resources/runtime" "$FAIRWAY_CREATION_TEST/runtime"
printf 'Test parent folder: %s/Projects café 日本\n' "$FAIRWAY_CREATION_TEST"
```

If APFS cloning is unsupported, use `cp -R` with the same paths instead. A full
copy needs roughly 800 MB. If the source runtime or tools are missing, stop and
report the error. Run `npm ci` in the checkout only if its dependencies are absent.
Never run runtime-preparation scripts against installed app resources.

```sh
env FAIRWAY_DEV_HOME="$FAIRWAY_CREATION_TEST/home" \
    FAIRWAY_DEV_DATA="$FAIRWAY_CREATION_TEST/data" \
    FAIRWAY_RUNTIME_DIR="$FAIRWAY_CREATION_TEST/runtime" \
    ZDOTDIR="$FAIRWAY_CREATION_TEST/home" \
    CARGO_TARGET_DIR="$FAIRWAY_CREATION_TEST/build" \
    npm run tauri -- dev --no-watch --config '{"identifier":"dev.fairway.nodecreation.manual","app":{"windows":[{"title":"Fairway Node Creation Test","width":1080,"height":760,"minWidth":820,"minHeight":650}]}}'
```

Expect **Fairway Node Creation Test**. Its Environment document root must start
with the printed `/tmp/fairway-creation-manual.../home/`. If it shows your normal
home directory, quit this debug instance before proceeding. Overrides are debug
only: do not use `--release` or relaunch through Finder/Dock. After Quit, repeat
this Terminal launch with the same variables. Do not kill an unknown program if
port 1420 or another service port is occupied.

## Create all six variants

Repeat these steps for each row. Use the printed **Projects café 日本** directory
in the native picker (Command+Shift+G accepts the absolute printed path).

| Template | Suggested project name | Explicit dev-server check |
| --- | --- | --- |
| Plain Node.js JavaScript | accept-node-js | http://127.0.0.1:3000/health |
| Plain Node.js TypeScript | accept-node-ts | http://127.0.0.1:3000/health |
| Hono API TypeScript | accept-hono-ts | http://127.0.0.1:3000/health |
| Express API | accept-express-js | http://127.0.0.1:3000/health |
| React + Vite JavaScript | accept-react-js | http://127.0.0.1:5173/ |
| React + Vite TypeScript | accept-react-ts | http://127.0.0.1:5173/ |

1. Open **Node Projects → Create New Project**. Confirm six descriptions and the
   correct JavaScript/TypeScript labels. Choose the row, then Configure project.
2. Enter its name, choose the disposable parent and an installed supported Node
   runtime. Alternate available qualified Node 22/24/26 versions and record the
   exact one tested. **Check setup** must show the selected exact Node and paired npm.
3. Review the final folder, files, template revision, dependencies and notes.
   Confirm Create and install is disabled until you check the approval box.
   npm downloads packages, so this step needs network access; installation scripts
   stay disabled and no development server starts.
4. Approve and create. Observe actual phases and npm output. Fast verification,
   publication and registration phases may pass between polls. Success must say
   registered and ready; View project must focus a **Stopped** card.
5. Inspect the generated folder: source, manifest, lockfile, README and dependencies
   should exist. Confirm it is inside the disposable parent, never an old project.
6. Explicitly click the card's **Start**, review the existing script confirmation,
   and confirm Start project. Use the card's reported Local URL and Open URL
   action; compare it with the logs and check the HTTP response. Running means
   the process is alive; the HTTP response checks readiness.
7. Stop the card before starting the next row (API templates share port 3000;
   Vite templates prefer 5173 and fall back to an available port). Confirm the
   address becomes unreachable. Do not stop unrelated listeners if a port is already occupied.

## Consent, continuity and failure checks

- Try an invalid name (`Bad Name`) and then an already-created name. They must
  fail before generation; existing files must remain unchanged.
- If an unsupported installed runtime is listed, Check setup must reject it.
  Do not install or change a runtime just to force this test.
- Go Back from a valid review and edit the name/runtime: it must require another
  Check setup and fresh confirmation. Leave a review for over ten minutes:
  Refresh review must require consent again, not silently create a project.
- During an installation, Hide setup, visit Environment, then return through
  **View setup**. Reload the debug WebView only if convenient. It must reconnect
  to the same native job, with no duplicate project or automatic restart.
- Start another setup with a new disposable name and try Cancel setup while
  installation is active. Expect cancellation/cleanup or an honest publication
  already-started message. Fast installs may finish before cancellation; record
  this as not observed. Retained staging data is intentional, not a deletion cue.
- A normal network failure should show an error and keep files. Do not alter
  system networking, npm settings, journals or permissions to manufacture failure.
  Registration retry/integrity/interruption paths already have controlled automated
  coverage. If one occurs naturally, Retry Registration should appear only when
  supported, and the published folder must be kept.
- Tab/Shift+Tab should stay in the dialog. Escape/Close should return focus.
  Resize to 820×650: all controls must remain reachable by scrolling.
- Quit the **debug instance** explicitly. Relaunch with the same environment:
  registered cards should persist and servers must remain stopped. If Quit happens
  during installation, an unfinished job must not resume automatically.

Record template, exact Node/npm, creation result, explicit HTTP/Stop result and
any error text. Report unobserved cancellation, reload or expiration separately.
Do not delete uncertain staging folders while the debug app is running. Keep the
disposable root for diagnosis; no automatic cleanup command is required.

## Stage 5 acceptance follow-up

- Generate fresh React/Vite JavaScript and TypeScript projects in the disposable
  parent. Start both together. Compare each card URL to its own `Local:` log line,
  open both, Stop one and verify the other still responds. Restart and compare
  again; do not assume the port stays the same. Repeat with Node 26 if available.
- A fresh stopped project has no reported URL. After Start, the card must still
  say **Running**, with readiness explicitly unchecked. Stop must hide that run's
  reported link. While stopped, configure a manual Local URL in Edit and check
  that its Open action uses exactly that address and labels it configured.
- Older generated projects keep their existing files. To retest the reported
  strict-port failure, manually choose another port or set `server.strictPort`
  to `false` in the older project's Vite config. Do this only in a disposable
  fixture; no automatic migration edits personal projects.
- Check native folder picker, keyboard/focus, reload continuity, cancellation and
  Quit/relaunch. Automated browser tests cover simulations, not Mac WebView or
  native dialog behavior. Record any unobserved case.
