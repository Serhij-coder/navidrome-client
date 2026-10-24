# P01.06 composition and scaling

The filter and inert CSS editor now retain Japanese preedit replacement across Home/End during composition. An ordered, editor-scoped guard removes only those two navigation keys while nonempty preedit is active. Original IME, text and paste events remain intact. Commit, cancellation, pointer press, Tab, Escape and editor/window focus loss release the guard. Ordinary Home/End and table navigation remain available outside composition.

The exact failing RawInput trace focused an empty filter, sent Enabled, Preedit `に`, Preedit `にほん`, ArrowDown/Home/End/Shift+F10, then Commit `日本`. Before remediation the filter retained `にほん`; after remediation it contains `日本`. This is a pinned toolkit-boundary reproduction, not evidence that a physical operating-system IME sends this sequence.

In egui 0.33.3, `widgets/text_edit/builder.rs:1088` guards commit replacement by the secondary cursor position. Home/End can move that cursor while preedit is selected; `remove_ime_incompatible_events` at line 1139 filters arrows but not these keys. egui-winit 0.33.3's keyboard translation can forward Home/End independently of IME state. The browser eframe composing-key path filters composing keydowns. These source observations justify a narrow local compatibility treatment; no Feishin defect or general OS IME repair is claimed.

The [seven private regressions](../../../tests/spike-ui/text_input.rs) exercise actual egui RawInput and TextEdit state: Japanese prediction replacement, cancellation, Arabic and combining characters, multiline paste/replacement, focus isolation, ordered same-frame commit/disable/navigation, pointer/focus transfer and retained editor selection under zoom/native-density changes. They also retain bounded virtualized rows and row height. [The guard](../../../crates/spike-ui/src/text_input.rs) uses stable editor identities and constant-size state; tests remain under the owning crate's private test boundary.

A composition beginning in the same RawInput batch as acquisition of a different editor is not reconstructed. Pointer presses deliberately end guard ownership so a later table key in that batch cannot be swallowed. Real OS IME candidate windows, simultaneous focus/composition delivery, physical mixed DPI, font coverage, shaping and RTL reading order remain open P01 acceptance cases. Unicode text preservation is separate from whether default fonts render every glyph.

The public bounded table-status summary supports a visible polite DOM status region in both browser roots. It exposes row/selection counts and the focused synthetic record, without query/CSS text or state setters. [Browser input evidence](browser-input.md) records actual pointer/touch and Unicode delivery separately from these composition-event tests. This status region does not provide a complete semantic browser widget tree.

Focused validation:

```sh
cargo test -p feishin-spike-ui --lib --locked
cargo clippy -p feishin-spike-ui --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

The delegate passed all 20 regular UI library tests, including seven text-input, five track-action and two keyboard tests; the existing concurrent lab probe remained explicitly ignored. Combined integration and independent review results are recorded in the [native-control/input manifest](evidence/controls-input-manifest.json). Production text controls/localization, imported settings and full platform parity remain deferred.
