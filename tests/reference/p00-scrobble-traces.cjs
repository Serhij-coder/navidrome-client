#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const assert = require('node:assert/strict');
const { loadProgressCallback } = require('../../tools/app/p00-reference-runtime.cjs');

function characterize(root, parser) {
    const cases = [];
    let provenance;
    function run(id, options, samples) {
        const h = loadProgressCallback(root, parser, options);
        const states = samples.map((sample) => h.sample(...sample));
        provenance = h.provenance;
        const result = { id, options, samples, states, requests: h.requests };
        cases.push(result);
        return result;
    }
    for (const speed of [0.5, 1, 2]) {
        const result = run('speed-' + speed, { speed }, Array.from({ length: Math.min(121, Math.floor(180 / speed) + 1) }, (_, second) => [second * speed]));
        const first = result.states.findIndex((s) => s.submitted);
        assert.equal(60 / speed, first);
        assert.equal(1, result.requests.filter((r) => r.query.submission).length);
        assert.equal(180000000, result.requests.find((r) => r.query.submission).query.position);
    }
    const seek = run('jump-rebaseline-and-restart', {}, [0, 2, 4, 120, 122, 124, 1, 2].map((s) => [s]));
    assert.equal(1000, seek.states.at(-1).listenedMs);
    const pause = run('pause-not-credited', {}, [[0], [2], [4, 'paused'], [6, 'paused'], [8], [10]]);
    assert.equal(6000, pause.states.at(-1).listenedMs);
    const privateMode = run('private-mode', { privateMode: true }, [[0], [2], [4], [60], [62]]);
    assert.equal(0, privateMode.requests.length);
    assert.equal(0, privateMode.states.at(-1).listenedMs);
    const ticks = run('jellyfin-progress-and-submission-units', { serverType: 'jellyfin', resumable: true }, Array.from({ length: 71 }, (_, s) => [s]));
    assert.equal(100000000, ticks.requests.find((r) => r.query.event === 'timeupdate').query.position);
    assert.equal(1800000000000, ticks.requests.find((r) => r.query.submission).query.position);
    const repeated = run('seek-to-start-allows-resubmission', { durationThresholdMs: 2000 }, [[0], [1], [2], [3], [11], [0], [1], [2]]);
    assert.equal(2, repeated.requests.filter((r) => r.query.submission).length);
    return { schema: 1, task: 'P00.06', evidenceKind: 'exact-progress-callback-with-explicit-boundaries', provenance, cases,
        limitations: ['Media-position samples are synthetic, not measured audio clocks.', 'No mounted React hook, song-change debounce, timers, backend transport or network completion tested.',
            'Request recorder never calls success; submission is marked before completion and is not retried within a play-through.'] };
}
if (require.main === module) {
    const [root, parser, expected] = process.argv.slice(2);
    if (!root || !parser) { console.error('Usage: node tests/reference/p00-scrobble-traces.cjs REFERENCE_ROOT TYPESCRIPT_JS [EXPECTED_JSON]'); process.exit(2); }
    try {
        const report = characterize(root, parser);
        if (expected) { assert.deepEqual(report, JSON.parse(fs.readFileSync(expected))); console.log('Pinned scrobble callback traces match expected evidence.'); }
        else process.stdout.write(JSON.stringify(report, null, 2) + '\n');
    } catch (error) { console.error(error.message); process.exitCode = 1; }
}
module.exports = { characterize };
