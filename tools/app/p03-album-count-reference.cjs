#!/usr/bin/env node
'use strict';
// Execute only selected hash-bound original count expressions against finite fixtures.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),vm=require('node:vm'),assert=require('node:assert/strict');
const fixturePath=path.resolve(__dirname,'../../tests/fixtures/p03/subsonic-album-count.json');
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
function checked(file,hash){const bytes=fs.readFileSync(file);if(sha(bytes)!==hash)throw new Error('hash mismatch');return bytes.toString('utf8');}
async function replay(sourceRoot,compiler,write=false){
 const fixture=JSON.parse(fs.readFileSync(fixturePath,'utf8'));
 const sources=Object.fromEntries(Object.entries(fixture.sources).map(([name,hash])=>[name,checked(path.join(sourceRoot,name),hash)]));
 checked(compiler,fixture.typescript_sha256);const ts=require(path.resolve(compiler));
 const functions={},bindings={},routes={},expressions=[];let controller;
 const endpoints=['search3','getArtist','getStarred','getAlbumList2'];
 for(const [name,source] of Object.entries(sources)){
  const file=ts.createSourceFile(name,source,ts.ScriptTarget.Latest,true);
  function capture(node,text){const start=file.getLineAndCharacterOfPosition(node.getStart(file));functions[`${name}:${start.line+1}:${start.character+1}`]=sha(text);}
  function visit(node){
   if(ts.isEnumDeclaration(node)&&node.name.text==='AlbumListSortType')bindings.AlbumListSortType=Object.fromEntries(node.members.map(m=>{if(!ts.isStringLiteral(m.initializer))throw new Error('unexpected enum');return[m.name.getText(file),m.initializer.text];}));
   if(ts.isVariableDeclaration(node)&&node.name.getText(file)==='MAX_SUBSONIC_ITEMS'){
    if(!ts.isNumericLiteral(node.initializer))throw new Error('unexpected constant');bindings.MAX_SUBSONIC_ITEMS=Number(node.initializer.text);
   }
   if(ts.isFunctionDeclaration(node)&&node.name.text==='getLibraryId'){const text=node.getText(file);capture(node,text);expressions.push(text);}
   if(ts.isPropertyAssignment(node)&&node.name.getText(file)==='getAlbumListCount'&&ts.isArrowFunction(node.initializer)){
    controller=node.initializer.getText(file);capture(node.initializer,controller);
   }
   if(ts.isPropertyAssignment(node)&&endpoints.includes(node.name.getText(file))&&ts.isObjectLiteralExpression(node.initializer)){
    const p=Object.fromEntries(node.initializer.properties.filter(ts.isPropertyAssignment).map(p=>[p.name.getText(file),p.initializer]));
    if(!p.path||!p.method||!ts.isStringLiteral(p.path)||!ts.isStringLiteral(p.method))throw new Error('unexpected route');routes[node.name.getText(file)]={path:p.path.text,method:p.method.text};
   }
   ts.forEachChild(node,visit);
  }
  visit(file);
 }
 if(Object.keys(functions).length!==2||Object.keys(bindings).length!==2||Object.keys(routes).length!==4)throw new Error('missing expressions');
 if(!write){assert.deepEqual(functions,fixture.functions);assert.deepEqual(bindings,fixture.bindings);assert.deepEqual(routes,fixture.routes);}
 const code=ts.transpileModule(expressions.join('\n')+'\nmodule.exports=('+controller+');',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 const cases=[];
 for(const input of fixture.cases){
  const module={exports:{}};const requests=[];let index=0;
  vm.runInNewContext(code,{module,...bindings,dayjs(){return{year(){return input.current_year;}};},ssApiClient(){return Object.fromEntries(endpoints.map(endpoint=>[endpoint,async args=>{
   if(index>=input.responses.length)throw new RangeError('fixture exhausted');
   const r=input.responses[index++];requests.push(JSON.parse(JSON.stringify({endpoint,args})));
   return{status:r.status??200,body:r.body,headers:{get(name){return name==='x-total-count'?(r.header??null):null;}}};
  }]));}},{timeout:1000});
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
 else replay(source,compiler,mode==='--write').then(n=>console.log(`Pinned album counts: ${n} synthetic cases passed; no network.`),()=>{console.error('Pinned album count replay failed; verify trusted inputs/hashes/fixture.');process.exitCode=1;});}
module.exports={replay};
