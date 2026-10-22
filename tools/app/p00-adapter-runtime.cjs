'use strict';

// Optional pinned-source execution. Axios I/O is replaced; no sockets are opened.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const crypto = require('node:crypto');
const { createRequire } = require('node:module');

async function loadAdapters(rootArg, esbuildArg) {
    const root = path.resolve(rootArg);
    const baseline = JSON.parse(fs.readFileSync(path.join(__dirname, '../../docs/app/baseline/source-audit.json')));
    for (const [file, expected] of Object.entries(baseline.sources)) {
        if (crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex') !== expected) {
            throw new Error(`Pinned reference hash mismatch: ${file}`);
        }
    }
    const esbuild = require(path.resolve(esbuildArg));
    const referenceRequire = createRequire(path.join(root, 'package.json'));
    const realAxios = referenceRequire('axios');
    const runtime = {
        replies: [], calls: [], warnings: [], authFailures: 0, toasts: 0, updates: [], held: [],
        currentServer: null, deviceId: 'p00-fixture-device',
        player: { player: { index: 0 }, queue: { default: [], shuffled: [], songs: {} } },
        shuffled: false,
    };
    const diagnostics = {
        warn(message, detail) { runtime.warnings.push({ message, ...(detail && { detail }) }); },
        error() { runtime.warnings.push({ message: 'Suppressed reference error log' }); },
        debug() {}, info() {}, log() {},
    };
    const transport = async (config) => {
        const url = new URL(config.url);
        if (url.origin !== 'http://p00.invalid') throw new Error('Only the offline fixture origin is allowed');
        const query = Object.fromEntries(Object.entries(config.params || {}).filter(([key]) => !['u', 'p', 's', 't'].includes(key)));
        let body = config.data;
        let formAuthentication = false;
        if (typeof body === 'string') {
            if ((config.headers.get('Content-Type') || '').includes('application/json')) body = JSON.parse(body);
            else {
                const params = new URLSearchParams(body);
                formAuthentication = params.has('t') || params.has('p');
                for (const key of ['u', 'p', 's', 't']) params.delete(key);
                body = params.toString();
            }
        }
        const call = { method: config.method.toUpperCase(), path: url.pathname, query, ...(body !== undefined && { body }),
            authenticationPresent: Boolean(config.headers.get('Authorization') || config.headers.get('x-nd-authorization') || config.params?.t || config.params?.p || formAuthentication),
            signalPresent: Boolean(config.signal) };
        runtime.calls.push(call);
        const reply = runtime.replies.shift();
        if (!reply || reply.path !== call.path || (reply.method && reply.method !== call.method)) {
            throw new Error('Unexpected offline fixture request');
        }
        if (reply.hold) await new Promise((resolve) => runtime.held.push(resolve));
        if (reply.error) throw new realAxios.AxiosError('Injected transport failure', reply.error, config);
        const response = { data: JSON.stringify(reply.body), status: reply.status, statusText: '',
            headers: new realAxios.AxiosHeaders(reply.headers || {}), config };
        if (config.validateStatus && !config.validateStatus(reply.status)) {
            throw new realAxios.AxiosError('Fixture HTTP failure', 'ERR_BAD_RESPONSE', config, undefined, response);
        }
        return response;
    };
    // create() inherits this instance's adapter, including inside the original API modules.
    const offlineAxios = realAxios.create({ adapter: transport });
    Object.assign(offlineAxios, { AxiosHeaders: realAxios.AxiosHeaders, AxiosError: realAxios.AxiosError,
        isAxiosError: realAxios.isAxiosError });
    const stubs = {
        'idb-keyval': 'export const set=async()=>{throw new Error("Unexpected fixture storage mutation")};',
        '/@/i18n/i18n': 'export default {t:(key)=>key};',
        '/@/renderer/store': `export const useAuthStore={getState:()=>({currentServer:globalThis.__p00.currentServer,
            deviceId:globalThis.__p00.deviceId, actions:{updateServer:(id,patch)=>globalThis.__p00.updates.push({id,patch})}})};`,
        '/@/renderer/api/utils': 'export const authenticationFailure=()=>{globalThis.__p00.authFailures++};',
        '/@/renderer/utils/logger': 'export const logger=globalThis.__p00Logger;',
        '/@/shared/components/toast/toast': 'export const toast={error:()=>{globalThis.__p00.toasts++}};',
        '/@/renderer/utils': 'export const randomString=()=>"fixture-salt";',
        '/@/renderer/features/player/components/audio-players': 'export const getDefaultTranscodingProfiles=()=>[]; export const getDirectPlayProfiles=()=>[];',
        '/@/renderer/features/radio/store/radio-store': 'export const useRadioStore={getState:()=>({currentStreamUrl:null})};',
        '/@/renderer/store/player.store': `export const usePlayerStoreBase={getState:()=>globalThis.__p00.player};
            export const isShuffleEnabled=()=>globalThis.__p00.shuffled;`,
    };
    const build = await esbuild.build({
        stdin: { contents: `
            export {SubsonicController} from './src/renderer/api/subsonic/subsonic-controller';
            export {NavidromeController} from './src/renderer/api/navidrome/navidrome-controller';
            export {JellyfinController} from './src/renderer/api/jellyfin/jellyfin-controller';
            export {ssApiClient} from './src/renderer/api/subsonic/subsonic-api';
            export {ndApiClient} from './src/renderer/api/navidrome/navidrome-api';
            export {jfApiClient} from './src/renderer/api/jellyfin/jellyfin-api';
            export {SongListSort,SortOrder,ServerType} from './src/shared/types/domain-types';
        `, resolveDir: root },
        tsconfigRaw: { compilerOptions: { baseUrl: root, paths: { '/@/*': ['./src/*'] } } },
        define: { 'process.env.NODE_ENV': '"production"', 'import.meta.env.VITE_VALIDATE_API_RESPONSES': '"true"' },
        bundle: true, platform: 'node', format: 'cjs', write: false, metafile: true, logLevel: 'silent', external: ['axios'],
        plugins: [{ name: 'explicit-p00-adapter-boundaries', setup(build) {
            build.onResolve({ filter: /.*/ }, (args) => Object.hasOwn(stubs, args.path) ? { path: args.path, namespace: 'p00-boundary' } : undefined);
            build.onLoad({ filter: /.*/, namespace: 'p00-boundary' }, (args) => ({ contents: stubs[args.path], loader: 'js' }));
        } }],
    });
    const context = vm.createContext({ module: { exports: {} }, exports: {},
        require: (name) => { if (name === 'util') return require('node:util'); if (name !== 'axios') throw new Error('Unexpected external reference import: ' + name); return offlineAxios; },
        __p00: runtime, __p00Logger: diagnostics, console: diagnostics, navigator: { userAgent: 'P00 offline fixture' },
        TextEncoder, TextDecoder, URL, URLSearchParams, Headers, Request, Response, AbortController, AbortSignal, Buffer, setTimeout, clearTimeout,
    });
    vm.runInContext(build.outputFiles[0].text, context, { timeout: 10000 });
    return { api: context.module.exports, runtime, provenance: {
        commit: baseline.commit, compilerVersion: esbuild.version, axiosVersion: realAxios.VERSION,
        sourceFiles: Object.keys(build.metafile.inputs).map((file) => path.relative(root, path.resolve(file))).filter((file) => file.startsWith('src/')).sort(),
        boundaries: Object.keys(stubs), responseValidation: 'enabled, diagnostic only as in original source',
        transport: 'Actual frozen Axios with an offline adapter; no sockets, server state or request success implied',
    } };
}
module.exports = { loadAdapters };
