# Standalone permission test builds

These are draft test apps, not a new release. They use your ordinary Fairway settings,
sites, databases and document root. Quit the installed app completely from its tray/menu
bar before opening a test copy. Quit the test copy the same way before returning to the
installed app; closing its window alone leaves Fairway running.

## Windows x64

Extract the entire ZIP into a permanent test folder such as `C:\FairwayPermissions`.
Open `Fairway XMP Permissions Test\fairway.exe`; keep its `runtime` folder beside it.
No Setup program is required. This uses the WebView2 and Visual C++ prerequisites already
present on a machine running the installed Fairway app. The EXE has no Authenticode
signature, as with RC 1. Its window is labeled **Permissions Test**.

Add a disposable site or rename a site address. The normal Windows administrator/UAC
prompt should appear without an empty PowerShell window behind it. Decline once and
retry through **Set up addresses**. The site is saved before address setup; do not add
it twice. Start Nginx and confirm the address works.

For HTTPS, enable it on a site and approve Windows' certificate trust confirmation.
There should be no empty console behind that confirmation. Existing trust may skip the
confirmation; to test fresh trust, turn HTTPS off for every site, remove certificate
trust, then enable it again. Also test cancellation/retry and trust removal. The change
suppresses console windows, while retaining Windows security confirmations and errors.

When done, quit from the tray and reopen the installed app. Do not enable/disable CLI
integration merely to test permission prompts; that would change your user PATH.

## Mac Apple Silicon

Unzip the archive and open **Fairway XMP Permissions Prototype.app** directly; do not
run an installer or overwrite the installed app. The test app is Developer ID signed
and must be notarized/stapled before hosts testing. Its window says **Native Permissions
Prototype**. The archive's build metadata records its notarization status.

Adding or renaming an address registers the native hosts helper when needed. macOS
requires one-time administrator approval in **System Settings → General → Login Items
& Extensions**. Fairway opens that page. Approve the Fairway XMP / Ei Maung helper, then
return and click **Set up addresses**. The site is already saved. macOS chooses Touch ID
or credentials for this approval. Later address changes should reuse the approved
helper without another password prompt. Test add, rename, remove and cancelled approval.
The helper is machine-wide and modifies only Fairway's loopback `.test` entries.

The native HTTPS implementation is the one already reported to work with one Touch ID
prompt. Existing CA trust is reused. Test it independently from hosts approval.

Before moving/deleting the test app or returning to RC 1, quit Fairway and unregister
the helper. In Terminal, use the actual path to the test bundle:

```sh
'/path/to/Fairway XMP Permissions Prototype.app/Contents/MacOS/fairway' --remove-native-hosts-helper
```

This keeps your saved sites, hosts entries and CA trust. Removing the last site address
also unregisters the helper. Then reopen the installed app normally.

## Shared UI checks

The Add/Edit site dialog keeps short labels and one approval notice. Question-mark
buttons contain the longer domain, folder and routing explanations. Escape closes a
help tip first, then the dialog on a second press. Validation errors remain visible.
Existing `index.php` files are preserved; the smaller welcome page appears only when
Fairway creates a missing default index.

Record the OS, embedded commit from `--build-info`, prompt/console behavior and whether
cancellation followed by retry works. Existing RC 1 remains the installed fallback.
