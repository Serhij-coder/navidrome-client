#!/usr/bin/env node
'use strict';
// Selected hash-bound mutation/user expressions, with disposable in-memory replies.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),vm=require('node:vm'),assert=require('node:assert/strict');
const fixturePath=path.resolve(__dirname,'../../tests/fixtures/p03/subsonic-playlist-mutations.json');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
function checked(file,hash){const b=fs.readFileSync(file);if(sha(b)!==hash)throw new Error('hash mismatch');return b.toString('utf8');}
async function replay(sourceRoot,compiler,write=false){
 const fixture=JSON.parse(fs.readFileSync(fixturePath,'utf8'));
 const sources=Object.fromEntries(Object.entries(fixture.sources).map(([n,h])=>[n,checked(path.join(sourceRoot,n),h)]));
 checked(compiler,fixture.typescript_sha256);const ts=require(path.resolve(compiler));
 const names=['addToPlaylist','createPlaylist','deletePlaylist','removeFromPlaylist','setPlaylistSongs','updatePlaylist'];
 const endpoints=['createPlaylist','updatePlaylist','deletePlaylist'];
 const functions={},calls={},bindings={},routes={};
 for(const[name,source]of Object.entries(sources)){
  const file=ts.createSourceFile(name,source,ts.ScriptTarget.Latest,true);
  function visit(node){
   if(ts.isPropertyAssignment(node)&&names.includes(node.name.getText(file))&&ts.isArrowFunction(node.initializer)){
    const text=node.initializer.getText(file),start=file.getLineAndCharacterOfPosition(node.initializer.getStart(file));functions[`${name}:${start.line+1}:${start.character+1}`]=sha(text);calls[node.name.getText(file)]=text;
   }
   if(ts.isPropertyAssignment(node)&&endpoints.includes(node.name.getText(file))&&ts.isObjectLiteralExpression(node.initializer)){
    const p=Object.fromEntries(node.initializer.properties.filter(ts.isPropertyAssignment).map(p=>[p.name.getText(file),p.initializer]));
    if(!p.path||!p.method||!ts.isStringLiteral(p.path)||!ts.isStringLiteral(p.method))throw new Error('unexpected route');routes[node.name.getText(file)]={path:p.path.text,method:p.method.text};
   }
   ts.forEachChild(node,visit);
  }visit(file);
 }
 if(Object.keys(functions).length!==6||Object.keys(bindings).length!==0||Object.keys(routes).length!==3)throw new Error('missing expressions');
 if(!write){assert.deepEqual(functions,fixture.functions);assert.deepEqual(bindings,fixture.bindings);assert.deepEqual(routes,fixture.routes);}
 const code=ts.transpileModule('module.exports={'+names.map(n=>n+':('+calls[n]+')').join(',')+'};',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 const cases=[];
 for(const input of fixture.cases){
  const module={exports:{}},requests=[];let index=0;
  vm.runInNewContext(code,{module,...bindings,ssApiClient(){return Object.fromEntries(endpoints.map(endpoint=>[endpoint,async args=>{
   if(index>=input.responses.length)throw new RangeError('fixture exhausted');const r=input.responses[index++];requests.push(JSON.parse(JSON.stringify({endpoint,args})));if(r.throw)throw new Error('synthetic transport failure');return{status:r.status??200,body:r.body??{}};
  }]));}},{timeout:1000});
  const result={...input};delete result.expected;delete result.expected_error;
  try{result.expected=JSON.parse(JSON.stringify(await module.exports[input.kind]({apiClientProps:{server:{id:'server-a'}},query:input.query||{},body:input.body||{}})));}
  catch(e){if(!['TypeError','Error'].includes(e.name))throw e;result.expected_error=e.name;}
  result.requests=requests;cases.push(result);
 }
 const actual={...fixture,functions,bindings,routes,cases};if(write)fs.writeFileSync(fixturePath,JSON.stringify(actual,null,2)+'\n');else assert.deepEqual(actual,fixture);return cases.length;
}
if(require.main===module){const[source,compiler,mode]=process.argv.slice(2);if(!source||!compiler||(mode&&mode!=='--write'))process.exitCode=1;
 else replay(source,compiler,mode==='--write').then(n=>console.log(`Pinned playlist mutations: ${n} synthetic cases passed; no network.`),()=>{console.error('Pinned playlist mutation replay failed; verify trusted inputs/hashes/fixture.');process.exitCode=1;});}
module.exports={replay};
