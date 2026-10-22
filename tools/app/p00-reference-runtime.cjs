'use strict';

// Optional P00 source execution. This does not implement any Rust application behavior.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const crypto = require('node:crypto');

async function loadReference(rootArg, esbuildArg, seed = 1) {
    const root = path.resolve(rootArg);
    const baseline = JSON.parse(fs.readFileSync(path.join(__dirname, '../../docs/app/baseline/source-audit.json')));
    // Check the baseline before evaluating source, not after recording a commit claim.
    for (const [file, expected] of Object.entries(baseline.sources)) {
        const actual = crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');
        if (actual !== expected) throw new Error(`Pinned reference hash mismatch: ${file}`);
    }
    const esbuild = require(path.resolve(esbuildArg));
    const runtime = {
        data: new Map(), local: new Map(), operations: [], events: [], warnings: 0,
        failWriteAt: null, writes: 0, ids: 0, random: seed >>> 0,
        settings: { general: { resume: true, upNextQueue: true, skipButtons: { skipBackwardSeconds: 10, skipForwardSeconds: 10 } }, playback: {} },
        radio: { currentStreamUrl: null, actions: { clear() { runtime.radio.currentStreamUrl = null; } } },
        async read(key) { runtime.operations.push(['get', key]); return runtime.data.get(key); },
        async write(key, value) {
            runtime.writes++;
            if (runtime.writes === runtime.failWriteAt) throw new Error('Injected storage interruption');
            runtime.operations.push(['set', key]); runtime.data.set(key, value);
        },
        async remove(key) { runtime.operations.push(['del', key]); runtime.data.delete(key); },
    };
    const stubs = {
        'idb-keyval': 'export const get=(key)=>globalThis.__p00.read(key); export const set=(key,value)=>globalThis.__p00.write(key,value); export const del=(key)=>globalThis.__p00.remove(key);',
        nanoid: 'export const nanoid=()=>"occ-"+(++globalThis.__p00.ids);',
        '/@/renderer/store/settings.store': 'export const useSettingsStore=Object.assign((select)=>select(globalThis.__p00.settings),{getState:()=>globalThis.__p00.settings});',
        '/@/renderer/features/radio/hooks/use-radio-player': 'export const useRadioStore={getState:()=>globalThis.__p00.radio};',
        '/@/renderer/utils/logger': 'export const logger={warn(){globalThis.__p00.warnings++},error(){globalThis.__p00.warnings++},debug(){},info(){}};',
    };
    const build = await esbuild.build({
        stdin: { contents: `
            export * from './src/renderer/store/player.store';
            export * from './src/renderer/store/utils';
            export * from './src/renderer/store/timestamp.store';
            export {eventEmitter} from './src/renderer/events/event-emitter';
            export {Play,PlayerRepeat,PlayerShuffle,PlayerStatus,ServerType} from './src/shared/types/types';
            export {ssNormalize} from './src/shared/api/subsonic/subsonic-normalize';
        `, resolveDir: root },
        tsconfigRaw: { compilerOptions: { baseUrl: root, paths: { '/@/*': ['./src/*'] } } },
        define: { 'process.env.NODE_ENV': '"production"' },
        bundle: true, platform: 'node', format: 'cjs', write: false, metafile: true, logLevel: 'silent',
        plugins: [{ name: 'explicit-p00-boundaries', setup(build) {
            build.onResolve({ filter: /.*/ }, (args) => Object.hasOwn(stubs, args.path) ? { path: args.path, namespace: 'p00-boundary' } : undefined);
            build.onLoad({ filter: /.*/, namespace: 'p00-boundary' }, (args) => ({ contents: stubs[args.path], loader: 'js' }));
        } }],
    });
    const localStorage = {
        getItem: (key) => runtime.local.get(key) ?? null,
        setItem: (key, value) => runtime.local.set(key, String(value)),
        removeItem: (key) => runtime.local.delete(key),
    };
    const context = vm.createContext({ module: { exports: {} }, exports: {}, require,
        __p00: runtime, localStorage, console: { warn() { runtime.warnings++; }, error() { runtime.warnings++; }, log() {} },
        crypto: { getRandomValues(array) {
            for (let i = 0; i < array.length; i++) { runtime.random = (Math.imul(runtime.random, 1664525) + 1013904223) >>> 0; array[i] = runtime.random; }
            return array;
        } }, setTimeout, clearTimeout, setInterval, clearInterval,
    });
    vm.runInContext(build.outputFiles[0].text, context, { timeout: 10000 });
    const api = context.module.exports;
    const originalEmit = api.eventEmitter.emit.bind(api.eventEmitter);
    api.eventEmitter.emit = (event, payload) => { runtime.events.push({ event, payload: JSON.parse(JSON.stringify(payload)) }); originalEmit(event, payload); };
    await api.usePlayerStoreBase.persist.rehydrate();
    await api.useTimestampStoreBase.persist.rehydrate();
    const sourceFiles = Object.keys(build.metafile.inputs).map((file) => path.relative(root, path.resolve(file))).filter((file) => file.startsWith('src/')).sort();
    return { api, runtime, provenance: { commit: baseline.commit, compilerVersion: esbuild.version, sourceFiles,
        boundaries: Object.keys(stubs), random: 'LCG seed ' + seed, storage: 'in-memory async adapter and localStorage; no browser IndexedDB',
        limitation: 'Actual pinned Zustand/Immer store and persistence code; no React UI, engine, OS, HTTP or browser lifecycle.' } };
}

function loadProgressCallback(root, parserArg, options = {}) {
    const ts = require(path.resolve(parserArg));
    if (ts.version !== '6.0.3') throw new Error('Use baseline parser TypeScript 6.0.3');
    const file = 'src/renderer/features/player/hooks/use-scrobble.ts';
    const bytes = fs.readFileSync(path.join(root, file));
    const baseline = JSON.parse(fs.readFileSync(path.join(__dirname, '../../docs/app/baseline/source-audit.json')));
    if (crypto.createHash('sha256').update(bytes).digest('hex') !== baseline.sources[file]) throw new Error('Pinned scrobble source hash mismatch');
    const source = ts.createSourceFile(file, bytes.toString(), ts.ScriptTarget.Latest, true);
    const names = ['getPositionValue', 'checkScrobbleConditions', 'handleScrobbleFromProgress'];
    const declarations = new Map();
    function visit(node) {
        if (ts.isVariableDeclaration(node) && names.includes(node.name.getText(source))) declarations.set(node.name.getText(source), node);
        ts.forEachChild(node, visit);
    }
    visit(source);
    if (declarations.size !== names.length) throw new Error('Missing scrobble callback declaration');
    const requests = [];
    const state = { status: 'playing', timestamp: 0 };
    const refs = Object.fromEntries(Object.entries({ isCurrentSongScrobbledRef: false, listenedMsRef: 0, lastListenSampleTimeRef: null,
        scrobbleAtDurationMsRef: options.durationThresholdMs ?? 60000, scrobbleAtPercentageRef: 50, minimumModeRef: 'both',
        lastProgressEventRef: 0, previousTimestampRef: 0 }).map(([key, current]) => [key, { current }]));
    const song = { id: 'fixture-song', _serverId: 'fixture-server', _serverType: options.serverType ?? 'subsonic', _itemType: 'song',
        _uniqueId: 'occ-fixture', albumId: 'fixture-album', duration: 180000, isResumable: options.resumable ?? false };
    const context = vm.createContext({ ...refs, useCallback: (fn) => fn, isScrobbleEnabled: true,
        isPrivateModeEnabled: options.privateMode ?? false, playbackRate: options.speed ?? 1,
        PlayerStatus: { PLAYING: 'playing' }, ServerType: { JELLYFIN: 'jellyfin' },
        ScrobbleMinimumMode: { BOTH: 'both', PERCENTAGE: 'percentage', SECONDS: 'seconds' },
        MAX_LISTEN_DELTA_SEC: 5, SCROBBLE_TRACK_BEGIN_SEC: 5, SCROBBLE_RESTART_PREVIOUS_MIN_SEC: 10,
        usePlayerStore: { getState: () => ({ getCurrentSong: () => song, player: { status: state.status } }) },
        sendScrobble: { mutate: (request) => requests.push(JSON.parse(JSON.stringify(request))) },
        sendProgressAfterSubmission() {}, logger: { debug() {}, info() {} },
    });
    const text = names.map((name) => `const ${declarations.get(name).getText(source)};`).join('\n');
    const js = ts.transpileModule(text, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
    vm.runInContext(js + '\n globalThis.progress=handleScrobbleFromProgress;', context, { timeout: 1000 });
    return { requests, refs, sample(timestamp, status = 'playing') {
        const previous = state.timestamp; state.timestamp = timestamp; state.status = status;
        context.progress({ timestamp }, { timestamp: previous });
        return { positionSec: timestamp, status, listenedMs: refs.listenedMsRef.current, submitted: refs.isCurrentSongScrobbledRef.current };
    }, provenance: { source: file, boundaries: 'Exact progress callback, refs/settings/player snapshot, useCallback identity and request recorder; no React effects/timers, success callback or network.',
        upstream_ids: names.map((name) => { const n = declarations.get(name).initializer;
            const pos = source.getLineAndCharacterOfPosition((ts.isCallExpression(n) ? n.arguments[0] : n).getStart(source));
            return `${file}:${pos.line + 1}:${pos.character + 1}`; }) } };
}

module.exports = { loadReference, loadProgressCallback };
