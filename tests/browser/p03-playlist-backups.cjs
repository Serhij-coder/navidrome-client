// Owned test-only WASM storage bundle; no original profile or remote mutations.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium,firefox,webkit}=require(process.env.P02_PLAYWRIGHT||'playwright');
const bundle=path.resolve(process.argv[2]),key='feishin-rust:dev:p03:playlist-backups:v1';
const server=http.createServer((req,res)=>{
 if(req.url==='/'){res.end('<!doctype html><title>Owned playlist backup storage</title>');return;}
 const files={'/probe.js':['probe.js','text/javascript'],'/probe_bg.wasm':['probe_bg.wasm','application/wasm']};const entry=files[req.url];if(!entry){res.writeHead(404).end();return;}res.setHeader('Content-Type',entry[1]);res.end(fs.readFileSync(path.join(bundle,entry[0])));
});
async function connect(page,origin){await page.goto(origin);await page.evaluate(async()=>{const m=await import('/probe.js');await m.default();window.backupProbe=m.playlist_backup_probe;window.backupPort=m.playlist_backup_port_probe;});}
(async()=>{
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const origin=`http://127.0.0.1:${server.address().port}`;
 try{for(const[name,engine]of Object.entries({chromium,firefox,webkit})){
  const browser=await engine.launch({headless:true,timeout:30000});try{
   const context=await browser.newContext(),page=await context.newPage();await connect(page,origin);assert.equal(await page.evaluate(()=>window.backupProbe('defaults')),true);
   await page.evaluate(()=>{localStorage.setItem('playlist-backup-p','original unscoped fixture');localStorage.setItem('feishin-rust:dev:p02:profiles:v1','independent profile fixture');});
   assert.equal(await page.evaluate(()=>window.backupProbe('save-scopes')),true);await connect(page,origin);assert.equal(await page.evaluate(()=>window.backupProbe('reload')),true);
   assert.equal(await page.evaluate(()=>localStorage.getItem('playlist-backup-p')),'original unscoped fixture');assert.equal(await page.evaluate(()=>localStorage.getItem('feishin-rust:dev:p02:profiles:v1')),'independent profile fixture');
   const other=await browser.newContext(),fresh=await other.newPage();await connect(fresh,origin);assert.equal(await fresh.evaluate(()=>window.backupProbe('defaults')),true);await other.close();
   assert.equal(await page.evaluate(key=>{
    const before=localStorage.getItem(key),original=Storage.prototype.setItem;let rejected=false;Storage.prototype.setItem=()=>{throw new DOMException('private-marker','QuotaExceededError');};
    try{window.backupProbe('try-save');}catch(e){rejected=!String(e).includes('private-marker');}finally{Storage.prototype.setItem=original;}return rejected&&localStorage.getItem(key)===before;
   },key),true);
   assert.equal(await page.evaluate(()=>{const original=Storage.prototype.getItem;let rejected=false;Storage.prototype.getItem=()=>{throw new DOMException('private-marker','SecurityError');};try{window.backupProbe('defaults');}catch(e){rejected=!String(e).includes('private-marker');}finally{Storage.prototype.getItem=original;}return rejected;}),true);
   const valid=await page.evaluate(key=>localStorage.getItem(key),key);
   for(const raw of ['{"schema_version":1,"revision":7,"backups":[{"serverId":"a","playlistId":"p","name":"Saved","songIds":["same","same"],"capturedAtUnixMs":1}],"backups":[]}',' '.repeat(256*1024+1),'{"schema_version":2,"new_field":"keep"}','{malformed','{"schema_version":1,"revision":0,"backups":[{"serverId":"a","playlistId":"p","name":"N","songIds":[""],"capturedAtUnixMs":1}]}']){
    assert.equal(await page.evaluate(({key,raw})=>{localStorage.setItem(key,raw);let rejected=false;try{window.backupProbe('try-save');}catch{rejected=true;}return rejected&&localStorage.getItem(key)===raw;},{key,raw}),true);
   }
   await page.evaluate(({key,valid})=>localStorage.setItem(key,valid),{key,valid});assert.equal(await page.evaluate(()=>window.backupProbe('remove-a')),true);
   await connect(page,origin);assert.equal(await page.evaluate(key=>JSON.parse(localStorage.getItem(key)).backups.length,key),1);
   for(const action of ['cancelled','expired','small']) {
    const before=await page.evaluate(key=>localStorage.getItem(key),key);
    assert.equal(await page.evaluate(action=>window.backupPort(action),action),true);
    assert.equal(await page.evaluate(key=>localStorage.getItem(key),key),before);
   }
   assert.equal(await page.evaluate(()=>window.backupPort('save')),true);
   await connect(page,origin);
   assert.equal(await page.evaluate(key=>JSON.parse(localStorage.getItem(key)).backups.find(b=>b.serverId==='boundary').songIds.join(','),key),'same,same');
   assert.equal(await page.evaluate(()=>{const original=Storage.prototype.setItem;let rejected=false;Storage.prototype.setItem=()=>{throw new DOMException('private-marker','QuotaExceededError');};try{window.backupPort('save');}catch(e){rejected=!String(e).includes('private-marker');}finally{Storage.prototype.setItem=original;}return rejected;}),true);
   await context.close();process.stdout.write(`${name}: isolated scoped backups, reload, bounds/protection and injected storage failures passed\n`);
  }finally{await browser.close();}
 }}finally{await new Promise(resolve=>server.close(resolve));}
})().catch(()=>{process.stderr.write('Owned browser playlist backup probe failed.\n');process.exitCode=1;});
