# P00 controlled reference run record

Date: 2026-10-21. Source: the archive and commit recorded in [baseline](README.md). No live Feishin profile was opened or modified. No source baseline/inventory or ignored reference plan was edited. Network dependency access used approved sandbox escalation; no release/deployment was performed.

The archive was extracted with Python `tarfile.extractall(filter='data')` into `/tmp/feishin-p00-reference`. Node v22.22.2 and npm 10.9.7 were available. pnpm 11.5.2 and TypeScript 6.0.3 were installed under `/tmp/feishin-p00-tooling` using npm `--ignore-scripts --no-audit --no-fund`, with cache under `/tmp`. Frozen reference dependencies were installed with all lifecycle scripts disabled:

```sh
node /tmp/feishin-p00-tooling/node_modules/pnpm/bin/pnpm.cjs install --frozen-lockfile --ignore-scripts --store-dir /tmp/feishin-p00-pnpm-store --config.cache-dir=/tmp/feishin-p00-pnpm-cache --config.state-dir=/tmp/feishin-p00-pnpm-state --config.manage-package-manager-versions=false
```

Run from the temporary reference root. Lockfile supply-chain verification passed after transient registry error 23 retries; 1,197 packages installed in 2m25.9s. The pinned git Node-MPV package required build scripts and remained unbuilt because scripts were disabled. Electron binary installation and `electron-builder install-app-deps` were not run. Do not interpret the package installation as a desktop runtime result.

Explicit controlled production builds, from the reference root, passed:

```sh
node node_modules/vite/bin/vite.js build --config web.vite.config.ts
node node_modules/vite/bin/vite.js build --config remote.vite.config.ts
```

Web: 3,415 modules, 46.40 s plus service-worker generation; remote: 2,060 modules, 9.50 s. Warnings: HTML bootstrap scripts without `type=module`, browser externalization of `module` from TagLib WASM, mixed static/dynamic mpv-filter import, large chunks; remote additionally had generated empty manifest chunk and CSS resolved at runtime. They did not fail bundling. These are reference build observations, not startup/frame/audio benchmarks.

The computer-use inventory returned `apps: []`, `browsers: []`; opening an isolated in-app browser returned `Browser is not available: iab`, and the Chrome fallback returned `Browser is not available: chrome`. No controlled UI could be attached. Consequently no page launch, screenshots, keyboard/drag flows, browser profile serialization, HTTP/event capture or OS playback results are claimed. No live Jellyfin/Navidrome/generic Subsonic/OpenSubsonic instance was provisioned in this task. Never use the user's running music server or live profile as an implicit substitute.

Reproduce source/fixture checks from the Rust repository root after preparing the optional reference directory and dependencies:

```sh
node tools/app/p00-source-audit.cjs /tmp/feishin-p00-reference /tmp/feishin-p00-tooling/node_modules/typescript/lib/typescript.js > /tmp/source-audit.json
cmp /tmp/source-audit.json docs/app/baseline/source-audit.json
node tools/app/p00-wire-check.cjs /tmp/feishin-p00-reference /tmp/feishin-p00-reference/node_modules/.pnpm/node_modules/esbuild/lib/main.js tests/fixtures/p00/songs.json > /tmp/wire-results.json
cmp /tmp/wire-results.json docs/app/baseline/wire-results.json
P00_TYPESCRIPT=/tmp/feishin-p00-tooling/node_modules/typescript/lib/typescript.js python3 -m unittest discover -s tests/tooling -p 'test_*.py'
python3 tools/app/check-p00.py --archive docs/upstream/feishin-7d6ad318.tar.gz --functions docs/inventory/functions.jsonl
```

The source harness executes only selected declarations with explicit bindings in VMs. The wire harness bundles the pinned schemas/normalizers/semver utility with the reference's esbuild 0.25.12; no installer/lifecycle hook is invoked. Its console suppresses raw normalizer payloads. These tools are optional reference development aids, not application build/CI prerequisites. All default tracked checks and the generator work without the reference package.

To close the runtime gate, supply an isolated controllable browser and desktop session with disposable profile roots under `/tmp`, disable analytics before first HTML execution (`settings.js` with `ANALYTICS_DISABLED=true` and profile `umami.disabled=1`), disable automatic updates before desktop startup, and bind disposable fixture services to loopback. Capture every journey's original steps/outcomes with versions, redacted traces and input hashes. Record the original outcome separately from each journey's `target_expected`, which may intentionally differ under its linked ADR; an original defect is evidence, not a failed target implementation. Export historical/current settings and queues from this profile, not fabricated target state. Then run [performance protocol](performance.md), complete the hardware matrix/calibration and document audio/device evidence. Do not install upstream lifecycle hooks merely to inspect source or substitute static fixtures for these missing runs.
