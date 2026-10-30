#!/usr/bin/env node
'use strict';
// Only selected original backup declarations/write run over normalized inputs.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),vm=require('node:vm'),assert=require('node:assert/strict');
const fixturePath=path.resolve(__dirname,'../../tests/fixtures/p03/subsonic-playlist-backup-source.json');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
function checked(file,hash){const b=fs.readFileSync(file);if(sha(b)!==hash)throw new Error('hash mismatch');return b.toString('utf8');}
async function replay(sourceRoot,compiler,write=false){
 const fixture=JSON.parse(fs.readFileSync(fixturePath,'utf8')),functions={},expressions={},selected={};
 const sources=Object.fromEntries(Object.entries(fixture.sources).map(([n,h])=>[n,checked(path.join(sourceRoot,n),h)]));checked(compiler,fixture.typescript_sha256);const ts=require(path.resolve(compiler));
 for(const[name,source]of Object.entries(sources)){
  const file=ts.createSourceFile(name,source,ts.ScriptTarget.Latest,true);
  function visit(node){
   if(ts.isPropertyAssignment(node)&&node.name.getText(file)==='replacePlaylist'&&ts.isArrowFunction(node.initializer)){
    const text=node.initializer.getText(file),start=file.getLineAndCharacterOfPosition(node.initializer.getStart(file));functions[`${name}:${start.line+1}:${start.character+1}`]=sha(text);
    function inner(n){
     if(ts.isVariableDeclaration(n)&&['backup','backupKey'].includes(n.name.getText(file))){selected[n.name.getText(file)]=n.initializer.getText(file);expressions[n.name.getText(file)]=sha(selected[n.name.getText(file)]);}
     if(ts.isAwaitExpression(n)&&ts.isCallExpression(n.expression)&&n.expression.expression.getText(file)==='set'){selected.write=n.getText(file);expressions.write=sha(selected.write);}
     ts.forEachChild(n,inner);
    }inner(node.initializer);
   }ts.forEachChild(node,visit);
  }visit(file);
 }
 if(Object.keys(functions).length!==1||Object.keys(selected).length!==3)throw new Error('missing expressions');
 if(!write){assert.deepEqual(functions,fixture.functions);assert.deepEqual(expressions,fixture.expressions);}
 const code=ts.transpileModule('module.exports=async()=>{const backup=('+selected.backup+');const backupKey=('+selected.backupKey+');'+selected.write+';};',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 const cases=[];
 for(const input of fixture.cases){const module={exports:{}},writes=[];vm.runInNewContext(code,{module,query:{id:input.playlist_id},playlist:{name:input.name},existingSongs:input.song_ids.map(id=>({id})),Date:{now:()=>input.captured_at_unix_ms},set:async(key,value)=>{writes.push(JSON.parse(JSON.stringify({key,value})));if(input.reject_write)throw new Error('synthetic storage rejection');}},{timeout:1000});
  const result={...input};delete result.expected;delete result.expected_error;try{await module.exports();}catch(e){if(e.name!=='Error')throw e;result.expected_error=e.name;}result.expected=writes;cases.push(result);
 }
 const actual={...fixture,functions,expressions,cases};if(write)fs.writeFileSync(fixturePath,JSON.stringify(actual,null,2)+'\n');else assert.deepEqual(actual,fixture);return cases.length;
}
if(require.main===module){const[source,compiler,mode]=process.argv.slice(2);if(!source||!compiler||(mode&&mode!=='--write'))process.exitCode=1;
 else replay(source,compiler,mode==='--write').then(n=>console.log(`Pinned backup statements: ${n} normalized-input cases passed; no network.`),()=>{console.error('Pinned backup replay failed; verify trusted inputs/hashes/fixture.');process.exitCode=1;});}
module.exports={replay};
