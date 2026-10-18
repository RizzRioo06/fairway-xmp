# CI and build performance

Routine verification runs on pull requests, `main` pushes, or manual dispatch. A newer commit cancels obsolete routine checks for the same PR/ref. Feed updates and candidate/release tags do not launch a duplicate routine build. Candidate checks still include every platform and browser review from one commit; no candidate is canceled by routine verification concurrency.

Browser review and the Docker portability host use the tested Ubuntu 24.04 image explicitly. This keeps their environment stable through GitHub's [announced Ubuntu 26.04 migration](https://github.com/actions/runner-images/issues/14748). Runtime compilation continues to use Ubuntu 22.04 for the glibc 2.35 baseline.

## Runtime and dependency reuse

`Warm build inputs` runs on `main` when runtime inputs, Cargo dependencies, or the relevant preparation/workflow files change. It can also be dispatched manually. It prepares the locked Windows/macOS/Linux inputs and compiles dependencies with the same Rust cache identity as the installer jobs. It covers debug test/lint profiles plus release app, updater-probe, and installer-probe variants, including Tauri's custom-protocol feature and the explicit Windows target. It does not import signing credentials, create candidate acceptance records, or publish anything. Warm-up failures are independent of routine verification and should be resolved before cutting a candidate.

Mac warm-up and installer jobs read `MACOSX_DEPLOYMENT_TARGET` from Tauri's configured minimum system version before restoring Rust caches. The deployment target participates in the cache key, keeping plain Cargo commands, probes, and Tauri compilation aligned. Application binaries are rebuilt for the current commit; the shared Rust cache retains dependencies rather than old application executables.

GitHub caches are scoped by branch/ref: different candidate tags cannot restore each other's caches, but they can restore `main` caches. Run the warmer on `main` before creating a candidate tag, especially after changing runtime inputs. A manual run on a feature branch warms that branch only. The routine **Build and verify Fairway XMP** workflow also has a manual `warm_inputs` checkbox, useful when validating the warmer on a branch before it reaches `main`. A Rust toolchain update can invalidate compiled dependencies; dispatch the warmer again after such an update. Warm-up is deliberately not a dependency of candidate builds, so a cache miss never blocks a correct fresh build.

Windows and macOS cache `.runtime-cache`, containing downloaded upstream archives. The exact key includes the platform/architecture and current runtime/source/upgrade-fixture locks. Older download caches may supply unchanged archives: every archive consumed by the preparer or source collector is checked against its committed checksum or pinned Git tree before use. Source archives, runtime binaries, versions, compression settings, and signatures are unchanged by caching. No credentials, student data, signing keychains, or generated release installers are cached.

Linux caches its staged glibc 2.35 runtime together with the corresponding sources, avoiding a second large archive cache for the same inputs. The warmer and installer use one composite action, so their preparation commands and keys stay aligned. A change to the runtime manifest, lock, notices, preparers, or composite action invalidates that cache. Every restore runs runtime verification. The builder still installs its compilation prerequisites; target machines need none of those packages.

At investigation time the repository was using 9.98 GB of its 10 GB cache allowance, including repeated caches under old candidate tags. Download, staged-runtime, and installer Rust caches are saved on branch runs rather than copied into every candidate tag's scope. Tagged candidates restore from the warmed `main` caches and can still build fresh on a miss. GitHub automatically evicts least recently used entries as new caches arrive; no old installer artifacts or release assets are deleted. Keep warm-up on `main` and avoid adding duplicate multi-gigabyte caches when tuning this further.

Runtime/download caches are saved after successful preparation, before expensive release packaging. Rust dependency caches are published only after a successful job. GitHub cache entries are immutable: saving an incomplete cache during a failed/canceled build would prevent a later successful run from filling it in. The independent warmer seeds complete release dependencies ahead of candidate packaging, so a later signing/lifecycle failure can reuse that completed cache. macOS collects locked sources before importing Apple credentials or notarizing, and desktop jobs compile lifecycle probes and obtain the baseline installer before final packaging. This catches source/network/compilation failures earlier without dropping release checks.

The shared `installer-complete-v1` key names the set of warmed Rust profiles. Bump it in the warmer and all three installers when that set changes; an exact cache hit cannot be extended in place.

Runtime version checks are local. In particular, Mailpit verification uses `version --no-release-check` to compare the bundled version without contacting GitHub for an upstream release lookup. A network failure during that unrelated lookup must not invalidate an otherwise correct locked runtime.

## Candidate retry rules

For a transient failure at the same commit, use GitHub's **Re-run failed jobs** on the existing candidate run. This preserves successful platform jobs and their exact installer artifacts. For example:

```sh
gh run rerun RUN_ID --failed
```

The failed platform job starts again; this change does not resume an arbitrary step inside a job. Keep using the full candidate run and its assembled manifest for acceptance and publication. A code, runtime, packaging, or workflow fix requires a new candidate at the new commit; do not combine platform artifacts from different source commits. Publish only the tested files, without rebuilding them for publication.

## Baseline and measurement

Before these changes, routine verification took about 6–7 minutes. Successful candidate run [38989070274](https://github.com/eimg/fairway-xmp/actions/runs/38989070274) took 28 minutes: Windows and macOS each used about 27 minutes in parallel. Windows release compilation took 5½ minutes and NSIS compression another 7 minutes; Mac signing/notarization took about 8 minutes. Nearby Linux cache misses spent 22–24 minutes preparing runtimes, compared with about 5 minutes for the entire Linux build job on a hit.

The beta.3 candidate attempts spanned roughly 12 hours including failures, fixes, and gaps between attempts. Avoiding repeated downloads and unnecessary full retries is the first target. Preserve the existing LZMA/offline installer compression for download size. Compare cold and warm runs separately, and inspect step times rather than adding parallel job durations. Apple service time and installation checks remain variable; cache warm-up is not a guarantee of a particular candidate duration.

Branch validation measured Linux runtime preparation at [19 minutes before its cache was populated](https://github.com/eimg/fairway-xmp/actions/runs/39088731864/job/117560911902), then [42 seconds on a verified cache hit](https://github.com/eimg/fairway-xmp/actions/runs/39090092616/job/117564821638). The Windows warm-up job took [14.5 minutes while populating the complete Rust cache](https://github.com/eimg/fairway-xmp/actions/runs/39090092616/job/117564821472), then [7.1 minutes with that cache restored](https://github.com/eimg/fairway-xmp/actions/runs/39091156000/job/117568015563). Signed-candidate duration also includes installer compression, signing/notarization, and lifecycle checks.
