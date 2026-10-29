# Retained historical settings transforms (P02.05)

Copied combined, exported and split settings now accept integer versions 14 through 34. The [import projector](imports.md) applies each selected category's own source version before replacing target values. Authentication, queue and timestamp version matrices are unchanged. This extends retained fields, not the entire settings store or P02 phase.

## Implemented transforms

| Source condition | Retained transformation |
| --- | --- |
| Version <=14 | Append missing `bitDepth` and `sampleRate` disabled columns on the supported matching song/playlist-song/queue-song/album-detail table keys |
| Version <=20 | Insert disabled `titleArtist` after `titleCombined`, or append if the anchor is absent, on the source-listed tables |
| Version <=21 | Insert disabled `composer` after `artists`, or append if the anchor is absent, on the source-listed tables |
| Version <=26 | Append disabled `albumGroup`, width 240 and left-pinned, on the source-listed song/folder/queue tables |
| Version <29 | Insert disabled `year` before `releaseYear` and `date` before `releaseDate`, or append without an anchor, on the source-listed tables and album detail view |
| Version <28 | Normalize a supplied legacy Auto DJ object: `default`/`similar_forward`/unknown strategies to `similar`, retain `library_random`, invalid mode to `songs`, and fill missing known fields from the pinned initial defaults |

Column insertions preserve existing order and user properties. Existing IDs are kept once; corrupt/duplicate incoming arrays are rejected before migration, retaining the target baseline. The migration operates on a copy and fails without partial mutation. Arrays then replace the complete corresponding target array. A v34 empty array remains empty; a historical empty array receives the applicable disabled schema additions. Invalid widths, bounds and vocabulary retain the existing validation policy. Version 28 and newer Auto DJ objects remain strict; aliases and missing required fields are rejected as a category. Explicit wrong/null booleans or numbers are not defaulted.

A wholly absent Auto DJ category is not synthesized over existing target state. For older copies this is reported as `LegacyDefaultsDeferred`, because the importer is a partial overlay rather than full source hydration. Missing list categories/views likewise leave the baseline untouched. Other source transforms (sidebar/home items, lyrics/display, filters, presets, headers, tags and window fields) remain deferred and continue to produce partial/deferred diagnostics. Versions 0-13 remain unsupported because their early-return/reset paths need a separate compatibility decision and fixtures; future versions fail closed.

## Source observations and deliberate differences

The exact pinned migration at `settings.store.ts:2442-2939` uses lowercase `fullscreen`/`sidequeue` only in the <=14 bit-depth/sample-rate branch. Its later branches and current ItemListKey values use `fullScreen`/`sideQueue`. Replays confirm that those two disabled columns are not added to the current camelCase keys in that old branch. The target preserves that distinction. Lowercase keys remain unsupported with a diagnostic, not silently renamed. No runtime defect in the actual historical UI is claimed.

For a synthetic v14 song array already containing `bitDepth`, the original method appends a second occurrence. The target keeps the existing ID and its user properties, preserving the duplicate-free durable schema and making reimport idempotent. The source result and explicit target difference have a regression fixture. This is a validation/normalization decision under [ADR-004](../adr/004-persistence.md) and [ADR-009](../adr/009-source-differences.md), not a claim that current upstream persistence loses data.

## Verification and reproduction

[Source replay fixtures](../../../tests/fixtures/p02/settings-migrations/) contain 273 original column outputs covering every accepted version and thirteen list/view combinations, 110 Auto DJ outputs, the original flat-category default merge and the duplicate/key-spelling observations. Rust tests compare retained output values/properties directly to these captured outputs. Invalid current-source Auto DJ cases are rejected by the target's closed schema. Helpers also test thresholds, application lists, idempotence, corruption and unsupported versions. Combined/split/mixed copy tests verify category precedence, absent-category preservation and transactional apply/rollback of the exact normalized preview.

The optional [replay script](../../../tests/reference/p02-settings-migrations.cjs) verifies all original source-file hashes and the stable method hash before loading external tools. It transpiles only the exact migration, original merge helper, selected enums and initial defaults into an isolated VM. It provides synthetic structural fields for unrelated branches and fixed translation/preset-ID functions. No original module initialization, storage/UI adapter, installer or package lifecycle runs. Source execution covers these controlled fixtures, not a complete historical application journey.

With the local pinned archive extracted and externally installed TypeScript 6.0.3/Lodash 4.17.21:

```sh
node tests/reference/p02-settings-migrations.cjs /absolute/reference /absolute/typescript/lib/typescript.js /absolute/lodash/lodash.js
cargo test -p feishin-storage --test settings_migration --test settings_versions --locked
```

The default build and CI use only tracked fixtures; the source archive and external JS tools are not build/test dependencies. The default tooling regression verifies that modified source is rejected before compiler loading or fixture writes. The optional script's explicit `--write` mode regenerates this task's fixture after successful source verification.

See the [migration map](../migration/p02-settings-transforms.csv) and [verification record](settings-history-verification.md). Full settings categories, older real-user export evidence, production state-owner/shell adoption, credential policy (P02.06), cache lifecycle (P02.07) and P00/P01/P02 exits remain open.
