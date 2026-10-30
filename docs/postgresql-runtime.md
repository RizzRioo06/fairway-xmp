# Managed PostgreSQL runtime

Stage 2 pins PostgreSQL **18.6**, macOS **arm64**, deployment target **15.0**.
Only the isolated macOS 27.0.1 arm64 host has been exercised; the deployment
minimum is a build setting, not evidence of testing on macOS 15/26.

## Authorized source preparation

The separate preparer never calls Homebrew, starts a server or touches a cluster.
An operator must explicitly authorize execution of upstream configure/make scripts:

```sh
python3 scripts/prepare_postgresql_runtime.py --authorize-source-build \
  --output /absolute/canonical/new-parent/postgresql
```

The parent must already exist; the output must not exist. For development, a new
`src-tauri/resources/runtime/postgresql` is the controller's payload input. Never
replace an existing runtime to make this command pass. Scratch build stages are
retained beside the output for diagnosis and can consume space; remove only your
known build stages after review, never cluster/data directories.

`postgresql-source.lock.json` pins the official source URL, SHA-256 and configure
options. The checksum is checked **before extraction or script execution**;
archive traversal, links and special files are refused before any extraction.
The official [checksum sidecar](https://ftp.postgresql.org/pub/source/v18.6/postgresql-18.6.tar.bz2.sha256)
reports `555610c24d53e4316da5b7d3fc25c279d96856d5e0e23ee308c328c5fa881d9f`.

The command clears the build environment, uses existing Apple tools, four make
jobs and `make install DESTDIR=<owned stage>` with a virtual `/fairway-postgresql`
prefix. It never uses sudo or writes that virtual prefix globally. ICU, readline,
compression libraries, GSSAPI, LDAP and XML/XSLT are disabled. TLS is absent in
this minimal runtime; Stage 2 permits only explicit loopback SCRAM connections.
This runtime is not qualified for remote connections, extensions or backups.

The payload contains nine fixed native tools, libpq/server modules, catalog and
timezone resources. Developer pgxs rules/static libraries are omitted. Dependencies
are rewritten only from this build's virtual prefix to private `@loader_path`
locations; unknown dependencies fail preparation. Every Mach-O file must be
arm64, ad-hoc signed and verified, with dependencies resolving inside the payload
or to macOS system libraries. The unchanged official source archive and COPYRIGHT
preserve upstream code and notices under `licenses/`.

## Integrity and release qualification

The preparer emits a manifest covering every payload file's size, SHA-256 and
executable flag. It publishes with an exclusive rename and prints the manifest
SHA-256. It **does not update** the application pin. A maintainer must review a
qualified payload and its real lifecycle results before updating
`src-tauri/src/postgresql/managed/runtime-pin.json` and compiling the application.
Different compiler/SDK/signature inputs may produce different bytes; a version
banner alone never authorizes a replacement. The current pin is the tested
41 MiB development payload; 45 arm64 Mach-O files passed dependency/signature checks.

Provisioning accepts only that pinned manifest and exact file tree, checks hashes,
permissions, links and ownership, copies to private staging, records its inode
identity and publishes exclusively. Installed files are rechecked before control
and health. An occupied or unowned runtime is never overwritten or adopted.
The server's runtime lease stays held until its owned child exits.

**Release blockers:** normal release workflows do not yet package this independent
runtime. Distribution must sign its nested code with the authorized team **before**
computing the final manifest/pin and compiling the app; signing after pinning changes
hashes and is refused. The final signed/hardened app, Keychain access, notarization,
installer and supported older macOS versions need qualification. Do not distribute
the development ad-hoc payload as a verified release or disable library validation
to bypass this boundary. Existing PHP/MySQL/Node runtime locks remain unchanged.

The rejected Homebrew-bottle candidate passed version/link checks but failed real
`initdb` because of embedded Homebrew catalog/timezone paths. It is not an accepted
payload; the official source build removes that packaging dependency.
