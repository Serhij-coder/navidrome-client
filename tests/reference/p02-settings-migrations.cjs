// Optional, controlled replay of the hash-pinned original migration method.
// No original module initialization, filesystem adapter or lifecycle hook runs.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const repo = path.resolve(__dirname, '../..');
const fixturePath = path.join(repo, 'tests/fixtures/p02/settings-migrations/source-replay.json');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function replay(referenceRoot, compilerPath, lodashPath, early = false) {
  const audit = JSON.parse(fs.readFileSync(path.join(repo, 'docs/app/baseline/source-audit.json')));
  const sources = {};
  function read(relative) {
    const bytes = fs.readFileSync(path.join(referenceRoot, relative));
    if (sha(bytes) !== audit.sources[relative]) throw new Error('Pinned source mismatch: ' + relative);
    sources[relative] = sha(bytes);
    return bytes.toString('utf8');
  }
  // Verify every original input before loading a compiler or evaluating source.
  const paths = ['src/renderer/store/settings.store.ts', 'src/shared/types/types.ts', 'src/shared/types/domain-types.ts', 'src/renderer/router/routes.ts', 'src/renderer/store/utils.ts'];
  const text = Object.fromEntries(paths.map(p => [p, read(p)]));
  const ts = require(path.resolve(compilerPath));
  const lodash = require(path.resolve(lodashPath));
  if (ts.version !== '6.0.3' || lodash.VERSION !== '4.17.21') throw new Error('Pinned TypeScript6.0.3 and Lodash4.17.21 required');
  const parsed = Object.fromEntries(paths.map(p => {
    const file = ts.createSourceFile(p, text[p], ts.ScriptTarget.Latest, true);
    if (file.parseDiagnostics.length) throw new Error('Source parse diagnostics');
    return [p, file];
  }));
  function find(p, predicate) {
    const found = [];
    function visit(node) { if (predicate(node)) found.push(node); ts.forEachChild(node, visit); }
    visit(parsed[p]);
    if (found.length !== 1) throw new Error('Ambiguous source anchor');
    return found[0];
  }
  const settings = paths[0], types = paths[1], domain = paths[2], routes = paths[3], utils = paths[4];
  const method = find(settings, n => ts.isMethodDeclaration(n) && n.name?.getText(parsed[settings]) === 'migrate');
  const methodText = method.getText(parsed[settings]);
  if (sha(methodText) !== '4113f0fd417709d00fbd141e4a4595845dbde7eb2be9c2560b86796958b293ac') throw new Error('Migration inventory hash mismatch');
  const context = vm.createContext({ mergeWith: lodash.mergeWith });
  function evaluate(source) {
    const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText;
    vm.runInContext(js, context, { timeout: 2000 });
  }
  for (const [p, names] of [[domain, ['LibraryItem']], [types, ['ItemListKey', 'TableColumn', 'Platform']], [routes, ['AppRoute']], [settings, ['HomeItem', 'ArtistItem']]]) {
    for (const name of names) {
      const node = find(p, n => ts.isEnumDeclaration(n) && n.name.text === name);
      evaluate(node.getText(parsed[p]).replace(/^export\s+/, ''));
    }
  }
  function initializer(p, name) {
    return find(p, n => ts.isVariableDeclaration(n) && n.name.getText(parsed[p]) === name).initializer;
  }
  evaluate('const AUTO_DJ_STRATEGY = ' + initializer(settings, 'AUTO_DJ_STRATEGY').getText(parsed[settings]) + ';');
  const initial = initializer(settings, 'initialState');
  const auto = initial.properties.find(n => n.name?.getText(parsed[settings]) === 'autoDJ').initializer.getText(parsed[settings]);
  const general = initial.properties.find(n => n.name?.getText(parsed[settings]) === 'general').initializer;
  function property(section, name) {
    const object = initial.properties.find(n => n.name?.getText(parsed[settings]) === section).initializer;
    return object.properties.find(n => n.name?.getText(parsed[settings]) === name).initializer.getText(parsed[settings]);
  }
  const layout = property('general','sideQueueLayout');
  evaluate(`const initialState = {autoDJ:${auto}, general:{sideQueueLayout:${layout}, language:${property('general','language')}, followSystemTheme:${property('general','followSystemTheme')}, artistBackground:${property('general','artistBackground')}, artistBackgroundBlur:${property('general','artistBackgroundBlur')}}, window:{releaseChannel:${property('window','releaseChannel')}}, playback:{mediaSession:${property('playback','mediaSession')}}}; const i18n={t:()=>"fixture-label"}; const nanoid=()=>"fixture-preset-id";`);
  evaluate(`globalThis.migrate = ({${methodText}}).migrate;`);
  const merge = initializer(utils, 'mergeOverridingColumns').getText(parsed[utils]);
  evaluate(`globalThis.mergeDefaults = ${merge}; globalThis.defaults = initialState.autoDJ;`);
  const column = id => ({id, align:'end', autoSize:false, isEnabled:true, pinned:'right', width:17});
  const input = ['titleCombined', 'artists', 'releaseYear', 'releaseDate'].map(column);
  function state(list, view, columns, autoDJ) {
    return { lists:{[list]:{table:{columns:view==='table'?columns:[]},detail:{columns:view==='detail'?columns:[]}}}, general:{sidebarItems:[],homeItems:[],artistItems:[]}, autoDJ,
      window:{}, playback:{}, lyrics:{}, lyricsDisplay:{}, visualizer:{}, tagEditor:{} };
  }
  function migrate(value, version) {
    context.fixture = JSON.parse(JSON.stringify(value)); context.version = version;
    vm.runInContext('globalThis.result = migrate(fixture,version)', context, {timeout:2000});
    return JSON.parse(JSON.stringify(context.result));
  }
  if (early) {
    evaluate('globalThis.retainedDefaults={general:{language:initialState.general.language,followSystemTheme:initialState.general.followSystemTheme},autoDJ:initialState.autoDJ,lists:{}};');
    const defaults = JSON.parse(JSON.stringify(context.retainedDefaults));
    const cases=[];
    for(let version=0;version<=13;version++) for(const variant of ['valid','aliases','sparse','missing']) {
      const autoDJ=variant==='sparse'?{enabled:true}:{...context.defaults,mode:variant==='aliases'?'legacy':'albums',albumStrategy:variant==='aliases'?'default':'library_random',songStrategy:variant==='aliases'?'similar_forward':'library_random',itemCount:7.5,timing:1.25};
      const incoming=state('song','table',input,autoDJ);
      incoming.general.language='uk'; incoming.general.followSystemTheme=true;
      incoming.general.sidebarItems=[{id:'Folders',disabled:false,label:'fixture-folder',route:'fixture-route'}];
      if(variant==='missing')delete incoming.autoDJ;
      const output=migrate(incoming,version);
      context.output=output;
      const merged=vm.runInContext('mergeDefaults(output,JSON.parse(JSON.stringify(retainedDefaults)))',context,{timeout:2000});
      const retained=JSON.parse(JSON.stringify(merged));
      cases.push({version,variant,input:incoming,returnedEmpty:Object.keys(output).length===0,columns:retained.lists?.song?.table?.columns??null,general:{language:retained.general.language,followSystemTheme:retained.general.followSystemTheme},autoDJ:retained.autoDJ,sidebarIds:output.general?.sidebarItems?.map(x=>x.id)??[],homeIds:output.general?.homeItems?.map(x=>x.id)??[],windowBarStyle:output.window?.windowBarStyle??null});
    }
    const invalidResetCases=[];
    for(const [name,version,value] of [['missing-general',10,{}],['missing-sidebar',10,{general:{}}],['null-sidebar',10,{general:{sidebarItems:null}}],['null-general',11,{general:null}],['null-state',11,null]]) {
      try { const output=migrate(value,version); invalidResetCases.push({name,version,input:value,returnedEmpty:Object.keys(output).length===0,error:null}); }
      catch(error){ invalidResetCases.push({name,version,input:value,returnedEmpty:false,error:error.name}); }
    }
    return {schema:1,baseline:'7d6ad318c79802e226996cfc39acba6b05bb22e8',methodId:'src/renderer/store/settings.store.ts:2442:13',methodSha256:sha(methodText),sources,compiler:ts.version,lodash:lodash.VERSION,boundaries:['Exact original migration and merge helper with selected exact defaults','Synthetic structural fields and fixed translation/preset identifiers','Reset defaults cover only retained preferences/AutoDJ; empty lists means removed imported overrides, not source UI column defaults'],defaults,cases,invalidResetCases};
  }
  const cases = [], shapes = {};
  const targets = [['song','table'],['album','table'],['playlistSong','table'],['queueSong','table'],['albumDetail','table'],['fullScreen','table'],['sideQueue','table'],['folder','table'],['albumArtistSong','table'],['genreSong','table'],['artist','table'],['album','detail'],['song','detail']];
  for (let version=14;version<=34;version++) for (const [list,view] of targets) {
    const output = migrate(state(list,view,input,{...context.defaults}),version).lists[list][view].columns;
    const shape = sha(JSON.stringify(output)).slice(0,16);
    shapes[shape] = output;
    cases.push({version,list,view,shape});
  }
  const autoCases = [];
  for (const version of [14,15,20,21,22,26,27,28,29,33,34]) for (const mode of ['albums','legacy']) for (const strategy of ['library_random','similar','default','similar_forward','unknown']) {
    const incoming = {...context.defaults, mode, songStrategy:strategy, albumStrategy:strategy, itemCount:7.5,timing:1.25};
    const result = migrate(state('artist','table',[],incoming), version).autoDJ;
    autoCases.push({version,input:incoming,output:result});
  }
  const sparseInput = { songStrategy:'default', mode:'legacy' };
  const sparse = migrate(state('artist','table',[],sparseInput),27).autoDJ;
  context.sparse = sparse;
  const completed = vm.runInContext('mergeDefaults(sparse,JSON.parse(JSON.stringify(defaults)))',context,{timeout:2000});
  const duplicateInput = [...input,column('bitDepth')];
  const duplicated = migrate(state('song','table',duplicateInput,{...context.defaults}),14).lists.song.table.columns;
  const keyCases = ['fullScreen','fullscreen','sideQueue','sidequeue'].map(list => ({ list, output:migrate(state(list,'table',input,{...context.defaults}),14).lists[list].table.columns }));
  return {schema:1,baseline:audit.commit || '7d6ad318c79802e226996cfc39acba6b05bb22e8',methodId:'src/renderer/store/settings.store.ts:2442:13',methodSha256:sha(methodText),sources,compiler:ts.version,lodash:lodash.VERSION,
    boundaries:['Exact original migration method and mergeOverridingColumns; selected exact enums/defaults only','Synthetic valid structural fields for unrelated migration branches','Fixed translation and preset-ID functions; no UI/runtime/storage initialization'],
    input,cases,shapes,autoCases,sparseAutoDj:{input:sparseInput,migrated:sparse,merged:JSON.parse(JSON.stringify(completed))},observations:{duplicateInput,duplicated,keyCases}};
}
module.exports={replay};
if(require.main===module){
  const [root,compiler,lodash,...flags] = process.argv.slice(2);
  if(!root||!compiler||!lodash||flags.some(x=>!['--early','--write'].includes(x))) throw new Error('Usage: node tests/reference/p02-settings-migrations.cjs REFERENCE_ROOT TYPESCRIPT_JS LODASH_JS [--early] [--write]');
  const early=flags.includes('--early');
  const result=replay(root,compiler,lodash,early);
  const target=early?path.join(repo,'tests/fixtures/p02/settings-migrations/early-source-replay.json'):fixturePath;
  if(flags.includes('--write')) fs.writeFileSync(target,JSON.stringify(result,null,2)+'\n');
  else assert.deepEqual(result,JSON.parse(fs.readFileSync(target)));
  console.log(early?`${result.cases.length} early/reset source cases and ${result.invalidResetCases.length} malformed observations matched`:`${result.cases.length} column and ${result.autoCases.length} Auto DJ source replays matched; sparse defaults/duplicate/key-spelling observations recorded`);
}
