# Node Projects: manual testing on your MacBook

Stage 4 is ready for your acceptance testing. This guide uses a disposable debug
profile and small projects, without replacing your installed Fairway app. It does
not authorize Stage 5.

## 1. Prepare the checkout and tools

Open Terminal and run:

```sh
cd /Users/rioo/projects/fairway-xmp-personal
git branch --show-current
git status --short
node --version
npm --version
cargo --version
```

Expect `feature/node-projects` and a clean working tree after the Stage 4 commit.
Build prerequisites are in [Contributing](../CONTRIBUTING.md): Node 24, Python
3.12+, Rust stable 1.90+ and Xcode Command Line Tools. If a tool is missing, stop
and share that error. Do not change your global Node default or installed app to
complete this check. Run `npm ci` only if this checkout's dependencies are missing.

The automated checks used a temporary Rust toolchain rather than installing one
system-wide. If `cargo` is missing and both of these `/tmp` directories still
exist, you may reuse that toolchain in **this Terminal only**:

```sh
export PATH="/tmp/fairway-stage1-cargo/bin:$PATH"
export CARGO_HOME="/tmp/fairway-stage1-cargo"
export RUSTUP_HOME="/tmp/fairway-stage1-rustup"
cargo --version
```

These exports do not edit a shell profile. If those directories are absent or
the command fails, stop and report it rather than changing the installed app.

## 2. Create an isolated profile

Run the following in that same Terminal. Keep it open: the variable identifies
this test session. The runtime is copied from the installed app, not modified.

```sh
export FAIRWAY_MANUAL_ROOT="$(mktemp -d /tmp/fairway-node-manual.XXXXXX)"
mkdir -p "$FAIRWAY_MANUAL_ROOT/home" "$FAIRWAY_MANUAL_ROOT/data"
cp -cR "/Applications/Fairway XMP.app/Contents/Resources/runtime" "$FAIRWAY_MANUAL_ROOT/runtime"
printf 'Disposable test root: %s\n' "$FAIRWAY_MANUAL_ROOT"
```

If the installed runtime path is absent, stop and report it. If `cp -cR` reports
that cloning is unsupported, use `cp -R` with the same paths instead. A full copy
needs roughly 800 MB. Do not run runtime preparation or verification against the
installed app's resources.

## 3. Create two safe Node fixtures

This creates two tiny HTTP servers with no dependencies. It also allocates
separate ports for the disposable PHP stack, reducing conflicts with an installed
Fairway instance. It does not start any service.

```sh
python3 - "$FAIRWAY_MANUAL_ROOT" <<'PY'
import json, pathlib, socket, sys
root = pathlib.Path(sys.argv[1])
listeners = [socket.socket() for _ in range(6)]
for listener in listeners:
    listener.bind(('127.0.0.1', 0))
ports = dict(zip(['website', 'phpmyadmin', 'mysql', 'php', 'mail', 'smtp'],
                 [listener.getsockname()[1] for listener in listeners]))
(root / 'data' / 'settings.json').write_text(json.dumps(ports))
for listener in listeners:
    listener.close()
server = r"""const http = require('node:http');
const path = require('node:path');
const port = Number(process.argv[2]);
const server = http.createServer((req, res) => {
  console.log('Request: ' + req.method + ' ' + req.url);
  res.writeHead(200, {'Content-Type': 'application/json; charset=utf-8'});
  res.end(JSON.stringify({app: path.basename(__dirname), node: process.version, message: 'Hello café 日本'}));
});
server.listen(port, '127.0.0.1', () => {
  console.log('Listening at http://127.0.0.1:' + port + '/ using ' + process.version);
  console.error('Example stderr: this is a test message, not a failure.');
});
process.on('SIGTERM', () => server.close(() => process.exit(0)));
"""
for name, port in [('first-app', 43101), ('second-app', 43102)]:
    directory = root / name
    directory.mkdir()
    (directory / 'server.cjs').write_text(server)
    (directory / 'failure.cjs').write_text("console.error('EXPECTED_TEST_FAILURE');process.exit(7);")
    (directory / 'package.json').write_text(json.dumps({
        'name': name, 'engines': {'node': '>=22'},
        'scripts': {'dev': f'node server.cjs {port}', 'fail': 'node failure.cjs'}
    }, indent=2))
www = root / 'home' / 'fairway' / 'www'
www.mkdir(parents=True)
(www / 'node-php-check.php').write_text('<?php echo "PHP still works: ", PHP_VERSION;')
print('Project folders:', root / 'first-app', root / 'second-app')
print('Disposable PHP website port:', ports['website'])
PY
```

Expect two folders, each with `package.json`, `server.cjs` and `failure.cjs`.
There is no npm installation step or private environment file. If port 43101 or
43102 is occupied, starting that fixture should report `EADDRINUSE`. Do not stop
an unrelated program. Change only the fixture's `dev` script to another unused
port, update its saved URL, and inspect again.

## 4. Launch your updated edition safely

Use this exact development launch from the same Terminal:

```sh
env FAIRWAY_DEV_HOME="$FAIRWAY_MANUAL_ROOT/home" \
    FAIRWAY_DEV_DATA="$FAIRWAY_MANUAL_ROOT/data" \
    FAIRWAY_RUNTIME_DIR="$FAIRWAY_MANUAL_ROOT/runtime" \
    ZDOTDIR="$FAIRWAY_MANUAL_ROOT/home" \
    CARGO_TARGET_DIR="$FAIRWAY_MANUAL_ROOT/build" \
    npm run tauri -- dev --no-watch --config '{"identifier":"dev.fairway.nodeprojects.manual","app":{"windows":[{"title":"Fairway Node Projects Test","width":1080,"height":760,"minWidth":820,"minHeight":650}]}}'
```

Expect a window titled **Fairway Node Projects Test**. In Environment, its
Document root must begin with the printed `/tmp/fairway-node-manual.../home/`.
If it shows `/Users/rioo/fairway/www`, quit that development instance before
continuing: the isolated environment was not applied.

The separate application identifier distinguishes the test instance; the explicit
home/data variables isolate its data. These overrides work in **debug builds
only**. Do not add `--release`. Do not resume this test by opening a binary,
Finder/Dock copy, or installed app: those launches can omit the isolation values.
After Quit, repeat this Terminal command. Keep the normal installed app separate.
No shell profile file is edited.

If port 1420 is occupied by another development server, stop this launch and
report the conflict. Do not kill an unknown process or reuse an unknown server.

## 5. Open Node Projects and import

1. Click **Node Projects** in the sidebar's Tools group. Expect its own page with
   **Import Node project**, **Refresh**, and an empty state.
2. Click **Import Node project**, then **Choose folder…**.
3. In the native picker, press Command+Shift+G. Paste the absolute `first-app`
   folder path printed in step 3, not the literal text `$FAIRWAY_MANUAL_ROOT`.
4. Expect `dev` and `fail` scripts, the Node requirement `>=22`, package manager
   npm, and installed Node runtime choices.
5. Set **Project name** to `First app`, select **npm script → dev**, and choose an
   installed Node 22-or-newer version under **Node runtime**.
6. Set **Local URL (optional)** to `http://127.0.0.1:43101/` and click **Save project**.

Expect a saved card marked **Stopped**, with a message saying it has not started.
The saved selection receives an additional Node/npm compatibility check. A warning
means it was saved but needs correction through **Edit**; no project ran.

If no runtime appears, report the inspection message. Do not switch a global
Node default to make it appear. Import must not start a server or install packages.
Tab/Shift+Tab should stay inside the dialog; Cancel/Escape should return focus.
Manage Sites and the PHP dashboard must remain separate. On a short window, scroll inside the sidebar to reach Settings or Updates.

## 6. Start and check real readiness

Click First app's **Start**. Review **Start Node project?**: it must name the
project, selected Node version, `npm run dev`, script text and lifecycle hooks.
Click **Start project**.

Expect Starting, then Running; Start/Edit/Remove are disabled while active. Click
**Open URL**. Expect a browser JSON response naming `first-app`, the selected Node
version and `Hello café 日本`. Refresh it to produce request logs.

Running describes a process, not a health check. A browser response proves this
fixture is working. A wrong version, an unconfirmed start, or Running without a
working fixture response is something to report.

## 7. Read live logs

Click **Logs** on First app. Expect its run ID, timestamps, stdout, stderr and
Unicode output. The example stderr message is intentional.

- Turn off **Follow output** or scroll upward; output should not pull you down.
  Re-enable it to follow the newest records.
- Click **Clear view**. Visible old text disappears without deleting native files.
  Refresh the fixture browser page to generate new request output.
- Click **Maximize Project logs**, then Escape or **Restore Project logs**.
- Switch pages and return. The native project keeps running. A complete WebView
  reload may reset the chosen console; choose Logs again for retained output.

Retention/truncation notices are expected after enough output. Duplicated records,
executable HTML, mixed run IDs, wrong project output or a frozen UI are failures.

## 8. Stop and restart

Click **Stop**. Expect Stopping, then Stopped, and the fixture URL should stop
responding. Click **Restart**, review **Restart Node project?**, then click
**Restart project**. Expect a new run ID, fresh output and the URL responding again.

Previous-run output must not become part of the new run. Quick repeated clicks
should be disabled or rejected rather than starting duplicate servers.

## 9. Run multiple projects

Import `second-app` as **Second app**, choose `dev`, and save
`http://127.0.0.1:43102/`. Select a different installed Node version if available;
otherwise use the same version and report that the different-version manual
check was skipped. Start it explicitly.

Expect both URLs to identify their own project and runtime. Switch their Logs
views and check that output stays separate. Stop First app: Second app must keep
responding. Restart First app: both should work again. The backend allows one
active run per project and at most eight active projects.

## 10. Test edits, removal and invalid configurations

Stop the relevant project before each change. Use only the disposable fixtures.

| Check | What to do | Expected result / failure indicator |
| --- | --- | --- |
| Edit | Change a stopped project's name, script or installed runtime, then Save project | Saved selection persists; nothing auto-starts. |
| Duplicate folder | Import first-app again | Already-registered error; original stays intact. |
| Invalid URL | Enter `https://example.com/` in Local URL | Save disabled, with localhost guidance. |
| Broken manifest | Temporarily make only the fixture's package.json invalid JSON; Edit → Refresh inspection and runtimes | Clear read error; old inspection cannot be saved. Restore valid JSON afterward. |
| Incompatible version | Change the fixture's engines.node to `>=99`; inspect and save | Runtime warning after Save; Start rejects execution. Restore `>=22` and inspect again. |
| Missing folder | Rename stopped first-app to first-app-moved, then Refresh | Missing-folder warning, Start disabled; Edit can choose the moved folder. |
| Unsupported manager | Add `"packageManager": "pnpm@1.0.0"` to the fixture manifest; inspect | npm-only limitation explained; Start cannot substitute npm for pnpm. Remove the property afterward. |
| Remove | Remove a stopped registration → Remove registration | Card disappears; project folder/files remain. Cancel/Escape preserves registration. |

Except for the intentional broken-JSON test, keep the manifest valid JSON. If a
dialog becomes stale, close it, Refresh projects, and reopen it before retrying.
The UI does not automatically repeat a conflicting execution.

For a **missing runtime** check, use only an extra installation owned by this
isolated profile. If Settings → Node.js offers **Install**, install an additional
version there without **Install & use** or **Use**. Select it for a disposable
registration, stop that project, and uninstall only that extra version. Refresh
Node Projects: expect a missing-runtime warning and disabled Start. Edit to an
available runtime to recover. Uninstall while a project is active must be blocked.

If Settings reports a read-only external manager/store, skip that check and share
the message. Do not remove, rename or alter your personal fnm/nvm/Homebrew/system
Node installations just to simulate a missing runtime.

## 11. Test a failing script

Stop First app, Edit it, choose `fail`, and Save project. Start and confirm.
Expect Failed, exit code **7**, and `EXPECTED_TEST_FAILURE` on stderr. The page must
stay usable. Select `dev` again to recover; it must not restart automatically.

## 12. Verify PHP and MySQL independently

In this isolated test window, open **Environment** and click **Start all**. Use
the Nginx card's Open control, then visit `/node-php-check.php` on that website
address. Expect `PHP still works:` and the PHP version. Open phpMyAdmin or Browse
Data to confirm the disposable MySQL instance is available.

While both Node fixtures are running, click PHP **Stop all**. Both Node URLs must
keep working. Start PHP services again; stopping one Node project must leave
PHP/MySQL running. First-time MySQL initialization may take longer.

If an assigned port has since become occupied, change only this disposable
profile's ports in Settings. Do not stop an unrelated service or change the
installed app. Do not add .test sites, change hosts/trust, enable global CLI
integration, choose Node Use, or test updates for this acceptance run.

## 13. Verify navigation and hiding

With both fixtures running, switch pages, minimize, then close the test window
using its close button. Closing normally hides Fairway; both URLs should still
respond. Use this test instance's menu-bar control to show the window and reopen
Node Projects.

Report if navigation/hiding stops a fixture, returning shows stale controls, or
another start creates a duplicate execution. Include the exact sequence.

## 14. Quit and confirm cleanup

Leave both fixtures running and use **Quit** in this test app's menu. Expect
shutdown, both URLs to stop responding, and the Terminal development command to
finish. From another Terminal, these two requests should now fail to connect:

```sh
curl --max-time 2 http://127.0.0.1:43101/
curl --max-time 2 http://127.0.0.1:43102/
```

Connection refusal after Quit is the expected result. If the fixture still
responds, record its response and any quit error. Do not kill processes by name
or port. Relaunch only using step 4's isolated Terminal command: registrations
should remain saved, but nothing should auto-start.

Your original Terminal's `node --version` should remain unchanged. Keep the test
root until results are reviewed; do not delete real project/runtime directories.

## What to Share Back After Testing

Send pass/fail/skipped results for steps 5–14, the Stage 4 commit, macOS version,
and Node versions selected. Helpful screenshots are:

- The test window's Document root and Node Projects cards.
- Import/Edit showing requirements, runtime and script selection.
- Both browser responses for the concurrent-project check.
- The failed-script console with run ID, stderr and exit code.
- Any incorrect status, inaccessible dialog, or hide/quit error.

Copy relevant console lines rather than entire directories. Native Node logs are
under `$FAIRWAY_MANUAL_ROOT/data/node-project-logs/<project-id>/`: `events.jsonl`
and `events.1.jsonl` contain output; `last-run.json` contains status. PHP logs are
under `$FAIRWAY_MANUAL_ROOT/data/logs/`. Include relevant build/launch errors from
the step 4 Terminal. Stage 4 has no graphical log-export button.

Before sharing, redact usernames/private paths as `<redacted>` and remove private
application output. Never share passwords, tokens, database credentials, private
environment values, `.env` files, or a whole personal project/configuration folder.
The sample fixture itself prints no application secrets.

Use this short issue template:

```text
Test step:
Expected result:
Actual result:
Exact error message:
Screenshot or relevant redacted log:
Selected Node version:
macOS version:
Stage 4 commit:
```

Automated verification and implementation completion are separate from your
manual acceptance. Report skipped checks honestly. Stage 5 remains unauthorized
until you review these results.
