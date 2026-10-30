#!/usr/bin/env node
'use strict';
// Only pinned, hash-verified expressions execute, with an in-memory API double.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const fixturePath = path.resolve(__dirname, '../../tests/fixtures/p03/subsonic-song-lists.json');
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
    const names = ['coerceYear','parsePartialIsoDate','getArtistList','getParticipants','getGenres','pad2','subsonicReleaseFields','subsonicTrackYearField','normalizeSong','sortSongList'];
    const controllers = ['getSongList'];
    const endpoints = ['search3','getSongsByGenre','getArtist','getAlbum','getStarred'];
    const routes = {}, constants = {};
    const bindings = {}, expressions = [], calls = {}, functions = {};
    for (const [name, source] of Object.entries(sources)) {
        const file = ts.createSourceFile(name, source, ts.ScriptTarget.Latest, true);
        function visit(node) {
            if (ts.isVariableDeclaration(node) && node.name.getText(file) === 'SortKeyRandom') {
                if (!ts.isStringLiteral(node.initializer)) throw new Error('unexpected alias');
                constants.SortKeyRandom = node.initializer.text;
            }
            if (ts.isEnumDeclaration(node) && ['LibraryItem','ServerType','ExplicitStatus','SongListSort','SortOrder'].includes(node.name.text)) {
                bindings[node.name.text] = Object.fromEntries(node.members.map(member => {
                    if (!member.initializer) throw new Error('unexpected enum');
                    const value = ts.isStringLiteral(member.initializer) ? member.initializer.text : constants[member.initializer.getText(file)];
                    if (typeof value !== 'string') throw new Error('unexpected enum');
                    return [member.name.getText(file), value];
                }));
            }
            if (ts.isPropertyAssignment(node) && endpoints.includes(node.name.getText(file)) && ts.isObjectLiteralExpression(node.initializer)) {
                const props = Object.fromEntries(node.initializer.properties.filter(ts.isPropertyAssignment).map(p => [p.name.getText(file), p.initializer]));
                if (!props.path || !props.method || !ts.isStringLiteral(props.path) || !ts.isStringLiteral(props.method)) throw new Error('unexpected route');
                routes[node.name.getText(file)] = {path:props.path.text, method:props.method.text};
            }
            if (ts.isFunctionDeclaration(node) && ['sortAndPaginate','getLibraryId'].includes(node.name.text)) {
                const text = node.getText(file), start = file.getLineAndCharacterOfPosition(node.getStart(file));
                functions[`${name}:${start.line+1}:${start.character+1}`] = sha(text);
                expressions.push(text);
            }
            if ((ts.isVariableDeclaration(node) && names.includes(node.name.getText(file)) && ts.isArrowFunction(node.initializer)) || (ts.isPropertyAssignment(node) && controllers.includes(node.name.getText(file)) && ts.isArrowFunction(node.initializer))) {
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
    if (Object.keys(functions).length !== 13 || Object.keys(bindings).length !== 5 || Object.keys(routes).length !== 5) throw new Error('missing expressions');
    if (!write) { assert.deepEqual(functions, fixture.functions); assert.deepEqual(routes, fixture.routes); assert.deepEqual(bindings,fixture.bindings); }
    async function runCase(input, timezone='UTC') {
        const previous=process.env.TZ;process.env.TZ=timezone;
        try {
            const module={exports:{}};let index=0,draws=0;const requests=[];
            const math=Object.create(Math);math.random=()=>{
                const values=input.random_draws || fixture.random_draws;
                if(draws>=values.length)throw new RangeError('RNG fixture exhausted');
                const raw=BigInt(values[draws++]);if(raw<0n||raw>18446744073709551615n)throw new RangeError('invalid u64 RNG fixture');
                return Number(raw>>11n)/9007199254740992;
            };
            const scoped=lodash.runInContext({Math:math});
            const code=ts.transpileModule(expressions.join('\n')+'\nconst ssNormalize={song:normalizeSong};\nmodule.exports=('+calls.getSongList+');',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
            vm.runInNewContext(code,{module,...bindings,PARTIAL_ISO:/^\d{4}(-\d{2}(-\d{2})?)?$/,filter:scoped.filter,orderBy:scoped.orderBy,shuffle:scoped.shuffle,dayjs(){return{year(){return input.current_year;}};},
                ssApiClient(){return Object.fromEntries(endpoints.map(endpoint=>[endpoint,async args=>{
                    if(index>=input.responses.length)throw new RangeError('response fixture exhausted');
                    const r=input.responses[index++];requests.push(JSON.parse(JSON.stringify({endpoint,args})));
                    return {status:r.status??200,body:r.body};
                }]));}
            },{timeout:1000});
            try{return{expected:JSON.parse(JSON.stringify(await module.exports({apiClientProps:{server:{id:'server-a'}},query:input.query}))),requests,rng_calls:draws};}
            catch(e){if(!['TypeError','Error'].includes(e.name))throw e;return{expected_error:e.name,requests,rng_calls:draws};}
        } finally {if(previous===undefined)delete process.env.TZ;else process.env.TZ=previous;}
    }
    const cases=[];
    for(const input of fixture.cases){
        const result={...input};for(const key of ['expected','expected_error','requests','rng_calls','alternate_expected'])delete result[key];
        Object.assign(result,await runCase(input));
        if(input.probe_timezone)result.alternate_expected=(await runCase(input,input.probe_timezone)).expected;
        cases.push(result);
    }
    const actual = {...fixture, functions, bindings, routes, cases};
    if (write) fs.writeFileSync(fixturePath, JSON.stringify(actual,null,2)+'\n');
    else assert.deepEqual(actual,fixture);
    return cases.length;
}
if (require.main === module) {
    const [source,compiler,lodashPath,mode] = process.argv.slice(2);
    if (!source || !compiler || !lodashPath || (mode && mode !== '--write')) { process.exitCode=1; }
    else replay(source,compiler,lodashPath,mode === '--write').then(count => console.log(`Pinned song lists: ${count} synthetic cases passed; no network.`), () => { console.error('Pinned song list replay failed; verify trusted inputs/hashes/fixture.'); process.exitCode=1; });
}
module.exports={replay};
