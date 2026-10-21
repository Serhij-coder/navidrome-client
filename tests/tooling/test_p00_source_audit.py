"""Optional parser regression checks; no ignored source/inventory input required."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
PARSER = os.environ.get("P00_TYPESCRIPT")


@unittest.skipUnless(PARSER, "set P00_TYPESCRIPT to TypeScript 6.0.3 for reference parser regression")
class SourceAudit(unittest.TestCase):
    def fixture(self, directory):
        files = {
            "src/renderer/main.tsx": "import './app';",
            "src/renderer/app.tsx": "import './features/player/components/audio-players';",
            "src/remote/index.tsx": "import './router';",
            "src/main/index.ts": "export {};",
            "src/preload/index.ts": "export {};",
            "src/renderer/features/player/audio-players.tsx": "",
            "src/renderer/features/player/components/audio-players.tsx": "export const player = 1;",
            "src/renderer/router/routes.ts": "export enum AppRoute {HOME='/', CHILD='/child', UNUSED='/unused'}",
            "src/renderer/router/app-router.tsx": "const view = <><Route path={AppRoute.HOME}/><Route path={\n AppRoute.CHILD\n } element={<Child/>}/></>;",
            "src/remote/router.tsx": "const view = <><Route index/><Route path='queue'/></>;",
            "src/shared/themes/test.ts": "import './test.css?inline';",
            "src/shared/themes/test.css": "body {}",
            "src/renderer/features/player/hooks/use-scrobble.ts": "const getPositionValue = (s:number,t:boolean) => t?s*1e7:s*1000; const checkScrobbleConditions=(x:any)=>true;",
            "src/renderer/store/player.store.ts": "function calculateNextIndex(i:number,n:number,r:string){return {nextIndex:i,shouldStop:false};}",
            "src/renderer/store/utils.ts": "function cleanQueueForPersistence(q:any){return q;} const splitSettingsStorage={setItem(n:string,v:string){},getItem(n:string){return null;}};",
            "web.vite.config.ts": "export {};",
            "remote.vite.config.ts": "export {};",
            "electron.vite.config.ts": "export {};",
            "package.json": "{}",
            "pnpm-lock.yaml": "lockfileVersion: '9.0'",
        }
        for name, text in files.items():
            file = Path(directory) / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text(text)

    def run_audit(self, directory):
        return subprocess.run(["node", str(ROOT / "tools/app/p00-source-audit.cjs"), directory, PARSER], capture_output=True, text=True, timeout=30)

    def test_multiline_routes_resource_queries_and_empty_duplicate(self):
        with tempfile.TemporaryDirectory() as directory:
            self.fixture(directory)
            result = self.run_audit(directory)
            self.assertEqual(0, result.returncode, result.stderr)
            report = json.loads(result.stdout)
            self.assertEqual(["registered", "registered", "unregistered-declaration"], [r["status"] for r in report["routes"]])
            self.assertEqual([], report["graph"]["unresolved"])
            empty, active = report["compositions"]
            self.assertEqual([], empty["incoming"])
            self.assertEqual(0, empty["bytes"])
            self.assertEqual(["src/renderer/main.tsx"], active["roots"])
            # IDs must point at function expressions, not the variable declaration.
            self.assertEqual("src/renderer/features/player/hooks/use-scrobble.ts:1:26", report["characterization"]["position"]["source_id"])

    def test_wrong_parser_version_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            self.fixture(directory)
            fake = Path(directory) / "parser.cjs"
            fake.write_text("module.exports={version:'0.0.0'}")
            result = subprocess.run(["node", str(ROOT / "tools/app/p00-source-audit.cjs"), directory, str(fake)], capture_output=True, text=True, timeout=30)
            self.assertNotEqual(0, result.returncode)
            self.assertIn("baseline parser", result.stderr)

    def test_parse_errors_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            self.fixture(directory)
            (Path(directory) / "src/renderer/router/app-router.tsx").write_text("const broken = <Route path={;")
            result = self.run_audit(directory)
            self.assertNotEqual(0, result.returncode)
            self.assertIn("Parse diagnostics", result.stderr)


if __name__ == "__main__":
    unittest.main()
