# Linux data browser

Prepared for `0.1.0-rc.5`, available in current source builds. RC 5 has not been built or published; published RC 4 installers predate this command.

Run `fairway data-browser`, then open the access URL printed by the command in a browser. It uses port **8803** and a temporary per-run token; opening it sets a session-only access cookie and redirects to the clean URL. Keep the command running; Ctrl+C stops the server. It serves the production build of [eimg/data-browser](https://github.com/eimg/data-browser), with its frontend and API on the same port. PHP services are independent; start MySQL with `fairway start mysql` if you want to browse Fairway’s database. PostgreSQL is a server you provide. SQLite opens an existing file on the Linux machine.

## Node setup

The command uses Node from your current shell’s PATH. The standalone browser’s SQLite driver requires Node **22 or newer**. npm is needed for initial dependency setup, not for warm offline launches. The desktop install picker still offers Node 20+ for other development work.

If Node is missing or older, Fairway prints commands using an existing fnm executable when detected, otherwise the included native executable. CLI integration preserves the existing fnm’s PATH priority. After installing Fairway’s CLI tools, you can use fnm directly:

```sh
fnm install --lts
eval "$(fnm env --shell bash)"
fnm use lts-latest
fairway data-browser
```

For Zsh use `--shell zsh`. For Fish use `fnm env --shell fish | source`, then `fnm use lts-latest`. Add the appropriate initialization to your shell configuration if you want it in future sessions. Fairway does not add Node activation or automatic project switching on Linux. Existing Node installations remain available.

The browser is bundled. On first use, npm downloads the exact dependencies in its packaged lockfile, without running package lifecycle scripts. The SQLite driver includes Node-API binaries for supported platforms; no compiler is required. Setup verifies the driver and retries on a later invocation if interrupted or unsuccessful. Concurrent invocations share a setup lock.

Later launches work offline. `fairway data-browser --prepare` downloads and verifies dependencies ahead of time without starting the server. Browser updates and changes to Node ABI get separate caches, so old and new versions do not overwrite one another. Cached files are under the Fairway application-data directory, in `data-browser/`. Removing Fairway preserves that directory, fnm-installed Node versions and database backups.

## Remote Linux hosts

Connect using SSH forwarding:

```sh
ssh -L 8803:127.0.0.1:8803 user@host
# In the remote shell:
fairway data-browser
```

Open the printed access URL on your own computer. Keep its token private; a new server run produces a new URL. The browser server listens only on loopback; no public host flag is provided. To use another port, pass `--port 9000` and forward `9000:127.0.0.1:9000`. Keep the local and remote port numbers the same.

Connection credentials stay in the running server process. File paths refer to the Linux machine. SQLite backups and SQL snapshots are saved under `~/.data-browser/backups`; Reveal in folder may be unavailable on a machine without a file manager. Stopping the command closes its sessions. Starting it again requires reconnecting.

The desktop data browser uses Fairway’s Rust/PHP bridge; its `fairway db` commands are a separate interface. See [desktop data browser](data-browser.md) for that implementation.

## Updates, removal and backups

Stop every running `fairway data-browser` command with Ctrl+C before updating or uninstalling. An active browser holds a lifetime lock; update preparation and uninstall stop with instructions instead of removing its files or leaving the server running. `fairway shutdown` stops PHP services and the supervisor; the independent browser remains running until you stop its command.

Standalone SQL snapshots preserve the table structure/data supported by this browser; they are not equivalent to complete engine backups containing every PostgreSQL/MySQL routine, trigger, view and other object. Use `fairway backup` for full Fairway MySQL environment backups and the external server’s own backup tools for PostgreSQL recovery. SQLite copies are stored beside the standalone SQL snapshots. None of these are a substitute for copying important backups off the machine.
