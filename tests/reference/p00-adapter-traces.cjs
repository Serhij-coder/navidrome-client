#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { loadAdapters } = require('../../tools/app/p00-adapter-runtime.cjs');
const fixtures = path.join(__dirname, '../fixtures/p00');
const plain = (value) => JSON.parse(JSON.stringify(value));

async function characterize(root, esbuild) {
    const { api, runtime: r, provenance } = await loadAdapters(root, esbuild);
    const captureBytes = fs.readFileSync(path.join(fixtures, 'live-responses.json'));
    const live = JSON.parse(captureBytes);
    const scrobbleBytes = fs.readFileSync(path.join(fixtures, 'scrobble-traces.json'));
    const scrobbles = JSON.parse(scrobbleBytes);
    const cases = [];
    const ssFixtures = ['navidrome-0.61.0', 'navidrome-0.47.5', 'gonic-0.16.4', 'gonic-0.15.2'];
    function server(id, type = 'subsonic') {
        return { id, type, url: 'http://p00.invalid', username: 'fixture',
            credential: 'u=fixture&s=fixture&t=fixture', ndCredential: 'fixture-native', features: {} };
    }
    function replay(fixture, endpoint, occurrence = 0) {
        const matches = live.records.map((row, index) => ({ row, index })).filter(({ row }) => row.fixture === fixture && row.path.toLowerCase().endsWith(endpoint.toLowerCase()));
        assert.ok(matches[occurrence], `Missing saved response: ${fixture} ${endpoint}`);
        const { row, index } = matches[occurrence];
        assert.ok(!row.body?.non_json, 'Non-JSON markers cannot stand in for original bytes');
        return { path: row.path.startsWith('/rest/') ? row.path + '.view' : row.path.toLowerCase(),
            status: row.http_status, body: plain(row.body), source: { kind: 'saved-live-response', record: index } };
    }
    function synthetic(endpoint, body, extras = {}) {
        return { path: endpoint, status: 200, body, source: { kind: 'synthetic' }, ...extras };
    }
    function ok(endpoint, extras = {}) {
        return synthetic(endpoint, { 'subsonic-response': { status: 'ok', version: '1.16.1' } }, extras);
    }
    async function run(id, currentServer, replies, action) {
        assert.equal(r.replies.length, 0);
        assert.equal(r.held.length, 0);
        r.calls.length = 0; r.warnings.length = 0; r.authFailures = 0; r.toasts = 0; r.updates.length = 0;
        r.currentServer = currentServer;
        r.replies.push(...replies);
        const observed = await action({ server: currentServer, serverId: currentServer?.id });
        assert.equal(r.replies.length, 0, `${id}: unconsumed response`);
        assert.equal(r.held.length, 0, `${id}: unreleased transport`);
        cases.push(plain({ id, inputs: replies.map(({ source, path, status, error, headers }) => ({ source, path, status, error, headers })),
            observed, requests: r.calls, validationWarnings: r.warnings, authFailures: r.authFailures,
            toasts: r.toasts, credentialClearRequests: r.updates.length }));
    }
    function projection(result) {
        return { startIndex: result.startIndex, totalRecordCount: result.totalRecordCount,
            songs: result.items.map(({ id, _serverId, duration, name, _itemType }) => ({ id, serverId: _serverId, durationMs: duration, name, itemType: _itemType })) };
    }
    const listQuery = { startIndex: 0, limit: 20, sortBy: api.SongListSort.NAME, sortOrder: api.SortOrder.ASC, searchTerm: 'P00' };
    for (const fixture of ssFixtures) {
        const s = server(fixture, fixture.startsWith('navidrome') ? 'navidrome' : 'subsonic');
        const controller = fixture.startsWith('navidrome') ? api.NavidromeController : api.SubsonicController;
        const ping = replay(fixture + ':admin', '/ping');
        const replies = fixture.startsWith('navidrome') ? [ping, plain(ping)] : [ping];
        if (ping.body['subsonic-response'].openSubsonic && ping.body['subsonic-response'].serverVersion) {
            replies.push(replay(fixture + ':admin', '/getOpenSubsonicExtensions'));
            replies.push(synthetic('/rest/jukeboxControl.view', { 'subsonic-response': { status: 'failed', version: '1.16.1', error: { code: 70, message: 'Synthetic unsupported jukebox' } } }));
        }
        await run(fixture + '-capabilities', s, replies, async (apiClientProps) => {
            const result = await controller.getServerInfo({ apiClientProps });
            assert.equal(ping.body['subsonic-response'].openSubsonic ? 1 : 0,
                r.calls.filter((call) => call.path === '/rest/getOpenSubsonicExtensions.view').length);
            return result;
        });
        for (const offset of [0, 2]) {
            await run(fixture + '-search-offset-' + offset, s, [replay(fixture + ':admin', '/search3', offset ? 1 : 0)], async (apiClientProps) => {
                const result = projection(await api.SubsonicController.getSongList({ apiClientProps, query: { ...listQuery, startIndex: offset, musicFolderId: ['first-library', 'second-library'] } }));
                assert.equal(offset ? 0 : 2, result.songs.length);
                assert.equal(null, result.totalRecordCount);
                assert.deepEqual(plain(r.calls[0].query.musicFolderId), ['first-library']);
                return result;
            });
        }
        for (const afterRemoval of [false, true]) {
            await run(fixture + '-playlist-' + (afterRemoval ? 'after-removal' : 'duplicates'), s,
                [replay(fixture + ':admin', '/getPlaylist', afterRemoval ? 1 : 0)], async (apiClientProps) => {
                    const playlist = r.replies[0].body['subsonic-response'].playlist;
                    const result = await api.SubsonicController.getPlaylistSongList({ apiClientProps, query: { id: String(playlist.id) } });
                    const items = result.items.map(({ id, playlistItemId }) => ({ id, playlistItemId }));
                    assert.equal(afterRemoval ? 1 : 2, items.length);
                    if (!afterRemoval) {
                        assert.equal(items[0].id, items[1].id);
                        assert.notEqual(items[0].playlistItemId, items[1].playlistItemId);
                    }
                    return { items, totalRecordCount: result.totalRecordCount };
                });
        }
        await run(fixture + '-invalid-auth-envelope', s, [replay(fixture + ':admin', '/ping', 1)], async (apiClientProps) => {
            const result = await api.ssApiClient(apiClientProps).ping();
            assert.equal(40, result.status);
            assert.equal(1, r.authFailures);
            return { clientStatus: result.status, protocolCode: result.body.error.code };
        });
        await run(fixture + '-reader-scan', s, [replay(fixture + ':reader', '/startScan')], async (apiClientProps) => {
            const result = await api.ssApiClient(apiClientProps).startScan();
            assert.equal(fixture.startsWith('navidrome') ? 50 : 200, result.status);
            return { clientStatus: result.status, protocolStatus: result.body.status };
        });
        if (fixture.startsWith('navidrome')) {
            await run(fixture + '-native-songs-missing-count-header', s, [replay(fixture + ':admin', '/api/song')], async (apiClientProps) => {
                const result = projection(await controller.getSongList({ apiClientProps, query: listQuery }));
                assert.equal(2, result.songs.length);
                assert.equal(0, result.totalRecordCount);
                return result;
            });
        }
    }
    for (const fixture of ['jellyfin-10.10.7', 'jellyfin-10.8.13']) {
        const s = server(fixture, 'jellyfin');
        s.userId = live.records.find((row) => row.fixture === fixture + ':admin' && row.path === '/Users/AuthenticateByName').body.User.Id;
        await run(fixture + '-capabilities', s, [replay(fixture + ':admin', '/System/Info')], async (apiClientProps) => api.JellyfinController.getServerInfo({ apiClientProps }));
        for (const offset of [0, 2]) {
            await run(fixture + '-songs-offset-' + offset, s, [replay(fixture + ':admin', '/Items', offset ? 1 : 0)], async (apiClientProps) => {
                const result = projection(await api.JellyfinController.getSongList({ apiClientProps, query: { ...listQuery, startIndex: offset } }));
                assert.equal(offset ? 0 : 2, result.songs.length);
                return result;
            });
        }
        await run(fixture + '-http-401', s, [replay(fixture, '/System/Info')], async (apiClientProps) => {
            const result = await api.jfApiClient(apiClientProps).getServerInfo();
            assert.equal(401, result.status);
            assert.equal(1, r.authFailures);
            assert.equal(1, r.updates.length);
            return { clientStatus: result.status };
        });
    }
    await run('old-gonic-missing-extension-envelope', server('old-gonic'), [replay('gonic-0.15.2:admin', '/getOpenSubsonicExtensions')], async (props) => {
        const result = await api.ssApiClient(props).getServerInfo();
        assert.equal(70, result.status);
        return { clientStatus: result.status, protocolCode: result.body.error.code };
    });
    await run('silent-subsonic-error-rewrites-code', server('silent'), [replay('navidrome-0.61.0:reader', '/startScan')], async (props) => {
        const result = await api.ssApiClient({ ...props, silent: true }).startScan();
        assert.equal(200, result.status);
        assert.equal('failed', result.body.status);
        assert.equal(0, result.body.error.code);
        assert.equal(0, r.toasts);
        return { clientStatus: result.status, protocolStatus: result.body.status, protocolCode: result.body.error.code };
    });
    for (const [name, code] of [['network', 'ERR_NETWORK'], ['cancellation', 'ERR_CANCELED']]) {
        await run('subsonic-' + name, server(name), [synthetic('/rest/ping.view', null, { error: code })], async (props) => {
            if (name === 'network') {
                await assert.rejects(api.ssApiClient(props).ping(), /error.networkError/);
                return { error: 'error.networkError' };
            }
            const result = await api.ssApiClient(props).ping();
            assert.equal(undefined, result.status);
            return { clientStatus: result.status ?? null, body: result.body ?? null };
        });
    }
    await run('malformed-subsonic-envelope', server('malformed'), [synthetic('/rest/ping.view', {})], async (props) => {
        await assert.rejects(api.ssApiClient(props).ping(), { name: 'TypeError' });
        return { errorClass: 'TypeError', validationReached: r.warnings.length > 0 };
    });
    await run('subsonic-pre-aborted-request', server('pre-aborted'), [], async (props) => {
        const controller = new AbortController();
        controller.abort();
        const result = await api.ssApiClient({ ...props, signal: controller.signal }).ping();
        assert.equal(undefined, result.status);
        assert.equal(0, r.calls.length);
        return { clientStatus: result.status ?? null, transportCalls: r.calls.length };
    });
    for (const formPost of [false, true]) {
        const s = server('form-post-' + formPost);
        s.features = formPost ? { osFormPost: [1], reportPlayback: [1] } : { reportPlayback: [1] };
        await run('subsonic-report-start-form-post-' + formPost, s,
            [ok('/rest/scrobble.view'), ok('/rest/reportPlayback.view'), ok('/rest/reportPlayback.view')], async (apiClientProps) => {
                await api.SubsonicController.scrobble({ apiClientProps, query: { id: 'fixture-song', event: 'start', submission: false, position: 1234.5, playbackRate: 2, mediaType: 'song' } });
                assert.equal(3, r.calls.length);
                assert.ok(r.calls.every((call) => call.method === (formPost ? 'POST' : 'GET')));
                const reports = r.calls.slice(1).map((call) => formPost ? Object.fromEntries(new URLSearchParams(call.body)) : call.query);
                assert.equal('starting', String(reports[0].state));
                assert.equal('playing', String(reports[1].state));
                assert.equal('1235', String(reports[1].positionMs));
                return { states: reports.map((report) => report.state), positionMs: reports[1].positionMs };
            });
    }
    for (const backend of ['subsonic', 'navidrome', 'jellyfin']) {
        const isJellyfin = backend === 'jellyfin';
        const sourceCase = scrobbles.cases.find((row) => row.id === (isJellyfin ? 'jellyfin-progress-and-submission-units' : 'speed-1'));
        const request = sourceCase.requests.find((row) => row.query.submission);
        const s = server('scrobble-' + backend, backend);
        const controller = api[backend === 'subsonic' ? 'SubsonicController' : backend === 'navidrome' ? 'NavidromeController' : 'JellyfinController'];
        const reply = isJellyfin ? synthetic('/sessions/playing/stopped', null, { status: 204, hold: true }) : ok('/rest/scrobble.view', { hold: true });
        await run(backend + '-submission-transport-boundary', s, [reply], async (apiClientProps) => {
            let resolved = false;
            const operation = controller.scrobble({ apiClientProps, query: request.query }).then(() => { resolved = true; });
            await new Promise(setImmediate);
            assert.equal(1, r.held.length);
            assert.equal(isJellyfin, resolved);
            const resolvedBeforeTransport = resolved;
            r.held.shift()();
            await operation;
            await new Promise(setImmediate);
            if (isJellyfin) assert.equal(1800000000000, r.calls[0].body.PositionTicks);
            else assert.equal(undefined, r.calls[0].query.position);
            return { callbackCase: sourceCase.id, callbackSubmissionPosition: request.query.position,
                resolvedBeforeTransport, outgoingSubmissionPosition: isJellyfin ? r.calls[0].body.PositionTicks : null };
        });
    }
    await run('subsonic-rejected-submission', server('submission-error'), [synthetic('/rest/scrobble.view', { 'subsonic-response': { status: 'failed', version: '1.16.1', error: { code: 50, message: 'Synthetic permission denial' } } })], async (apiClientProps) => {
        await assert.rejects(api.SubsonicController.scrobble({ apiClientProps, query: { id: 'fixture-song', submission: true } }), /Failed to scrobble/);
        return { error: 'Failed to scrobble' };
    });
    for (const event of ['start', 'unpause', 'timeupdate', 'stop']) {
        const order = Array.from({ length: 30 }, (_, i) => 'occ-' + i);
        r.player = { player: { index: 3 }, queue: { default: order, shuffled: order.map((_, i) => order.length - i - 1),
            songs: Object.fromEntries(order.map((id, i) => [id, { id: 'song-' + Math.floor(i / 2) }])) } };
        r.shuffled = true;
        const endpoint = event === 'start' ? '/sessions/playing' : event === 'stop' ? '/sessions/playing/stopped' : '/sessions/playing/progress';
        await run('jellyfin-queue-window-' + event, server('queue-window', 'jellyfin'), [synthetic(endpoint, null, { status: 204 })], async (apiClientProps) => {
            await api.JellyfinController.scrobble({ apiClientProps, query: { id: 'fixture-song', event, position: 100000000 } });
            await new Promise(setImmediate);
            const queue = r.calls[0].body.NowPlayingQueue;
            if (event === 'start' || event === 'unpause') {
                assert.equal(20, queue.length);
                assert.deepEqual(queue.slice(0, 3), [{ Id: 'song-13' }, { Id: 'song-12' }, { Id: 'song-12' }]);
            } else assert.equal(undefined, queue);
            return { reportedQueue: queue ?? null, positionTicks: r.calls[0].body.PositionTicks };
        });
    }
    return { schema: 1, tasks: ['P00.04', 'P00.06'], evidenceKind: 'pinned-adapters-with-offline-transport', provenance,
        liveCaptureSha256: crypto.createHash('sha256').update(captureBytes).digest('hex'),
        scrobbleCaptureSha256: crypto.createHash('sha256').update(scrobbleBytes).digest('hex'), cases,
        limitations: ['Redacted saved HTTP bodies/statuses replayed at Axios adapter boundary; no new live request or authentication verified.',
            'Saved capture has no headers. Missing-count-header cases deliberately replay none; native total-count observations are not server results.',
            'Jukebox replies, scrobble acknowledgements, malformed bodies and transport failures are synthetic.',
            'Auth UI/store side effects are counters; no logout, credential refresh, engine, React effects or physical storage.',
            'Request paths use pinned contract spelling; query/body records are Axios configurations before socket serialization.',
            'Jellyfin held acknowledgement succeeds after release. Failed detached requests and server/plugin interpretation remain untested.'] };
}
if (require.main === module) {
    const [root, esbuild, expected] = process.argv.slice(2);
    if (!root || !esbuild) { console.error('Usage: node tests/reference/p00-adapter-traces.cjs REFERENCE_ROOT ESBUILD_JS [EXPECTED_JSON]'); process.exit(2); }
    characterize(root, esbuild).then((report) => {
        if (expected) { assert.deepEqual(report, JSON.parse(fs.readFileSync(expected))); console.log('Pinned adapter traces match expected evidence.'); }
        else process.stdout.write(JSON.stringify(report, null, 2) + '\n');
    }).catch((error) => { console.error(error.message); process.exitCode = 1; });
}
module.exports = { characterize };
