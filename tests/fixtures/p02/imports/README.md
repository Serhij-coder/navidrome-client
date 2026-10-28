# P02 copied import regression inputs

These small fixtures are synthetic/source-derived from Feishin `7d6ad318c79802e226996cfc39acba6b05bb22e8`, especially `src/renderer/store/{utils,settings.store,player.store,auth.store,timestamp.store}.ts` and `src/shared/types/{types,domain-types}.ts`. They are not historical real-user exports and contain no actual credentials, authenticated media URLs or live profile data. Marked secret strings are regression canaries that must not survive projection.

- `settings-v14.json`: historical combined envelope and partial current-schema Auto DJ values, exercising explicit deferred migrations.
- `general-split.json`, `autodj-split.json`: copied category objects, including fractional source-schema numbers.
- `columns-corrupt.json`: duplicate column IDs, wrong types, negative width and a valid empty detail array to verify independent recovery.
- `player-v3.json`: embedded queue with two occurrences of one song, orphan data, active track permutation and metadata to omit.
- `player-v4.json`, `queue-v4.json`: current split shapes, preserving separate player/queue input boundaries.
- `auth-v2.json`: copied authentication shape with synthetic credential canaries and non-secret LAN profile projection.

Tests additionally read the existing captured v34 export and source-characterized synthetic state traces directly. See [the supported matrix and limitations](../../../../docs/app/p02/imports.md).
