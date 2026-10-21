#!/usr/bin/env node
'use strict';

// Optional reference analysis. Never used by application builds or CI.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');

const [rootArg, parserArg] = process.argv.slice(2);
if (!rootArg || !parserArg) {
    console.error('Usage: node tools/app/p00-source-audit.cjs REFERENCE_ROOT TYPESCRIPT_JS');
    process.exit(2);
}
const root = path.resolve(rootArg);
const ts = require(path.resolve(parserArg));
if (ts.version !== '6.0.3') throw new Error('Use baseline parser TypeScript 6.0.3');
const read = (p) => fs.readFileSync(path.join(root, p), 'utf8');
const hash = (s) => crypto.createHash('sha256').update(s).digest('hex');
const parsed = new Map();
const sources = {};
function parse(p) {
    if (!parsed.has(p)) {
        const text = read(p);
        sources[p] = hash(text);
        const file = ts.createSourceFile(p, text, ts.ScriptTarget.Latest, true);
        if (file.parseDiagnostics.length) throw new Error(`Parse diagnostics in ${p}`);
        parsed.set(p, file);
    }
    return parsed.get(p);
}
function visit(node, fn) {
    fn(node);
    ts.forEachChild(node, (child) => visit(child, fn));
}
function line(node, file) {
    return file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1;
}
function anchor(node, file, p) {
    const pos = file.getLineAndCharacterOfPosition(node.getStart(file));
    return `${p}:${pos.line + 1}:${pos.character + 1}`;
}
function resolveImport(from, spec) {
    spec = spec.replace(/\?.*$/, ''); // Vite resource query, e.g. CSS ?inline.
    let base;
    if (spec.startsWith('/@/')) {
        // All source namespace aliases except the target-dependent conversion API.
        if (spec === '/@/lyrics-conversion-api') return { targetDependent: true };
        base = 'src/' + spec.slice(3);
    } else if (spec.startsWith('.')) {
        base = path.posix.normalize(path.posix.join(path.posix.dirname(from), spec));
    } else return { external: true };
    const candidates = [base, ...['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '.json', '.css', '.scss'].map((e) => base + e), ...['ts', 'tsx', 'js', 'jsx'].map((e) => `${base}/index.${e}`)];
    const found = candidates.find((p) => fs.existsSync(path.join(root, p)) && fs.statSync(path.join(root, p)).isFile());
    return found ? { path: found } : { unresolved: base };
}
const edges = [];
const dynamic = [];
function walkDir(dir) {
    for (const entry of fs.readdirSync(path.join(root, dir), { withFileTypes: true })) {
        const p = dir + '/' + entry.name;
        if (entry.isDirectory()) walkDir(p);
        else if (/\.[cm]?[jt]sx?$/.test(p)) {
            const file = parse(p);
            visit(file, (n) => {
                let spec;
                let kind;
                if ((ts.isImportDeclaration(n) || ts.isExportDeclaration(n)) && n.moduleSpecifier && ts.isStringLiteral(n.moduleSpecifier)) {
                    spec = n.moduleSpecifier.text;
                    kind = n.isTypeOnly || n.importClause?.isTypeOnly ? 'type' : 'static';
                } else if (ts.isCallExpression(n) && (n.expression.kind === ts.SyntaxKind.ImportKeyword || n.expression.getText(file) === 'require')) {
                    kind = 'dynamic';
                    if (n.arguments.length === 1 && ts.isStringLiteral(n.arguments[0])) spec = n.arguments[0].text;
                    else dynamic.push({ from: p, line: line(n, file), expression: n.getText(file) });
                }
                if (spec) edges.push({ from: p, line: line(n, file), kind, specifier: spec, ...resolveImport(p, spec) });
            });
        }
    }
}
walkDir('src');
const routePath = 'src/renderer/router/routes.ts';
const routeFile = parse(routePath);
const routes = [];
visit(routeFile, (n) => {
    if (ts.isEnumMember(n) && n.initializer && ts.isStringLiteral(n.initializer)) routes.push({ name: n.name.getText(routeFile), path: n.initializer.text, line: line(n, routeFile) });
});
const routerPath = 'src/renderer/router/app-router.tsx';
const router = parse(routerPath);
const registrations = [];
const remoteRegistrations = [];
function routeElements(p, destination) {
    const file = parse(p);
    visit(file, (n) => {
        if (!(ts.isJsxSelfClosingElement(n) || ts.isJsxOpeningElement(n)) || n.tagName.getText(file) !== 'Route') return;
        const attrs = Object.fromEntries(n.attributes.properties.filter(ts.isJsxAttribute).map((a) => {
            const value = a.initializer;
            return [a.name.getText(file), value && ts.isJsxExpression(value) && value.expression
                ? `{${value.expression.getText(file)}}` : value?.getText(file) ?? true];
        }));
        destination.push({ line: line(n, file), ...attrs });
    });
}
routeElements(routerPath, registrations);
routeElements('src/remote/router.tsx', remoteRegistrations);
for (const route of routes) {
    route.registrations = registrations.filter((r) => r.path === `{AppRoute.${route.name}}`).map((r) => r.line);
    route.status = route.registrations.length ? 'registered' : 'unregistered-declaration';
}
function closure(start) {
    const seen = new Set([start]);
    const pending = [start];
    while (pending.length) {
        const p = pending.pop();
        for (const e of edges.filter((e) => e.from === p && e.path && /\.[cm]?[jt]sx?$/.test(e.path))) {
            if (!seen.has(e.path)) { seen.add(e.path); pending.push(e.path); }
        }
    }
    return seen;
}
const entrypoints = ['src/renderer/main.tsx', 'src/remote/index.tsx', 'src/main/index.ts', 'src/preload/index.ts'];
const closures = Object.fromEntries(entrypoints.map((p) => [p, closure(p)]));
const compositions = ['src/renderer/features/player/audio-players.tsx', 'src/renderer/features/player/components/audio-players.tsx'].map((p) => ({
    path: p, bytes: Buffer.byteLength(read(p)), sha256: hash(read(p)),
    incoming: edges.filter((e) => e.path === p),
    roots: entrypoints.filter((e) => closures[e].has(p)),
}));

// Execute only selected exact declarations in a fresh VM, with explicit stubs.
// This is source-level characterization, never an app/server/audio runtime test.
function declaration(p, name) {
    const file = parse(p);
    let found;
    visit(file, (n) => {
        if ((ts.isVariableDeclaration(n) || ts.isFunctionDeclaration(n)) && n.name?.getText(file) === name) {
            if (found) throw new Error(`Ambiguous declaration ${name}`);
            found = n;
        }
    });
    if (!found) throw new Error(`Missing declaration ${name}`);
    const text = ts.isVariableDeclaration(found) ? `const ${found.getText(file)};` : found.getText(file).replace(/^export\s+/, '');
    // Function inventory IDs use the arrow/function expression, not its binding.
    return { text, id: anchor(ts.isVariableDeclaration(found) ? found.initializer : found, file, p) };
}
function execute(p, name, expression, bindings = {}) {
    const d = declaration(p, name);
    const js = ts.transpileModule(d.text, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText;
    const context = vm.createContext(bindings);
    const result = vm.runInContext(js + '\n' + expression, context, { timeout: 1000 });
    return { source_id: d.id, result: result === undefined ? null : JSON.parse(JSON.stringify(result)) };
}
const characterization = {};
const scrobble = 'src/renderer/features/player/hooks/use-scrobble.ts';
characterization.position = execute(scrobble, 'getPositionValue', '[getPositionValue(180, false), getPositionValue(180, true), getPositionValue(180000, false), getPositionValue(180000, true)]');
characterization.threshold = execute(scrobble, 'checkScrobbleConditions', `[
    checkScrobbleConditions({minimumMode:'both',scrobbleAtDurationMs:240000,scrobbleAtPercentage:50,songCompletedDurationMs:90000,songDurationMs:180000}),
    checkScrobbleConditions({minimumMode:'both',scrobbleAtDurationMs:60000,scrobbleAtPercentage:80,songCompletedDurationMs:60000,songDurationMs:180000})
]`, { ScrobbleMinimumMode: { BOTH: 'both', PERCENTAGE: 'percentage', SECONDS: 'seconds' } });
characterization.naturalEnd = execute('src/renderer/store/player.store.ts', 'calculateNextIndex', `['NONE','ALL','ONE'].map((repeat)=>({repeat, ...calculateNextIndex(2,3,repeat)}))`, { PlayerRepeat: { NONE: 'NONE', ALL: 'ALL', ONE: 'ONE' } });
characterization.queueCleanup = execute('src/renderer/store/utils.ts', 'cleanQueueForPersistence', `cleanQueueForPersistence({default:['occ-1','occ-2'],shuffled:[1,0],priority:['occ-2'],songs:{'occ-1':{id:'same'},'occ-2':{id:'same'},orphan:{id:'other'}}})`);
const storage = new Map();
characterization.settingsWrite = execute('src/renderer/store/utils.ts', 'splitSettingsStorage', `(() => {
    splitSettingsStorage.setItem('store_settings',JSON.stringify({state:{general:{volume:42}},version:14}));
    return {general:localStorage.getItem('store_settings_general'),version:localStorage.getItem('store_settings_version'),read:splitSettingsStorage.getItem('store_settings')};
})()`, {
    settingsKeys: ['store_settings_general'], logger: { warn() {}, error() {} },
    localStorage: { getItem: (k) => storage.get(k) ?? null, setItem: (k, v) => storage.set(k, String(v)), removeItem: (k) => storage.delete(k) },
});
characterization.settingsWrite.upstream_ids = ['src/renderer/store/utils.ts:195:14', 'src/renderer/store/utils.ts:297:14'];
for (const p of ['web.vite.config.ts', 'remote.vite.config.ts', 'electron.vite.config.ts', 'package.json', 'pnpm-lock.yaml']) sources[p] = hash(read(p));
const report = {
    schema: 1, commit: '7d6ad318c79802e226996cfc39acba6b05bb22e8', parser: ts.version,
    evidenceKind: 'static-and-isolated-declarations',
    limitations: ['Conservative module graph includes type-only edges; not a symbol call graph or runtime proof.', 'Target-dependent lyrics conversion and computed imports require manual inspection.', 'Unregistered route declarations are not proof that their feature is unused.'],
    sources, routes, registrations, remoteRegistrations, compositions,
    graph: { modules: parsed.size, imports: edges.length, roots: Object.fromEntries(entrypoints.map((p) => [p, closures[p].size])), unresolved: edges.filter((e) => e.unresolved), targetDependent: edges.filter((e) => e.targetDependent), computed: dynamic },
    characterization,
};
process.stdout.write(JSON.stringify(report, null, 2) + '\n');
