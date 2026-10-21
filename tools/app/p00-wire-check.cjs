#!/usr/bin/env node
'use strict';

// Optional controlled reference harness. Requires the frozen reference dependencies.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const [rootArg, esbuildArg, corpusArg] = process.argv.slice(2);
if (!rootArg || !esbuildArg || !corpusArg) {
    console.error('Usage: node tools/app/p00-wire-check.cjs REFERENCE_ROOT ESBUILD_JS CORPUS_JSON');
    process.exit(2);
}
const root = path.resolve(rootArg);
const esbuild = require(path.resolve(esbuildArg));
const corpus = JSON.parse(fs.readFileSync(corpusArg));
const build = esbuild.buildSync({
    stdin: { contents: `
        export {jfType} from './src/shared/api/jellyfin/jellyfin-types';
        export {ndType} from './src/shared/api/navidrome/navidrome-types';
        export {ssType} from './src/shared/api/subsonic/subsonic-types';
        export {jfNormalize} from './src/shared/api/jellyfin/jellyfin-normalize';
        export {ndNormalize} from './src/shared/api/navidrome/navidrome-normalize';
        export {ssNormalize} from './src/shared/api/subsonic/subsonic-normalize';
        export {getFeatures} from './src/shared/api/utils';
    `, resolveDir: root },
    tsconfigRaw: { compilerOptions: { baseUrl: root, paths: { '/@/shared/*': ['./src/shared/*'] } } },
    bundle: true, platform: 'node', format: 'cjs', write: false, logLevel: 'silent',
});
let warnings = 0;
const referenceConsole = { warn() { warnings++; }, log() {}, error() { throw new Error('Reference normalizer logged an error'); } };
const context = vm.createContext({ module: { exports: {} }, exports: {}, require, console: referenceConsole, setTimeout, clearTimeout, Buffer });
vm.runInContext(build.outputFiles[0].text, context, { timeout: 10000 });
const api = context.module.exports;
const prefixes = { jellyfin: 'jf', navidrome: 'nd', subsonic: 'ss' };
const results = [];
for (const sample of corpus.songs) {
    const prefix = prefixes[sample.backend];
    const parsed = api[`${prefix}Type`]._response.song.safeParse(sample.body);
    assert.equal(parsed.success, true, `${sample.id}: ${parsed.error?.message}`);
    const normalized = api[`${prefix}Normalize`].song(parsed.data, { id: sample.server_id });
    assert.equal(normalized.id, sample.expected.id, sample.id);
    assert.equal(normalized._serverId, sample.server_id, sample.id);
    assert.equal(normalized.duration, sample.expected.duration_ms, sample.id);
    results.push({ id: sample.id, schema: 'passed', normalized: { id: normalized.id, serverId: normalized._serverId, durationMs: normalized.duration } });
}
const versions = {};
for (const sample of corpus.invalidSongs) {
    const parsed = api[`${prefixes[sample.backend]}Type`]._response.song.safeParse(sample.body);
    assert.equal(parsed.success, false, `${sample.id} unexpectedly accepted`);
    results.push({ id: sample.id, schema: 'rejected-as-expected' });
}
const versionInfo = [['0.57.0', { queue: [2] }], ['0.48.0', { smart: [1] }]];
for (const version of ['0.47.0', '0.48.0', '0.57.0', 'development', '0.57.0-dev']) {
    versions[version] = api.getFeatures(versionInfo, version);
}
assert.equal(JSON.stringify(versions.development), JSON.stringify({ queue: [2], smart: [1] }));
assert.equal(JSON.stringify(versions['0.47.0']), '{}');
process.stdout.write(JSON.stringify({ evidenceKind: 'synthetic-inputs-through-pinned-source', songs: results, featureVersionProbe: versions, warnings, limitations: 'No live server, HTTP transport, authentication, playback or complete normalization parity verified.' }, null, 2) + '\n');
