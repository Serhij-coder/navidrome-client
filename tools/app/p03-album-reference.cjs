#!/usr/bin/env node
'use strict';
// Only pinned, hash-verified expressions execute, with an in-memory API double.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const fixturePath = path.resolve(__dirname, '../../tests/fixtures/p03/subsonic-album-reads.json');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function checked(file, hash) {
    const bytes = fs.readFileSync(file);
    if (sha(bytes) !== hash) throw new Error('hash mismatch');
    return bytes.toString('utf8');
}
async function replay(sourceRoot, compiler, write = false) {
    const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
    const sources = Object.fromEntries(Object.entries(fixture.sources).map(([name, hash]) => [name, checked(path.join(sourceRoot, name), hash)]));
    checked(compiler, fixture.typescript_sha256);
    const ts = require(path.resolve(compiler));
    const names = ['coerceYear','parsePartialIsoDate','getArtistList','getParticipants','getGenres','pad2','subsonicReleaseFields','subsonicTrackYearField','normalizeSong','normalizeAlbumArtist','getReleaseType','normalizeAlbum'];
    const controllers = ['getAlbumDetail','getAlbumArtistDetail'];
    const bindings = {}, expressions = [], calls = {}, functions = {};
    for (const [name, source] of Object.entries(sources)) {
        const file = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, true);
        function visit(node) {
            if (ts.isEnumDeclaration(node) && ['LibraryItem','ServerType','ExplicitStatus'].includes(node.name.text)) {
                bindings[node.name.text] = Object.fromEntries(node.members.map(member => {
                    if (!member.initializer || !ts.isStringLiteral(member.initializer)) throw new Error('unexpected enum');
                    return [member.name.getText(file), member.initializer.text];
                }));
            }
            if (ts.isVariableDeclaration(node) && node.name.getText(file) === 'PRIMARY_RELEASE_TYPES') {
                if (!ts.isArrayLiteralExpression(node.initializer)) throw new Error('unexpected release types');
                bindings.PRIMARY_RELEASE_TYPES = node.initializer.elements.map(element => {
                    if (!ts.isStringLiteral(element)) throw new Error('unexpected release type');
                    return element.text;
                });
            }
            if ((ts.isVariableDeclaration(node) && names.includes(node.name.getText(file))) || (ts.isPropertyAssignment(node) && controllers.includes(node.name.getText(file)))) {
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
    if (Object.keys(functions).length !== 14 || Object.keys(bindings).length !== 4) throw new Error('missing expressions');
    if (!write) assert.deepEqual(functions, fixture.functions);
    const cases = [];
    for (const input of fixture.cases) {
        const module = { exports: {} };
        let request;
        const code = ts.transpileModule(expressions.join('\n') + '\nconst ssNormalize = {album:normalizeAlbum,albumArtist:normalizeAlbumArtist};\nmodule.exports = (' + calls[input.kind === 'album' ? 'getAlbumDetail' : 'getAlbumArtistDetail'] + ');', {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
        vm.runInNewContext(code, {module, ...bindings, PARTIAL_ISO:/^\d{4}(-\d{2}(-\d{2})?)?$/,
            ssApiClient() { return Object.fromEntries(['getAlbum','getArtist'].map(endpoint => [endpoint, async args => {
                request = {endpoint, id:args.query.id};
                return {status:200,body:{[input.kind === 'album' ? 'album' : 'artist']: input.input}};
            }])); }
        }, {timeout:1000});
        const result = {...input}; delete result.expected; delete result.expected_error;
        try {
            result.expected = JSON.parse(JSON.stringify(await module.exports({apiClientProps:{server:{id:'server-a'}}, query:{id:String(input.input.id)}})));
        } catch (error) {
            if (error.name !== 'TypeError') throw error;
            result.expected_error = 'TypeError';
        }
        result.request = request;
        cases.push(result);
    }
    const actual = {...fixture, functions, cases};
    if (write) fs.writeFileSync(fixturePath, JSON.stringify(actual,null,2)+'\n');
    else assert.deepEqual(actual,fixture);
    return cases.length;
}
if (require.main === module) {
    const [source,compiler,mode] = process.argv.slice(2);
    if (!source || !compiler || (mode && mode !== '--write')) { process.exitCode=1; }
    else replay(source,compiler,mode === '--write').then(count => console.log(`Pinned album/artist reads: ${count} synthetic cases passed; no network.`), () => { console.error('Pinned album replay failed; verify trusted inputs/hashes/fixture.'); process.exitCode=1; });
}
module.exports={replay};
