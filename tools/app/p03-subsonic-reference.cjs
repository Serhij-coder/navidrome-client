#!/usr/bin/env node
// Controlled execution of selected pinned expressions; no original module startup or I/O.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '../..');
const fixturePath = path.join(root, 'tests/fixtures/p03/subsonic-foundation.json');
const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
const hash = data => crypto.createHash('sha256').update(data).digest('hex');
function checked(file, expected) {
    const bytes = fs.readFileSync(file);
    if (hash(bytes) !== expected) throw new Error('reference hash mismatch');
    return bytes.toString('utf8');
}
function moduleValue(code, dependencies = {}) {
    const module = { exports: {} };
    vm.runInNewContext(code, {
        module, exports: module.exports,
        require(name) {
            if (!Object.hasOwn(dependencies, name)) throw new Error('unexpected reference dependency');
            return dependencies[name];
        },
    }, { timeout: 1000 });
    return module.exports;
}
async function replay(sourceRoot, compilerPath, dependenciesRoot) {
    // Validate every source/dependency before loading the compiler or executing expressions.
    const sources = Object.fromEntries(Object.entries(fixture.sources).map(([name, sha]) =>
        [name, checked(path.join(sourceRoot, name), sha)]));
    const dependencies = Object.fromEntries(Object.entries(fixture.dependencies).map(([name, sha]) =>
        [name, checked(path.join(dependenciesRoot, name), sha)]));
    checked(compilerPath, fixture.typescript_sha256);
    const ts = require(path.resolve(compilerPath));
    const expressions = {};
    for (const [id, sha] of Object.entries(fixture.functions)) {
        const [name, line, column] = id.split(':');
        const file = ts.createSourceFile(name, sources[name], ts.ScriptTarget.Latest, true);
        let selected;
        function visit(node) {
            const start = file.getLineAndCharacterOfPosition(node.getStart(file));
            if (ts.isArrowFunction(node) && start.line + 1 === Number(line)
                && start.character + 1 === Number(column)) selected = node.getText(file);
            ts.forEachChild(node, visit);
        }
        visit(file);
        if (!selected || hash(selected) !== sha) throw new Error('reference function mismatch');
        expressions[id] = ts.transpileModule(`module.exports = (${selected});`, {
            compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
        }).outputText;
    }
    function expression(id, bindings = {}) {
        const module = { exports: {} };
        vm.runInNewContext(expressions[id], { module, URLSearchParams, ...bindings }, { timeout: 1000 });
        return module.exports;
    }
    const qs = moduleValue(dependencies['qs/dist/qs.js']);
    const md5 = moduleValue(dependencies['md5/md5.js'], {
        crypt: moduleValue(dependencies['crypt/crypt.js']),
        charenc: moduleValue(dependencies['charenc/charenc.js']),
        'is-buffer': moduleValue(dependencies['is-buffer/index.js']),
    });
    const normalize = expression('src/renderer/utils/normalize-server-url.ts:3:35');
    const getServerUrl = expression('src/renderer/utils/normalize-server-url.ts:9:29');
    const parsePath = expression('src/renderer/api/subsonic/subsonic-api.ts:438:19', {
        keysToSkipEmptyCheck: new Set(['artist', 'comment', 'genre', 'name', 'query', 'u', 'username']),
    });
    const serialize = expression('src/renderer/api/subsonic/subsonic-api.ts:389:41', { qs });
    const cases = [];
    for (const input of fixture.cases) {
        let draws = 0;
        const math = Object.create(Math);
        math.random = () => ((draws++ % 62) + 0.5) / 62;
        const randomString = expression('src/renderer/utils/random-string.ts:1:29', { Math: math });
        let authRequest;
        const authenticate = expression('src/renderer/api/subsonic/subsonic-controller.ts:316:19', {
            randomString, md5,
            ssApiClient(args) {
                return { async authenticate(request) {
                    authRequest = { base: args.url, query: request.query };
                    return { status: 200, body: { user: { username: input.username, adminRole: false } } };
                } };
            },
        });
        const auth = await authenticate(input.base, {
            username: input.username, password: input.password, legacy: input.legacy,
        });
        let request;
        const api = expression('src/renderer/api/subsonic/subsonic-api.ts:489:14', {
            server: { url: normalize(input.base), credential: auth.credential, username: auth.username },
            forceRemoteUrl: false, signal: undefined, silent: false, url: undefined,
            parsePath, getServerUrl, hasFeature: () => false, ServerFeature: { OS_FORM_POST: 'form-post' },
            qs, validateResponse() {}, isAxiosError: () => false,
            axiosClient: { async request(config) {
                request = config;
                return { data: { 'subsonic-response': { status: 'ok' } }, headers: {}, status: 200 };
            } },
        });
        // Only generic URL construction is exercised; no getUser or other endpoint is sent.
        const route = 'fixture.view?' + input.pairs.map(([k, v]) =>
            `${encodeURIComponent(k)}=${encodeURIComponent(v)}`).join('&');
        const result = { ...input, expected: {
            salt: authRequest.query.s ?? null, token: authRequest.query.t ?? null,
            auth_base: authRequest.base,
        } };
        try {
            await api({ body: undefined, headers: {}, method: 'GET', path: route, rawQuery: {}, route: {} });
            result.expected.wire_url = new URL(request.url).href + '?' + serialize(request.params);
            result.expected.request_error = null;
        } catch (error) {
            if (error.name !== 'URIError') throw error;
            result.expected.wire_url = null;
            result.expected.request_error = 'URIError';
        }
        cases.push(result);
    }
    const observations = {
        route_string_normalization: JSON.parse(JSON.stringify(parsePath(
            'fixture.view?id=null&id=undefined&query=null&name=undefined&id%5B%5D=1&id%5B0%5D=2'))),
        whitespace_and_slashes: [' https://example.test/music/ ', 'https://example.test/music//']
            .map(input => ({ input, normalized: normalize(input) })),
        reserved_defaults_overridden_by_route: parsePath('fixture.view?u=override&c=other').params,
    };
    return { ...fixture, cases, observations: JSON.parse(JSON.stringify(observations)) };
}
async function main() {
    const [sourceRoot, compilerPath, dependenciesRoot, mode] = process.argv.slice(2);
    if (!sourceRoot || !compilerPath || !dependenciesRoot || (mode && mode !== '--write')) {
        throw new Error('usage: p03-subsonic-reference.cjs SOURCE_ROOT TYPESCRIPT_JS DEPENDENCIES_ROOT [--write]');
    }
    const actual = await replay(sourceRoot, compilerPath, dependenciesRoot);
    if (mode === '--write') fs.writeFileSync(fixturePath, JSON.stringify(actual, null, 2) + '\n');
    else {
        try { assert.deepEqual(actual, fixture); }
        catch { throw new Error('reference fixture mismatch'); }
    }
    console.log(`Pinned Subsonic reference: ${actual.cases.length} synthetic cases passed; no network/authentication performed.`);
}
if (require.main === module) main().catch(() => {
    // Inputs are synthetic, but keep even failure diagnostics free of payloads and paths.
    console.error('Subsonic reference replay failed; check trusted inputs, hashes and fixture.');
    process.exitCode = 1;
});
module.exports = { replay };
