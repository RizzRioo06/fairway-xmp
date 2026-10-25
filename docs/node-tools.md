# Node.js and native fnm

Prepared for `0.1.0-rc.5`; RC 5 has not been built or published. Current source builds for all three platforms include the pinned upstream **fnm** executable. Published RC 4 installers predate this integration. Node itself is downloaded only when requested. macOS distribution applies the app’s normal code signing to fnm; its source and CLI behavior are unmodified. Use `fnm` directly; there is no `fairway node` or `fairway fnm` command.

| Platform | Detection and activation |
| --- | --- |
| macOS | Settings checks an interactive login Zsh environment, including nvm/Homebrew and inherited custom ZDOTDIR. Embedded terminals import exported tool settings, then use Fairway’s own prompt/history configuration. |
| Windows | Settings and embedded terminals refresh the system/user registry environment. Opting into fnm Node configures new embedded terminals and, with CLI tools enabled, new Command Prompt and PowerShell sessions. |
| Linux | Native fnm is on PATH through CLI integration or `fairway shell`. Node activation is configured by the user’s shell. The standalone browser uses the invoking shell’s Node. |

## Desktop settings

The Node.js card sits above Command-line tools. It highlights the selected version, source manager and executable path. Other detected installations are listed separately; only versions in fnm’s store can be managed here. A missing/broken fnm is reported separately from existing Node detection.

Manage Node.js opens automatically when no active Node is found and detection succeeds. **Install & use** installs the selected version and enables it in new terminals. With an existing Node selection, **Install** preserves it; choose **Use** beside an installed version to activate it. The picker shows the latest stable patch per major from Node 20 onward, including non-LTS majors. It currently defaults to the newest major, which may be non-LTS. Exact older versions remain available with `fnm install VERSION`, and all installed versions stay visible.

Use affects new terminals. Existing terminals/development servers keep their selection until restarted; no project pins, directory-change hooks or automatic version switching are added. On macOS activation initializes new Bash/Zsh shells when CLI tools are enabled. Windows uses a reversible current-user Command Prompt AutoRun hook and managed blocks in the normal Windows PowerShell 5/PowerShell 7 profiles. Existing content is preserved. PowerShell execution policy is respected; `-NoProfile` and Command Prompt `/D` skip external hooks. Windows activation paths cannot contain `%`, `!`, quotes or line breaks.

**Use existing Node setup** removes Fairway’s activation. **Uninstall** removes the chosen fnm version and its global npm packages; removing an active default first disables Fairway’s activation. Stop terminals and servers using the version before uninstalling. Versions managed by nvm, Homebrew or other tools remain unchanged. Native fnm users should select an installed numeric default if they want it reflected as Fairway’s managed choice.

## Storage and shell use

Fairway honors an inherited absolute `FNM_DIR`. Otherwise it follows fnm’s normal store: XDG data storage (normally `~/.local/share/fnm`) on macOS/Linux and roaming AppData `fnm` on Windows, retaining supported legacy `.fnm` stores. A variable set only inside a personal shell profile is not necessarily inherited by a GUI app launched from Finder/Start; compare Command details with `fnm env` when using a custom store. Node versions and global packages live outside the application and survive updates/removal. Removing CLI integration removes Fairway’s launchers/hooks and preserves the store.

For manual activation, Bash/Zsh use `eval "$(fnm env --shell bash)"` or `--shell zsh`. PowerShell uses `fnm env --shell powershell | Out-String | Invoke-Expression`; Fish uses `fnm env --shell fish | source`. Then use ordinary commands:

```sh
fnm install --lts
fnm use lts-latest
fnm list
fnm default VERSION
fnm uninstall VERSION
```

The [Linux standalone browser](linux-data-browser.md) requires Node 22+ and npm for first-use setup. Fairway’s PHP services, desktop browser and ordinary CLI do not require a preinstalled Node runtime.

## Verification and troubleshooting

Refresh detection after changing managers or PATH. New embedded terminals read the configured tool environment; macOS export capture does not import personal aliases/functions, prompts or history hooks. Detection timeouts and fnm errors stay visible. If fnm is missing, reinstall the complete Fairway package. If native fnm changed/deleted a default outside Settings, select an installed version with Use to repair activation.

The noninteractive Mac/Windows fnm smoke examples use scratch stores/profiles, exercise downloads/defaults/uninstall and leave personal installations unchanged. The Linux browser smoke covers native fnm, missing/old Node, concurrent setup, offline reuse, local access authentication, lifetime locks and real SQLite/MySQL requests. Commands are in [Contributing](../CONTRIBUTING.md) and [Testing](testing.md). These checks do not replace native Windows execution or optional WebView review.
