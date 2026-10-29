#!/usr/bin/env node
'use strict';
// Only pinned, hash-verified expressions execute, with an in-memory API double.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const fixturePath = path.resolve(__dirname, '../../tests/fixtures/p03/subsonic-list-reads.json');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function checked(file, hash) {
    const bytes = fs.readFileSync(file);
    if (sha(bytes) !== hash) throw new Error('hash mismatch');
    return bytes.toString('utf8');
}
async function replay(sourceRoot, compiler, lodashPath, write = false) {
    const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
    const sources = Object.fromEntries(Object.entries(fixture.sources).map(([name, hash]) => [name, checked(path.join(sourceRoot, name), hash)]));
    checked(compiler, fixture.typescript_sha256);
    checked(lodashPath, fixture.lodash_sha256);
    const ts = require(path.resolve(compiler));
    const lodash = require(path.resolve(lodashPath));
    if (lodash.VERSION !== '4.17.21') throw new Error('unexpected Lodash');
    const names = ['normalizePlaylist','normalizeGenre'];
    const controllers = ['getPlaylistList','getPlaylistListCount','getGenreList'];
    const endpoints = ['getPlaylists','getGenres'];
    const routes = {};
    const bindings = {}, expressions = [], calls = {}, functions = {};
    for (const [name, source] of Object.entries(sources)) {
        const file = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, true);
        function visit(node) {
            if (ts.isEnumDeclaration(node) && ['LibraryItem','ServerType','PlaylistListSort','GenreListSort','SortOrder'].includes(node.name.text)) {
                bindings[node.name.text] = Object.fromEntries(node.members.map(member => {
                    if (!member.initializer || !ts.isStringLiteral(member.initializer)) throw new Error('unexpected enum');
                    return [member.name.getText(file), member.initializer.text];
                }));
            }
            if (ts.isPropertyAssignment(node) && endpoints.includes(node.name.getText(file)) && ts.isObjectLiteralExpression(node.initializer)) {
                const props = Object.fromEntries(node.initializer.properties.filter(ts.isPropertyAssignment).map(p => [p.name.getText(file), p.initializer]));
                if (!props.path || !props.method || !ts.isStringLiteral(props.path) || !ts.isStringLiteral(props.method)) throw new Error('unexpected route');
                routes[node.name.getText(file)] = {path:props.path.text, method:props.method.text};
            }
            if (ts.isFunctionDeclaration(node) && node.name.text === 'sortAndPaginate') {
                const text = node.getText(file), start = file.getLineAndCharacterOfPosition(node.getStart(file));
                functions[`${name}:${start.line+1}:${start.character+1}`] = sha(text);
                expressions.push(text);
            }
            if ((ts.isVariableDeclaration(node) && names.includes(node.name.getText(file))) || (ts.isPropertyAssignment(node) && controllers.includes(node.name.getText(file)) && ts.isArrowFunction(node.initializer))) {
                const text = node.initializer.getText(file);
                const start = file.getLineAndCharacterOfPosition(node.initializer.getStart(file));
                functions[`${name}:${start.line+1}:${start.character+1}`] = sha(text);
                if (controllers.includes(node.name.getText(file))) calls[node.name.getText(file)] = text;
                else expressions.push(`const ${node.name.getText(file)} = (${text});`);
            }
            ts.forEachChild(node, visit);
        }
        visit(file);
    }
    if (Object.keys(functions).length !== 6 || Object.keys(bindings).length !== 5 || Object.keys(routes).length !== 2) throw new Error('missing expressions');
    if (!write) { assert.deepEqual(functions, fixture.functions); assert.deepEqual(routes, fixture.routes); }
    const cases = [];
    for (const input of fixture.cases) {
        const module = { exports: {} };
        let request;
        const code = ts.transpileModule(expressions.join('\n') + '\nconst ssNormalize = {playlist:normalizePlaylist,genre:normalizeGenre};\nmodule.exports = (' + calls[input.operation] + ');', {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
        vm.runInNewContext(code, {module, ...bindings, filter:lodash.filter, orderBy:lodash.orderBy,
            ssApiClient() { return Object.fromEntries(endpoints.map(endpoint => [endpoint, async args => {
                request = JSON.parse(JSON.stringify({endpoint, args}));
                return {status:200,body:input.input};
            }])); }
        }, {timeout:1000});
        const result = {...input}; delete result.expected; delete result.expected_error;
        try {
            result.expected = JSON.parse(JSON.stringify(await module.exports({apiClientProps:{server:{id:'server-a'}}, query:input.query || {}})));
        } catch (error) {
            if (error.name !== 'TypeError') throw error;
            result.expected_error = 'TypeError';
        }
        result.request = request;
        cases.push(result);
    }
    const actual = {...fixture, functions, routes, cases};
    if (write) fs.writeFileSync(fixturePath, JSON.stringify(actual,null,2)+'\n');
    else assert.deepEqual(actual,fixture);
    return cases.length;
}
if (require.main === module) {
    const [source,compiler,lodashPath,mode] = process.argv.slice(2);
    if (!source || !compiler || !lodashPath || (mode && mode !== '--write')) { process.exitCode=1; }
    else replay(source,compiler,lodashPath,mode === '--write').then(count => console.log(`Pinned list reads: ${count} synthetic cases passed; no network.`), () => { console.error('Pinned list replay failed; verify trusted inputs/hashes/fixture.'); process.exitCode=1; });
}
module.exports={replay};
