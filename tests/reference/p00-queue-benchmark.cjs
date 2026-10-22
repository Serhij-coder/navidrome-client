#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const os = require('node:os');
const { performance } = require('node:perf_hooks');
const { loadReference } = require('../../tools/app/p00-reference-runtime.cjs');

async function measure(root, esbuild) {
    const h = await loadReference(root, esbuild, 1729);
    const { api, runtime } = h;
    const store = api.usePlayerStoreBase;
    const initial = JSON.parse(JSON.stringify({ player: store.getState().player, queue: store.getState().queue }));
    const body = JSON.parse(fs.readFileSync(require('node:path').join(__dirname, '../fixtures/p00/songs.json'))).songs.find((s) => s.backend === 'subsonic').body;
    const template = api.ssNormalize.song(body, { id: 'benchmark-fixture' });
    const rows = [];
    for (const count of [10000, 100000]) {
        const songs = Array.from({ length: count }, (_, i) => ({ ...template, id: `song-${i}`, name: `Track ${i}`, albumId: `album-${Math.floor(i / 10)}` }));
        // Warm module execution once; keep samples separate from renderer/engine claims.
        for (let sample = -1; sample < 5; sample++) {
            runtime.settings.general.resume = true;
            store.setState(JSON.parse(JSON.stringify(initial)));
            await new Promise(setImmediate);
            const run = async (operation, fn) => {
                const start = performance.now(); fn(); const applied = performance.now();
                await new Promise(setImmediate); const settled = performance.now();
                if (sample >= 0) rows.push({ count, sample, operation, applyMs: applied - start, settledMs: settled - start,
                    processRssBytes: process.memoryUsage().rss, heapUsedBytes: process.memoryUsage().heapUsed });
            };
            await run('replace-queue', () => store.getState().addToQueueByType(songs, api.Play.NOW));
            await run('toggle-shuffle', () => store.getState().toggleShuffle());
            const s = store.getState(); const lastId = s.queue.default.at(-1);
            await run('move-last-to-next', () => store.getState().moveSelectedToNext([s.queue.songs[lastId]]));
            await run('remove-last-occurrence', () => store.getState().clearSelected([store.getState().queue.songs[lastId]]));
        }
    }
    const host = { cpu: os.cpus()[0]?.model, logicalCpus: os.cpus().length, totalMemoryBytes: os.totalmem(),
        platform: os.platform(), architecture: os.arch(), kernel: os.release(), node: process.version };
    return { schema: 1, task: 'P00.07', evidenceKind: 'headless-original-queue-subset-measurement', date: new Date().toISOString().slice(0, 10),
        host, conditions: process.env.P00_BENCHMARK_CONDITIONS ?? 'unrecorded', fixture: { records: [10000, 100000], fieldsPerNormalizedSong: Object.keys(template).length, source: 'pinned ssNormalize.song on tracked synthetic DTO', samplesPerOperation: 5, warmups: 1 },
        provenance: h.provenance, rows, limitations: ['Node production-mode bundle, not packaged Electron or Rust application.',
            'Timers measure synchronous command application and subsequent in-memory persistence settling; no physical disk/IndexedDB latency.',
            'RSS is this measurement process only, including bundling overhead; not total app/helper RSS. No GPU/display/audio data.',
            'Five measured samples per operation; report range/median only, no p95/p99 or desktop budget calibration.'] };
}
if (require.main === module) {
    const [root, esbuild] = process.argv.slice(2);
    if (!root || !esbuild) { console.error('Usage: node tests/reference/p00-queue-benchmark.cjs REFERENCE_ROOT ESBUILD_JS'); process.exit(2); }
    measure(root, esbuild).then((r) => process.stdout.write(JSON.stringify(r, null, 2) + '\n')).catch((e) => { console.error(e.message); process.exitCode = 1; });
}
module.exports = { measure };
