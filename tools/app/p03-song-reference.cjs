#!/usr/bin/env node
// Executes only hash-bound expressions from the pinned GPL-3.0 Feishin source.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const fixturePath = path.resolve(__dirname, '../../tests/fixtures/p03/subsonic-song-reads.json');
const sha = s => crypto.createHash('sha256').update(s).digest('hex');
function checked(file, expected) {
    const bytes = fs.readFileSync(file);
    if (sha(bytes) !== expected) throw new Error('hash mismatch');
    return bytes.toString('utf8');
}
function replay(sourceRoot, compilerPath, write = false) {
    const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
    const sources = Object.fromEntries(Object.entries(fixture.sources).map(([name, hash]) => [name, checked(path.join(sourceRoot, name), hash)]));
    checked(compilerPath, fixture.typescript_sha256);
    const ts = require(path.resolve(compilerPath));
    const names = ['coerceYear', 'parsePartialIsoDate', 'getArtistList', 'getParticipants', 'getGenres', 'pad2', 'subsonicReleaseFields', 'subsonicTrackYearField', 'normalizeSong'];
    const functions = {};
    const enumBindings = {};
    const expressions = [];
    for (const [name, source] of Object.entries(sources)) {
        const file = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, true);
        function visit(node) {
            if (ts.isEnumDeclaration(node) && ['LibraryItem', 'ServerType', 'ExplicitStatus'].includes(node.name.text)) {
                enumBindings[node.name.text] = Object.fromEntries(node.members.map(member => {
                    if (!member.initializer || !ts.isStringLiteral(member.initializer)) throw new Error('unexpected enum');
                    return [member.name.getText(file), member.initializer.text];
                }));
            }
            if (ts.isVariableDeclaration(node) && names.includes(node.name.getText(file))) {
                const expression = node.initializer.getText(file);
                const start = file.getLineAndCharacterOfPosition(node.initializer.getStart(file));
                const id = `${name}:${start.line + 1}:${start.character + 1}`;
                functions[id] = sha(expression);
                expressions.push(`const ${node.name.getText(file)} = (${expression});`);
            }
            ts.forEachChild(node, visit);
        }
        visit(file);
    }
    if (Object.keys(functions).length !== names.length || Object.keys(enumBindings).length !== 3) throw new Error('missing expression');
    if (!write) assert.deepEqual(functions, fixture.functions);
    const code = ts.transpileModule(expressions.join('\n') + '\nmodule.exports = normalizeSong;', {
        compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
    }).outputText;
    const module = { exports: {} };
    vm.runInNewContext(code, {
        module, PARTIAL_ISO: /^\d{4}(-\d{2}(-\d{2})?)?$/,
        ...enumBindings,
    }, { timeout: 1000 });
    const cases = fixture.cases.map(c => {
        try {
            return { name: c.name, input: c.input, expected: JSON.parse(JSON.stringify(module.exports(c.input, { id: 'server-a' }))) };
        } catch (error) {
            if (error.name !== 'TypeError') throw error;
            return { name: c.name, input: c.input, expected_error: 'TypeError' };
        }
    });
    const actual = { ...fixture, functions, cases };
    if (write) fs.writeFileSync(fixturePath, JSON.stringify(actual, null, 2) + '\n');
    else assert.deepEqual(actual, fixture);
    return cases.length;
}
if (require.main === module) {
    try {
        const [source, compiler, mode] = process.argv.slice(2);
        if (!source || !compiler || (mode && mode !== '--write')) throw new Error('usage');
        console.log(`Pinned song normalizer: ${replay(source, compiler, mode === '--write')} synthetic cases passed; no network.`);
    } catch {
        console.error('Pinned song replay failed; verify trusted inputs, hashes and fixture.');
        process.exitCode = 1;
    }
}
module.exports = { replay };
