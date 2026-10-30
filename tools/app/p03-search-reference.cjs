#!/usr/bin/env node
'use strict';
// Run selected hash-bound search/normalization expressions without network access.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),vm=require('node:vm'),assert=require('node:assert/strict');
const fixturePath=path.resolve(__dirname,'../../tests/fixtures/p03/subsonic-search.json');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
function checked(file,hash){const bytes=fs.readFileSync(file);if(sha(bytes)!==hash)throw new Error('hash mismatch');return bytes.toString('utf8');}
async function replay(sourceRoot,compiler,write=false){
 const fixture=JSON.parse(fs.readFileSync(fixturePath,'utf8'));
 const sources=Object.fromEntries(Object.entries(fixture.sources).map(([name,hash])=>[name,checked(path.join(sourceRoot,name),hash)]));
 checked(compiler,fixture.typescript_sha256);const ts=require(path.resolve(compiler));
 const names=['coerceYear','parsePartialIsoDate','getArtistList','getParticipants','getGenres','pad2','subsonicReleaseFields','subsonicTrackYearField','normalizeSong','getReleaseType','normalizeAlbum','normalizeAlbumArtist'];
 const expressions=[],functions={},bindings={},routes={};let controller;
 for(const[name,source]of Object.entries(sources)){
  const file=ts.createSourceFile(name,source,ts.ScriptTarget.Latest,true);
  function capture(node,text){const start=file.getLineAndCharacterOfPosition(node.getStart(file));functions[`${name}:${start.line+1}:${start.character+1}`]=sha(text);}
  function visit(node){
   if(ts.isVariableDeclaration(node)&&node.name.getText(file)==='PRIMARY_RELEASE_TYPES'){
    if(!ts.isArrayLiteralExpression(node.initializer)||node.initializer.elements.some(e=>!ts.isStringLiteral(e)))throw new Error('unexpected release types');bindings.PRIMARY_RELEASE_TYPES=node.initializer.elements.map(e=>e.text);
   }
   if(ts.isEnumDeclaration(node)&&['LibraryItem','ServerType','ExplicitStatus'].includes(node.name.text)){
    bindings[node.name.text]=Object.fromEntries(node.members.map(m=>{if(!ts.isStringLiteral(m.initializer))throw new Error('unexpected enum');return[m.name.getText(file),m.initializer.text];}));
   }
   if(ts.isVariableDeclaration(node)&&names.includes(node.name.getText(file))&&ts.isArrowFunction(node.initializer)){
    const text=node.initializer.getText(file);capture(node.initializer,text);expressions.push(`const ${node.name.getText(file)}=(${text});`);
   }
   if(ts.isFunctionDeclaration(node)&&node.name.text==='getLibraryId'){const text=node.getText(file);capture(node,text);expressions.push(text);}
   if(ts.isPropertyAssignment(node)&&node.name.getText(file)==='search'&&ts.isArrowFunction(node.initializer)){controller=node.initializer.getText(file);capture(node.initializer,controller);}
   if(ts.isPropertyAssignment(node)&&node.name.getText(file)==='search3'&&ts.isObjectLiteralExpression(node.initializer)){
    const p=Object.fromEntries(node.initializer.properties.filter(ts.isPropertyAssignment).map(p=>[p.name.getText(file),p.initializer]));
    if(!p.path||!p.method||!ts.isStringLiteral(p.path)||!ts.isStringLiteral(p.method))throw new Error('unexpected route');routes.search3={path:p.path.text,method:p.method.text};
   }
   ts.forEachChild(node,visit);
  }
  visit(file);
 }
 if(Object.keys(functions).length!==14||Object.keys(bindings).length!==4||Object.keys(routes).length!==1)throw new Error('missing expressions');
 if(!write){assert.deepEqual(functions,fixture.functions);assert.deepEqual(bindings,fixture.bindings);assert.deepEqual(routes,fixture.routes);}
 const code=ts.transpileModule(expressions.join('\n')+'\nconst ssNormalize={song:normalizeSong,album:normalizeAlbum,albumArtist:normalizeAlbumArtist};\nmodule.exports=('+controller+');',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 const cases=[];
 for(const input of fixture.cases){
  const module={exports:{}};const requests=[];let index=0;
  vm.runInNewContext(code,{module,...bindings,PARTIAL_ISO:/^\d{4}(-\d{2}(-\d{2})?)?$/,ssApiClient(){return{search3:async args=>{
   if(index>=input.responses.length)throw new RangeError('fixture exhausted');const r=input.responses[index++];requests.push(JSON.parse(JSON.stringify({endpoint:'search3',args})));
   return{status:r.status??200,body:r.body};
  }};}},{timeout:1000});
  const result={...input};delete result.expected;delete result.expected_error;
  try{result.expected=JSON.parse(JSON.stringify(await module.exports({apiClientProps:{server:{id:'server-a'}},query:input.query})));}
  catch(e){if(!['TypeError','Error'].includes(e.name))throw e;result.expected_error=e.name;}
  result.requests=requests;cases.push(result);
 }
 const actual={...fixture,functions,bindings,routes,cases};
 if(write)fs.writeFileSync(fixturePath,JSON.stringify(actual,null,2)+'\n');else assert.deepEqual(actual,fixture);
 return cases.length;
}
if(require.main===module){const[source,compiler,mode]=process.argv.slice(2);if(!source||!compiler||(mode&&mode!=='--write'))process.exitCode=1;
 else replay(source,compiler,mode==='--write').then(n=>console.log(`Pinned search: ${n} synthetic cases passed; no network.`),()=>{console.error('Pinned search replay failed; verify trusted inputs/hashes/fixture.');process.exitCode=1;});}
module.exports={replay};
