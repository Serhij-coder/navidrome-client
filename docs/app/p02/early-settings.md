# Early settings and retained reset handling (P02.05)

Copied settings with explicit integer versions 0-34 are now accepted for the retained schema. The [source migration](settings-history.md) has three control-flow paths; the projector preserves that ordering rather than applying later transforms to every older version.

| Version | Observed source path | Target retained behavior |
| --- | --- | --- |
| 0-9 | Returns before later column/Auto DJ normalization; v8 also changes sidebar items | Valid columns stay unchanged. Supplied Auto DJ objects receive missing-field defaults from persistence hydration, but existing aliases/invalid enums are rejected rather than normalized. Partial/early-return diagnostics remain explicit |
| 10-11 | Returns an empty settings object before later transforms | The preview resets retained preferences, removes imported column overrides and supplies initial Auto DJ defaults, with `SettingsReset` reported |
| 12-34 | Continues through subsequent versioned conditions | Existing retained column/Auto DJ transforms apply with the [documented thresholds](settings-history.md) |

Future, negative, fractional, nonnumeric or missing combined/export versions fail explicitly. The observed split-key default remains version 14 when its copied version is absent; no version-zero guess is made. Authentication/player/timestamp matrices are unchanged.

## Reset scope and precedence

A combined/export v10/11 copy or authoritative split snapshot with a General category resets only retained SETTINGS fields: language `en`, follow-system-theme false, no imported column overrides, and the pinned initial Auto DJ configuration. Profiles, selected server, queue occurrences/current occurrence and saved queue position are unchanged. Removing all override entries means a future view uses its defaults; it does not mean an explicit empty array in every view. Full source UI default columns and other settings are not reconstructed by this subset.

An authoritative reset ignores supplemental weaker split copies, with ignored-copy diagnostics, so old copy values cannot undo the reset. With no authoritative reset, a selected supplemental reset-version category resets only that category. Current stronger categories and unrelated target state remain intact. These mixed-copy rules extend the existing partial-overlay contract; they do not claim that the original app combines independent snapshot versions in this way.

Malformed envelopes do not produce reset candidates. Version 10 must include an object General category with a `sidebarItems` array, because the original method calls `push` before returning `{}`. A malformed marker category or a non-object state fails without writes. Version 11's original method also returns `{}` for null state/General inputs; the target deliberately rejects those malformed typed copies rather than allowing accidental resets. Source observations and target differences are captured in regression fixtures.

Missing whole categories in early/non-reset partial copies preserve the target baseline and report deferred defaults. A provided early Auto DJ object can fill missing fields but cannot silently translate existing aliases: versions 0-9 return before the source's alias-normalization branch. Other unretained sidebar/window/playback fields remain deferred and are never claimed applied. The v8 sidebar and early Linux window-bar effects are recorded as source observations only.

## Preview, apply and rollback

Reset detection does not write or activate application state. The existing [journal contract](import-journal.md) prepares an immutable candidate, checks current-plus-backup capacity and rejects stale baselines. Apply persists exactly the reviewed reset projection and retains the previous target snapshot; rollback restores that exact previous data, including profiles and queue position. Repeated freshly prepared equivalent resets are idempotent. Failed writes and malformed reset inputs preserve the target. No source file/profile is opened for mutation, credentials are not imported and no shell/production owner adopts this data yet.

The low-level array/Auto DJ helpers return `ImportError::SettingsReset` for 10/11. They cannot represent a whole settings reset as an empty per-view array, preventing callers from bypassing the preview-level reset scope.

## Evidence and limitations

The [early source fixture](../../../tests/fixtures/p02/settings-migrations/early-source-replay.json) captures 56 exact original migration/default-merge cases for all versions 0-13, plus five malformed observations. It uses synthetic structural inputs, exact retained defaults and the same hash-pinned VM boundaries as the [earlier source replay](settings-history.md#verification-and-reproduction). Existing 273 column and 110 Auto DJ replays remain unchanged and still reproduce.

The default Rust tests run from tracked inputs. Optional source reproduction adds `--early` to the existing replay command; explicit `--early --write` regenerates this task's fixture after hash verification. The tooling regression checks both output modes reject modified source before loading a compiler or writing either fixture. Tests cover combined/split equivalence, early aliases/defaults, reset scope, priority of independent copies, malformed cases and exact journal rollback; the browser fixture executes reset/reload/undo with real origin storage.

See [verification](early-settings-verification.md) and [migration mapping](../migration/p02-settings-transforms.csv). The numeric version range is covered for the retained schema; older real-user profile evidence, unknown legacy fields, complete settings/queue metadata, production adoption, P02.06 credential policy and P02.07 cache lifecycle remain open. P00/P01/P02 exits are not declared.
