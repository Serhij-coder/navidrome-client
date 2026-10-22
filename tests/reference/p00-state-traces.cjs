#!/usr/bin/env node
'use strict';

// Reference characterization, not a Rust implementation or browser test.
const fs = require('node:fs');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const { loadReference } = require('../../tools/app/p00-reference-runtime.cjs');
const plain = (value) => JSON.parse(JSON.stringify(value));
const digest = (value) => crypto.createHash('sha256').update(JSON.stringify(value)).digest('hex');
const drain = () => new Promise(setImmediate);

async function characterize(root, esbuild) {
    const { api, runtime, provenance } = await loadReference(root, esbuild, 1729);
    const store = api.usePlayerStoreBase;
    const initial = plain({ player: store.getState().player, queue: store.getState().queue });
    const song = (id, albumId = 'album-a', server = 'fixture-a') => ({ id, albumId, duration: 180000, _serverId: server, _itemType: 'song', name: id });
    const sample = [song('a'), song('a'), song('b', 'album-b')];
    const current = () => store.getState().getCurrentSong()?._uniqueId ?? null;
    const snapshot = () => {
        const s = store.getState();
        return plain({ player: s.player, canonical: s.queue.default, shuffled: s.queue.shuffled, priority: s.queue.priority,
            tracks: Object.fromEntries(s.queue.default.map((id) => [id, { id: s.queue.songs[id]?.id, server: s.queue.songs[id]?._serverId }])),
            current: current(), timestamp: api.useTimestampStoreBase.getState().timestamp });
    };
    async function reset() {
        runtime.settings.general.upNextQueue = true;
        runtime.settings.general.resume = true;
        store.setState(plain(initial)); api.setTimestamp(0); await drain();
        runtime.ids = 0; runtime.random = 1729; runtime.events.length = 0;
        runtime.data.clear(); runtime.operations.length = 0; runtime.writes = 0; runtime.failWriteAt = null;
    }
    const cases = [];
    const record = (id, input, result) => cases.push({ id, input, observed: plain(result) });
    for (const [name, mode] of Object.entries(api.Play)) {
        await reset(); store.getState().addToQueueByType(sample, api.Play.NOW);
        store.getState().addToQueueByType([song('c'), song('d')], mode, 'd');
        record('play-' + name, { initial: ['a', 'a', 'b'], insert: ['c', 'd'], mode, selectedSong: 'd' }, snapshot());
    }
    for (const repeat of Object.values(api.PlayerRepeat)) {
        for (const command of ['mediaNext', 'mediaAutoNext']) {
            await reset(); store.getState().addToQueueByType(sample, api.Play.NOW);
            store.getState().mediaPlayByIndex(2); store.getState().setRepeat(repeat);
            store.getState()[command](false);
            record(command + '-' + repeat, { index: 2, repeat }, { state: snapshot(), events: runtime.events });
        }
    }
    await reset(); store.getState().addToQueueByType(sample, api.Play.NOW);
    assert.equal(3, new Set(store.getState().queue.default).size);
    assert.equal('a', store.getState().queue.songs['occ-1'].id);
    assert.equal('a', store.getState().queue.songs['occ-2'].id);
    store.getState().mediaPlayByIndex(1); const identity = current();
    store.getState().toggleShuffle(); assert.equal(identity, current());
    store.getState().toggleShuffle(); assert.equal(identity, current());
    store.getState().clearSelected([store.getState().queue.songs['occ-1']]);
    assert.equal(identity, current());
    record('duplicate-occurrence-remove-and-shuffle', { remove: 'occ-1', current: 'occ-2' }, snapshot());
    for (const timestamp of [10, 11]) {
        await reset(); store.getState().addToQueueByType(sample, api.Play.NOW);
        store.getState().mediaPlayByIndex(1); api.setTimestamp(timestamp); runtime.events.length = 0;
        store.getState().mediaPrevious(false);
        record('previous-' + timestamp, { timestamp }, { state: snapshot(), events: runtime.events });
    }
    await reset(); store.getState().addToQueueByType(sample, api.Play.NOW);
    store.getState().addToQueueByType([song('c'), song('d')], api.Play.NEXT);
    const before = snapshot();
    store.getState().moveSelectedTo([store.getState().queue.songs['occ-3']], 'occ-4', 'top', true);
    record('up-next-boundary-pin', { move: 'occ-3', target: 'occ-4', edge: 'top', pinAbove: true }, { before, after: snapshot() });
    await reset(); store.getState().addToQueueByType(sample, api.Play.NOW);
    store.getState().setPauseOnNextSongEnd(true); store.getState().mediaAutoNext();
    assert.equal('paused', store.getState().player.status);
    assert.equal(false, store.getState().player.pauseOnNextSongEnd);
    record('pause-at-end-one-shot', {}, snapshot());

    // Stable seeded valid UI operations, capturing source state rather than substituting a reducer.
    await reset(); const trace = []; let rng = 99;
    const random = (n) => { rng = (Math.imul(rng, 1664525) + 1013904223) >>> 0; return rng % n; };
    const violations = [];
    for (let step = 0; step < 1000; step++) {
        const s = store.getState(); const ids = [...s.queue.default];
        const choice = random(7); let command;
        if (!ids.length || choice < 2) {
            const mode = ids.length ? [api.Play.LAST, api.Play.NEXT][choice] : api.Play.NOW;
            command = { type: 'add', mode, song: 'song-' + random(5) };
            s.addToQueueByType([song(command.song)], mode);
        } else if (choice === 2) { command = { type: 'toggleShuffle' }; s.toggleShuffle(); }
        else if (choice === 3) {
            const id = ids[random(ids.length)]; command = { type: 'remove', id }; s.clearSelected([s.queue.songs[id]]);
        } else if (choice === 4) {
            const id = ids[random(ids.length)]; command = { type: 'moveBottom', id }; s.moveSelectedToBottom([s.queue.songs[id]]);
        } else if (choice === 5) { command = { type: 'next' }; s.mediaNext(false); }
        else { command = { type: 'repeat', repeat: [api.PlayerRepeat.NONE, api.PlayerRepeat.ONE, api.PlayerRepeat.ALL][random(3)] }; s.setRepeat(command.repeat); }
        const state = store.getState();
        const queue = state.queue;
        const problems = [];
        if (new Set(queue.default).size !== queue.default.length || queue.default.some((id) => !queue.songs[id])) problems.push('canonical occurrence resolution');
        if (queue.shuffled.length && (queue.shuffled.length !== queue.default.length || new Set(queue.shuffled).size !== queue.default.length || queue.shuffled.some((i) => i < 0 || i >= queue.default.length))) problems.push('shuffle permutation');
        if (queue.priority.some((id) => !queue.default.includes(id))) problems.push('priority resolution');
        if (problems.length) violations.push({ step, problems });
        trace.push({ step, command, stateDigest: digest(snapshot()) });
    }
    record('seeded-queue-1000', { seed: 99, steps: 1000 }, { traceDigest: digest(trace), violations, final: snapshot() });

    // Test exact persistence module with explicit in-memory async I/O.
    await reset();
    const queue = { default: ['one', 'two'], shuffled: [1, 0], priority: ['two'], songs: { one: song('same'), two: song('same'), orphan: song('other') } };
    runtime.data.set('legacy', JSON.stringify({ version: 3, state: { player: { volume: 81 }, queue } }));
    await api.migratePlayerStorePersist('legacy');
    const migrated = await api.playerStoreStorage.getItem('legacy');
    assert.deepEqual(['one', 'two'], Object.keys(migrated.state.queue.songs));
    record('embedded-queue-v3-migration', { version: 3, occurrenceIds: ['one', 'two'], orphan: true }, migrated);
    const columns = api.mergeOverridingColumns({ list: { columns: ['title'] } }, { list: { columns: ['favorite', 'artist'], size: 30 } });
    assert.deepEqual(['title'], plain(columns.list.columns));
    record('columns-replace', { persisted: ['title'], defaults: ['favorite', 'artist'] }, columns);
    runtime.data.set('legacy-queue', '{broken');
    record('corrupt-split-queue', {}, await api.playerStoreStorage.getItem('legacy'));
    runtime.data.set('legacy', '{broken');
    assert.equal(null, await api.playerStoreStorage.getItem('legacy'));
    record('corrupt-player', {}, { result: null });
    runtime.data.clear(); runtime.writes = 0; runtime.failWriteAt = 2;
    runtime.data.set('interrupted', JSON.stringify({ version: 3, state: { player: { volume: 81 }, queue } }));
    await assert.rejects(api.migratePlayerStorePersist('interrupted'), /Injected storage interruption/);
    runtime.failWriteAt = null;
    await api.migratePlayerStorePersist('interrupted');
    record('migration-interruption-retry', { failWrite: 2 }, await api.playerStoreStorage.getItem('interrupted'));
    runtime.local.clear();
    const exported = { general: { resume: true }, autoDJ: { enabled: true }, lists: { song: { columns: ['title'] } } };
    runtime.local.set('store_settings', JSON.stringify({ state: exported, version: 14 }));
    const read = JSON.parse(api.splitSettingsStorage.getItem('store_settings'));
    assert.equal(undefined, read.state.autoDJ);
    api.splitSettingsStorage.setItem('store_settings', JSON.stringify({ state: { general: { resume: false } }, version: 34 }));
    const reread = JSON.parse(api.splitSettingsStorage.getItem('store_settings'));
    assert.equal(true, reread.state.general.resume);
    record('legacy-settings-category-loss-and-stale-write', { legacy: exported, update: { general: { resume: false } } }, { read, reread, keys: [...runtime.local.keys()].sort() });
    return { schema: 1, task: 'P00.06', evidenceKind: 'pinned-store-execution-with-explicit-boundaries', provenance, cases, generatedTrace: trace,
        limitations: ['No physical playback or UI/React lifecycle tested.', 'Storage values are synthetic historical-shape inputs, not exported user profiles.', 'Queue invariant violations, if recorded, characterize upstream; no source patch applied.'] };
}

if (require.main === module) {
    const [root, esbuild, expected] = process.argv.slice(2);
    if (!root || !esbuild) { console.error('Usage: node tests/reference/p00-state-traces.cjs REFERENCE_ROOT ESBUILD_JS [EXPECTED_JSON]'); process.exit(2); }
    characterize(root, esbuild).then((report) => {
        if (expected) { assert.deepEqual(report, JSON.parse(fs.readFileSync(expected))); console.log('Pinned state/persistence traces match expected evidence.'); }
        else process.stdout.write(JSON.stringify(report, null, 2) + '\n');
    }).catch((error) => { console.error(error.message); process.exitCode = 1; });
}
module.exports = { characterize };
