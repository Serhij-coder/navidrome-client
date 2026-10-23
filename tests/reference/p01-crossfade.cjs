// Offline replay of pinned Feishin gain functions; no UI/audio/server execution.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const FILE = 'src/renderer/features/player/audio-player/web-player.tsx';
const SHA = 'fcb4863a9efae3a2bd858cd491d5ae82722256c1c298916b5ddf36b92706340d';
const EXTRA = {
  'src/renderer/features/player/audio-player/engine/web-player-engine.tsx':'09a9ca7751ff791ce3c74a193b794bf4a3d17dc6cdf03d8002abdb002386bf77',
  'src/renderer/features/player/audio-player/utils/player-utils.ts':'42092ef332b3e3b3902d10ab60f3a2d408114eaaed5448e850d479f41038c889',
  'src/shared/types/types.ts':'6b52103372fca31dc540458766a318835c418a993a37d62db3a4ea554c57af82'
};
try {
  const [root, parser, output] = process.argv.slice(2);
  if (!root || !parser || !output || fs.existsSync(output)) throw new Error();
  const source = fs.readFileSync(path.join(root, FILE), 'utf8');
  if (crypto.createHash('sha256').update(source).digest('hex') !== SHA) throw new Error();
  const ts = require(path.resolve(parser));
  if (ts.version !== '6.0.3') throw new Error();
  const ast = ts.createSourceFile(FILE, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  if (ast.parseDiagnostics.length) throw new Error();
  const wanted = new Set(['crossfadeHandler', 'equalPowerEaseIn', 'equalPowerEaseOut', 'exponentialEaseIn', 'exponentialEaseOut', 'getCrossfadeEasing', 'linearEase', 'sCurveEase']);
  const functions = ast.statements.filter(node => ts.isFunctionDeclaration(node) && wanted.has(node.name?.text));
  if (functions.length !== wanted.size) throw new Error();
  const anchors = functions.map(node => {
    const position = ast.getLineAndCharacterOfPosition(node.getStart(ast));
    const body = node.getText(ast);
    return {id:`${FILE}:${position.line+1}:${position.character+1}`, name:node.name.text,
      sha256:crypto.createHash('sha256').update(body).digest('hex')};
  });
  const extras = Object.entries(EXTRA).map(([name,hash]) => {
    const text=fs.readFileSync(path.join(root,name),'utf8');
    if (crypto.createHash('sha256').update(text).digest('hex')!==hash) throw new Error();
    const parsed=ts.createSourceFile(name,text,ts.ScriptTarget.Latest,true,name.endsWith('.tsx')?ts.ScriptKind.TSX:ts.ScriptKind.TS);
    if (parsed.parseDiagnostics.length) throw new Error();
    return parsed;
  });
  const declarations = extras.flatMap(parsed=>parsed.statements.filter(ts.isVariableStatement)
    .flatMap(statement=>statement.declarationList.declarations.filter(node=>['normalizeVolume','convertToLogVolume'].includes(node.name.getText(parsed)))
      .map(node=>{const position=parsed.getLineAndCharacterOfPosition(node.initializer.getStart(parsed));
        anchors.push({id:`${parsed.fileName}:${position.line+1}:${position.character+1}`,name:node.name.text,
          sha256:crypto.createHash('sha256').update(node.initializer.getText(parsed)).digest('hex')});
        return `const ${node.getText(parsed)};`;})));
  if (declarations.length!==2) throw new Error();
  const enumeration=extras[2].statements.find(node=>ts.isEnumDeclaration(node)&&node.name.text==='CrossfadeStyle');
  if (!enumeration) throw new Error();
  const styles=Object.fromEntries(enumeration.members.map(member=>{
    if (!ts.isStringLiteral(member.initializer)) throw new Error();
    return [member.name.text,member.initializer.text];
  }));
  const context = vm.createContext({CrossfadeStyle:styles, PlayerStatus:{PLAYING:'playing'},
    usePlayerStoreBase:{getState:()=>({player:{status:'playing',pauseOnNextSongEnd:false}})}});
  const code = ts.transpileModule([...functions.map(node=>node.getText(ast)),...declarations].join('\n'),
    {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  vm.runInContext(code, context, {timeout:1000});
  const samples = [];
  for (const style of [...Object.values(styles), 'unknown-saved-style']) {
    for (const progress of [-1, 0, .01, .1, .25, .5, .75, .9, .99, 1, 2]) {
     for (const master of [100, 50]) {
      context.input = {style, progress, master};
      const result = vm.runInContext(`(() => {
        const result = {};
        crossfadeHandler({crossfadeDuration:5,crossfadeStyle:input.style,
          currentPlayer:{ref:null,setVolume:value=>{result.outgoing=value/input.master;result.web_outgoing_gain=convertToLogVolume(normalizeVolume(value));}},currentPlayerNum:1,
          currentTime:95+5*input.progress,duration:100,hasNextSong:true,isTransitioning:'player1',
          nextPlayer:{ref:null,setVolume:value=>{result.incoming=value/input.master;result.web_incoming_gain=convertToLogVolume(normalizeVolume(value));}},playerNum:1,
          setIsTransitioning:()=>{},volume:input.master});
        return result;
      })()`, context, {timeout:1000});
      samples.push({style,progress,master_fraction:master/100,...result});
     }
    }
  }
  fs.writeFileSync(output, JSON.stringify({baseline:'7d6ad318c79802e226996cfc39acba6b05bb22e8',file:FILE,
    file_sha256:SHA,extra_sources:EXTRA,parser:ts.version,functions:anchors,samples,
    scope:'Exact pinned active web-player slider coefficient and assigned media gain replay with mocked PLAYING/transition state; not browser audio or full transition lifecycle'}, null, 2)+'\n', {flag:'wx'});
} catch {
  console.error('Crossfade reference replay failed: inputs, source/parser provenance or evaluation rejected. No paths or payloads logged.');
  process.exitCode = 1;
}
