# ADR-010: development identity, analytics and updates

Status: development defaults adopted within existing project rules; final release identity and distribution policy remain open for the release gate.

Keep the public description and jeffvli/Feishin attribution. Use "Feishin Rust" only as the working documentation name; no final product name, reverse-domain app ID, signing identity, update endpoint or new remote has been invented. Preserve the GPL-3.0 license. Do not reuse `org.jeffvli.feishin` for an independent release or overwrite the original profile. Release identity must be finalized before P13 packaging.

No external analytics is added. The source renderer HTML conditionally loads `umami.jeffvli.org/script.js`; do not carry its site ID or enrollment into the rewrite. Controlled characterization uses an isolated profile with `umami.disabled=1` and web `ANALYTICS_DISABLED=true` before first page load. Private mode must suppress reporting and integrations according to the agreed domain contract. No live profile, credentials or authenticated URLs enter tracked evidence.

Do not inherit Feishin's GitHub updater owner/repository/channel. Updates are inactive until this project's identity, signed artifact policy, user controls, rollback behavior and trusted source are recorded. P00 does not publish a release, deploy or send messages. Future Linux releases require architecture-specific executable AppImages with LICENSE and complete notices plus clean-environment startup/assets/playback evidence.

Alternatives: copying upstream analytics/update configuration would contact unrelated infrastructure and misidentify releases; silently removing feature support would also lose scope. Retain update preferences and integration workflows in the migration backlog while requiring an explicit project policy before enabling network effects.
