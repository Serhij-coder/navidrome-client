import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("p00_library", ROOT / "tools/app/p00-library.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class LibraryFixtures(unittest.TestCase):
    def items(self, backend, response):
        if backend == "jellyfin":
            return response["Items"]
        if backend == "subsonic":
            return response["subsonic-response"]["searchResult3"]["song"]
        return response

    def test_large_library_page_boundaries_and_identity(self):
        for backend in ["jellyfin", "navidrome", "subsonic"]:
            with self.subTest(backend=backend):
                first = self.items(backend, MODULE.page(backend, 100_000, 0, 500))
                last = self.items(backend, MODULE.page(backend, 100_000, 99_999, 500))
                empty = self.items(backend, MODULE.page(backend, 100_000, 100_000, 500))
                key = "Id" if backend == "jellyfin" else "id"
                self.assertEqual(500, len(first))
                self.assertEqual(500, len({item[key] for item in first}))
                self.assertEqual("song-099999", last[0][key])
                self.assertEqual(1, len(last))
                self.assertEqual([], empty)
                self.assertEqual([], self.items(backend, MODULE.page(backend, 0, 0, 10)))

    def test_units_same_ids_and_determinism(self):
        jf = MODULE.page("jellyfin", 10, 0, 10)["Items"][0]
        nd = MODULE.page("navidrome", 10, 0, 10)[0]
        ss = self.items("subsonic", MODULE.page("subsonic", 10, 0, 10))[0]
        self.assertEqual(jf["Id"], nd["id"])
        self.assertEqual(ss["id"], nd["id"])
        self.assertEqual(180_000, jf["RunTimeTicks"] / 10_000)
        self.assertEqual(180_000, nd["duration"] * 1000)
        self.assertEqual(180_000, ss["duration"] * 1000)
        self.assertIn("日本語", jf["Name"])
        self.assertEqual(MODULE.page("navidrome", 10, 0, 10), MODULE.page("navidrome", 10, 0, 10))

    def test_reject_unbounded_or_invalid_requests(self):
        for backend, count, offset, limit in [("other", 10, 0, 1), ("jellyfin", -1, 0, 1),
                                             ("navidrome", 100_001, 0, 1), ("subsonic", 10, -1, 1),
                                             ("jellyfin", 10, 0, 501), ("subsonic", 10, 0, 0)]:
            with self.subTest(args=(backend, count, offset, limit)), self.assertRaises(ValueError):
                MODULE.page(backend, count, offset, limit)


if __name__ == "__main__":
    unittest.main()
