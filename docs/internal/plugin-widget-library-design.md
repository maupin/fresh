# Plugin widget library — design proposal

Status: proposal (rev 2)
Author: staff-eng review, branch `claude/plugin-ui-component-library-wO76I`
Scope: shared UI components for the Fresh plugin runtime
Criterion: end-state UX, robustness, flexibility. Shipping speed is
explicitly *not* a constraint; risk-reduction and time-to-merge are
not optimization targets here. If you are optimizing for those, see
Appendix A and the parallel proposal on
`claude/design-plugin-ui-library-pxri8`.
Related: `docs/internal/UNIFIED_UI_FRAMEWORK_PLAN.md`,
`docs/internal/unified-hit-test-theme-plan.md`,
`docs/internal/unified-keybinding-resolution.md`,
`docs/internal/event-dispatch-architecture.md`,
`docs/internal/visual-layout-unification.md`,
`docs/internal/plugin-usability-review.md`,
`docs/internal/settings-controls-usability-report.md`

Rev 2 changes vs. rev 1: `Transient` and `Layer` promoted to v1
widgets; new §3.5 (layered Compositor unifying `Popup`/`Prompt`/
`showActionPopup`/hover/modals); §6 expanded with Spec-as-first-class
state (session restore, theme switch, replay, headless render,
cross-plugin composition); Appendix A engaging the rejected TS-only
alternative; framing throughout sharpened on UX/robustness/flexibility.

> Note. The brief referenced `lib/text_area.ts` and
> `docs/internal/search-replace-ux-improvements.md`; neither exists in
> the tree. Where the brief asked for them as evidence, we substitute
> the equivalent live sources: the Rust `view/controls/*` modules,
> per-plugin call-sites (`search_replace.ts`, `git_log.ts`,
> `dashboard.ts`, `audit_mode.ts`, `lib/finder.ts`), and the open
> §-items in `plugin-usability-review.md` and
> `settings-controls-usability-report.md`.

---

## 1. Recommendation

**Hybrid: a Rust-resident widget runtime with a thin TypeScript
declarative front-end. Plugins describe widgets as data, the host
reconciles, owns layout/hit-test/focus, and emits semantic events.
The existing `setVirtualBufferContent` primitive stays as the
escape-hatch.**

This is the only shape that simultaneously satisfies the five
constraints in the brief:

| Constraint | Pure-TS on `setVirtualBufferContent` | Pure Rust-core | Hybrid (proposed) |
|---|---|---|---|
| Per-keystroke cost | Full re-serialization + buffer replace per char (today: `delete-all + insert-all + bulk overlay add`, `crates/fresh-editor/src/app/virtual_buffers.rs:356`) | One IPC call carries `set_input_value`, no JS render | TS sends a delta against last spec; reconciler in Rust applies the minimum mutation |
| Theme | Plugin must hard-code `"syntax.keyword"` etc.; no abstraction over palette intent | Widget asks `theme.button_fg()` directly | Widgets carry *roles* (`Role::Action`, `Role::Toggle`); core resolves to the active `Theme` (`crates/fresh-editor/src/view/theme/types.rs:1116`) |
| Reach (built-ins) | Built-ins stay in `view/controls/*`; no sharing | Plugins call the same widgets the settings pane uses | Settings keeps using the Rust controls; plugins describe widgets with the same `Spec` enum, rendered by the same Rust code |
| Backwards compat | Native; nothing to do | Hard cut-over; hundreds of `TextPropertyEntry[]` call-sites | New API is additive; `setVirtualBufferContent` and `defineMode` stay; widgets are mounted *into* a virtual buffer the plugin already owns |
| Sandboxing | No new capabilities granted (good) | Risk: a Rust widget that "does the thing" gives the plugin capabilities it didn't have | Widgets emit *events*, never side-effects. A `Button` fires `onActivate` back into the plugin; the plugin still calls existing APIs to do the work. No new capability bits. |

### Why not pure TypeScript on the existing primitive

It is tempting because nothing on the Rust side changes and 100% of
the existing call-sites keep working. But:

- Per-keystroke cost is non-trivial. `set_virtual_buffer_content` is
  a *full* replacement — every keystroke deserializes the entries
  array, deletes the buffer's bytes, inserts the new text, and
  rebuilds the inline-overlay tree
  (`virtual_buffers.rs:356–405`).
- Theme integrity is unenforceable. Plugins already each ship their own
  palette guesses; a TS-only library has no leverage to make
  high-contrast or color-blind themes work.
- Built-in surfaces (`crates/fresh-editor/src/view/settings/`,
  `view/controls/{button,dropdown,toggle,number_input,text_input,text_list}`)
  cannot share code with TS. The split codebase is exactly what
  `UNIFIED_UI_FRAMEWORK_PLAN.md` set out to fix; doubling it would be a
  regression.
- Hit-testing math (`buffer_row → which widget`) keeps living in every
  plugin. `dashboard.ts` already has a `rowActions: Map<row, Range[]>`,
  `audit_mode.ts` has `entryPropsByRow`, `git_log.ts` does a
  `binarySearch(logRowByteOffsets)`. The library cannot remove this
  without owning hit-test, and the *information needed for hit-test*
  (visual columns, wide chars, ANSI escapes) lives Rust-side
  (`docs/internal/visual-layout-unification.md`).

### Why not pure Rust-core

- Capability inflation is the killer. A Rust `FilePicker` that reads
  the disk gives every plugin the disk-read capability via the widget
  call. The current model — plugins call `editor.openFile` and the host
  enforces — must be preserved.
- The flag-day rewrite is a non-starter. There are 107 plugins on
  `main`. Migration must be opt-in plugin-by-plugin.
- We lose the imperative-buffer escape hatch that
  `audit_mode.ts`, `code-tour.ts`, and the LSP plugins all rely on.

### Why hybrid wins

The Rust side already has the right shape: each control in
`view/controls/*` is `<Name>State + <Name>Layout + <Name>Colors +
render_<name>()`. `UNIFIED_UI_FRAMEWORK_PLAN.md` Steps 1–6 already
extracted `point_in_rect()` and `FocusManager<T>` and migrated Settings
onto them. Steps 7–8 (Menu/Tabs) and the unwritten "TS plugin mirrors"
(`controls.ts`, `vbuffer.ts`) are the missing bridge. This proposal is
that bridge.

The TS side gets a **declarative widget tree** (VS Code TreeView shape),
the Rust side runs a **layered Component compositor** (Helix shape) with
**transient-style keymaps** (Magit shape), and the imperative
`setVirtualBufferContent` survives unchanged for the rare cases where
declarative is the wrong fit (Neovim/modern-Emacs shape). Webview-style
HTML escape hatches are explicitly rejected (Sublime's `on_navigate`
href-dispatch is the safe analogue, and we already have it via
`editor.on("mouse_click", …)`).

---

## 2. Widget catalogue

Distilled from `search_replace.ts`, `git_log.ts`, `dashboard.ts`,
`audit_mode.ts`, `lib/finder.ts`, and the Rust `view/controls/*`
modules. "Today" = needed by an existing call-site; "next" = needed
to close a §-item in `plugin-usability-review.md` or
`settings-controls-usability-report.md`; "later" = speculative.

| Widget | Used by | Props | Internal state | Events | Cohort |
|---|---|---|---|---|---|
| `TextInput` (single line) | `search_replace.ts` (search/replace fields), Settings string controls | `value`, `placeholder`, `password?`, `validator?` | cursor byte offset, selection, undo ring, IME composition | `onChange(value)`, `onSubmit`, `onCursorMove` | today |
| `TextArea` (multi-line) | `search_replace.ts` (the implied lib/text_area.ts), composer plugins | `value`, `lineWrap`, `tabWidth` | as above + scroll offset | `onChange`, `onSubmit(modKey)` | today |
| `Toggle` / `Checkbox` | `search_replace.ts` (case/word/regex), Settings | `checked`, `label` | — | `onToggle(next)` | today |
| `Button` | `git_log.ts` toolbar, `dashboard.ts`, Settings | `label`, `kind: primary\|danger\|ghost`, `disabled` | hover, pressed | `onActivate` | today |
| `List` (virtual-scrolled, item-keyed) | `lib/finder.ts`, `git_log.ts` | `items: Array<{key, render, data?}>`, `selectedKey` | scroll offset, hover index | `onSelect(key)`, `onActivate(key)`, `onContext(key)` | today |
| `Tree` (expand/collapse, lazy children) | `search_replace.ts` (file → matches), `audit_mode.ts` (files → hunks), file-explorer | `roots`, `expandedKeys: Set<key>`, `selectedKey`, `provider: getChildren(key)` | scroll, hover | `onToggleExpand(key)`, `onSelect(key)`, `onActivate(key)`, `onContext(key)` | today |
| `Panel` | every panelled plugin | `title`, `toolbar?: Toolbar`, `body: Widget`, `footer?: HintBar` | focus index across children | `onClose` | today |
| `Toolbar` | `git_log.ts`, `audit_mode.ts` | `items: Array<Button \| Separator \| ToggleGroup>` | — | per-item events | today |
| `HintBar` | every plugin's "?" footer | `entries: Array<{keys, label}>` | — | — | today |
| `Tabs` / `Group` | `git_log.ts` buffer group, Settings categories | `tabs: Array<{key, title, badge?}>`, `activeKey` | — | `onSelect(key)` | today |
| `Prompt` (modal input) | `lib/finder.ts`, every confirm | `title`, `body: Widget`, `actions: Button[]` | as Panel | `onAction(key)` | today (built on `Layer`) |
| `Layer` (anchored or modal compositor layer) | tooltips, popovers, context menus, modals — subsumes today's `Popup`/`Prompt`/`showActionPopup` (see §3.5) | `kind: "tooltip"\|"popover"\|"modal"\|"panel"`, `anchor?: WidgetKey \| Rect`, `body: Spec`, `dismissOn: ["outside-click"\|"escape"\|"blur"\|"hover-out"]` | open?, focus | `onDismiss`, `onAction` | today |
| `Transient` (key-grouped command menu, Magit shape) | discoverability per `plugin-usability-review.md`; replaces `git_log.ts` toolbar key-hint sprawl and `search_replace.ts` tab-stack key-hint footer | `groups: [{title, entries: [{keys, label, command, enabled?}]}]` | `chord` state | `onCommand(id)`, `onDismiss` | today |
| `Table` (columns, sortable, selectable rows) | `git_log.ts` log, `find_references.ts`, audit | `columns`, `rows`, `sortKey?`, `selectedRowKey?` | scroll, hover, sort | `onSort`, `onSelectRow`, `onActivateRow` | today |
| `KeybindingList` | mirror Rust `keybinding_list/` | as Settings | as Settings | `onChange(binding[])` | next |
| `MapInput` | mirror Rust `map_input/` | as Settings | as Settings | `onChange(map)` | next |
| `Diagnostic` / `InlineHint` | LSP plugins | `severity`, `message`, `source?` | — | — | next |
| `ProgressBar`, `Spinner` | indexer plugins | `progress?: 0..1`, `label?` | — | — | later |
| `Dropdown` (closed-set picker) | Settings | `options`, `selectedKey` | open?, hover | `onSelect` | later |

A `Button` and a `Toggle` carry a *role* (`Role::Action`,
`Role::Destructive`, `Role::Selected`, `Role::Disabled`). The library
maps roles → theme keys; there is no `fg: [r,g,b]` in widget props.
Plugins that genuinely need a custom color use the imperative escape
hatch.

The catalogue is intentionally short. Anything not on this list (rich
text, syntax-highlighted code, custom drawing) lives inside an
imperative-virtual-buffer widget — a `RawBuffer` widget whose body is
just a `TextPropertyEntry[]` produced by the plugin.

---

## 3. Layout primitive

**Line-oriented flex along the row axis, absolute along the column
axis, with a small Rect-based composition layer.** Three reasons:

1. The terminal is row-major. Every plugin already thinks in rows. A
   web-style flex-everywhere model would force plugins to reason about
   things terminals don't have (sub-row positioning).
2. The *interesting* layout question is column distribution: a
   `Toolbar` packs left-to-right, a `Panel`'s body fills, a `HintBar`
   packs right-to-left. That's `flex-row` with `grow/shrink` on
   children, the same shape the Rust controls already have implicitly.
3. The terminal-line-wrap problem (toolbars must not wrap) is solved by
   marking widgets `wrap: "never"` and letting the host *clip* with
   ellipsis — never line-wrap a widget, never let line-wrap split a
   widget across rows.

API shape (TS):

```
type Spec =
  | { kind: "row"; children: Spec[]; wrap?: "never" | "soft" }
  | { kind: "col"; children: Spec[] }
  | { kind: "fill"; child: Spec }
  | { kind: "fixed"; rows: number; child: Spec }
  | { kind: "widget"; type: WidgetType; props: ...; key: string }
  | { kind: "raw"; entries: TextPropertyEntry[] };
```

`raw` is the integration with `setVirtualBufferContent` —
existing plugins gain widgets *inside* a buffer they already own
without rewriting their renderer. The host composes `raw` regions with
widget regions on the row axis.

Horizontal scroll is a property of `RawBuffer`, `Table`, and `List`
content; it is not a layout-level concern. Toolbars set
`wrap: "never"` and clip; this is consistent with how the Rust
`view/controls/keybinding_list/` already truncates.

---

## 3.5 Compositor: layered Components

Today the editor has half a dozen overlapping subsystems for "thing
that paints over content": `Popup` (`crates/fresh-editor/src/view/popup.rs`),
`Prompt` (`view/prompt.rs`), `showActionPopup` (yet another path),
the buffer-group panel renderer, hover tooltips, completion popups.
Each has its own focus stack, dismiss policy, mouse routing, and
keymap precedence. Plugins that want any of them compose ad-hoc, and
the rules for which dismisses which on Escape are scattered across
files.

**Unify them as layers in a single Compositor**, modelled on Helix's
`Component` trait, adapted for IPC:

```rust
trait Component {
    fn render(&mut self, area: Rect, surface: &mut Surface, ctx: &mut Ctx);
    fn handle_event(&mut self, event: &Event, ctx: &mut Ctx) -> EventResult;
    fn cursor(&self, area: Rect, ctx: &Ctx) -> (Option<Position>, CursorKind);
    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)>;
    fn dismiss_policy(&self) -> DismissPolicy;
    fn id(&self) -> ComponentId;
}
```

The Compositor owns a Z-ordered stack of Components. Events bubble
front-to-back until one returns `EventResult::Consumed`; the focused
component is implicitly the topmost layer with `accepts_focus()`. The
Spec runtime is one Component implementation among many. `Popup`,
`Prompt`, `showActionPopup`, hover tooltips, and the completion popup
become other implementations of the same trait — sharing dismiss,
mouse routing, focus, and accessibility paths.

The plugin-facing surface is a single `mountLayer` IPC:

```ts
const tooltip = editor.mountLayer({
  kind: "tooltip",                 // "tooltip" | "popover" | "modal" | "panel"
  anchor: { widget: "matchTree", row: hoveredRow },
  body: { kind: "widget", type: "InfoCard", props: { ... } },
  dismissOn: ["hover-out", "blur"],
});
// later: tooltip.dismiss();
```

UX wins this enables — none of which are reachable in a TS-only design:

- A hover tooltip on a `Tree` row is a child layer, not a Tree-internal
  feature. *Any* widget can host a tooltip via `mountLayer({ kind:
  "tooltip", anchor: ..., ... })`. Tooltips do not multiply across
  widget implementations.
- A `Button.kind = "danger"` can spawn a confirm modal as `Layer {
  kind: "modal", body: { type: "Prompt" } }`; the same focus, dismiss,
  and event routing the rest of the panel uses. There is no separate
  modal-dialog API.
- Right-click context menus are `Layer { kind: "popover", body: { type:
  "List" } }`. Plugins do not re-implement context menus per panel.
  The `dismissOn: ["outside-click", "escape"]` policy lives once.
- A `Prompt` mounted from inside a panel is the *same* Component as the
  top-level command palette (`view/prompt.rs`). One implementation,
  one keymap, one accessibility story, one set of bug fixes.
- The completion popup an LSP plugin shows over a buffer is a
  Component layer. So is the diagnostic hover. They stack and dismiss
  predictably with respect to each other and to plugin-mounted
  tooltips, because the dispatcher is one place.

**Files affected.** New `crates/fresh-editor/src/compositor/` with the
trait, the stack, the dispatcher, and the `mountLayer` IPC binding.
`view/popup.rs`, `view/prompt.rs`, `view/hover.rs`, and the
action-popup implementation migrate to be `Component` implementations
in successive PRs. None of those migrations is plugin-visible; the
plugin-facing `editor.startPrompt`, `editor.showActionPopup`, etc.
become thin wrappers that mount a layer.

**Why layers instead of "Popup is special"**: in the current code, a
`Prompt` mounted while a `Popup` is open has unspecified dismiss order,
and neither knows about plugin-side panels. A unified compositor makes
the precedence rules data, not control flow. Ditto the
`event-dispatch-architecture.md` Phase 2 dispatcher: it becomes the
Compositor's hit-test, not a parallel system.

---

## 4. Focus / keyboard model

A **panel-level focus stack** with one *Tab cycle* per panel, computed
from the widget tree's flattened tab-stops in declaration order. Each
panel has a single active widget; the host paints focus styling. This
replaces the per-plugin `focusedField` enums in `search_replace.ts`.

### Interaction with `defineMode`

Today plugins call `editor.defineMode("search_replace", [["Tab", "search_replace_tab"], …])`.
That model is fine for *panel-level* commands but wrong for
*widget-level* keys (Backspace, Arrow keys inside a `TextInput`). The
proposal:

- Each widget has a built-in keymap the host handles **before** the
  plugin's mode bindings see the key. A `TextInput` consumes
  Backspace/Arrow/Home/End; a `Tree` consumes Left/Right/Space.
- The plugin's mode bindings remain authoritative for **panel-level**
  keys (Tab, Enter when nothing claims it, Escape, plugin-defined
  chords like `g g`).
- This is exactly Helix's bubble-up `EventResult::Consumed | Ignored`
  semantics, translated across IPC: the host runs widget keymaps
  synchronously; only on `Ignored` does it dispatch the plugin's
  defined-mode handler. No round-trip on keystrokes that widgets
  already eat.

This composes cleanly with `unified-keybinding-resolution.md` (single
resolution path through `KeybindingResolver`): widget keymaps are an
extra layer *above* the resolver, registered when a widget mounts and
unregistered on unmount.

### Global pass-through

Global shortcuts (e.g. `C-,` settings, `C-q` quit) live above
everything. The order is:

1. Global resolver
2. Active widget's built-in keymap
3. Active panel's `defineMode` bindings
4. Buffer/normal-mode bindings (only if `allowTextInput` and unfocused)

A widget can opt out of step 1 only inside a `Prompt` (modal). This
covers the §18 "global-shortcut pass-through" question that the brief
asked about: it lives at the dispatcher, not per-widget.

### Chord bindings

Chords (`g g`, `Space f`) keep working through the existing
`KeybindingResolver` chord state. The widget layer is stateless w.r.t.
chords — a half-finished chord (`g`) is held by the resolver, not by
the widget.

### Terminal constraint

Shift+Enter ≡ Enter at the terminal, Shift+Alt+Enter ≡ Alt+Enter. We
do not bind Shift+Enter as a distinct key. `TextArea` submit is
**Enter** if `singleSubmitsOnEnter`, else **Alt+Enter** (or **Ctrl+J**
where preferred), and Enter inserts a newline. The widget exposes
`submit: "enter" | "altEnter"` and the plugin picks; the library's
default for multi-line inputs is `altEnter`. The `HintBar` shows the
chosen key string.

---

## 5. Mouse model

The host owns all hit-testing. The plugin never sees `(buffer_row,
buffer_col)`; it receives semantic events.

- Each widget instance has a `Rect` (rows × cols) computed by the
  layout. Hit-test dispatcher lives in
  `app/event_dispatch.rs` (per `event-dispatch-architecture.md` Phase
  2) and answers `(col, row) → WidgetHandle` via z-ordered
  `CachedLayout::region_at` (per `unified-hit-test-theme-plan.md`).
- The widget runtime translates a hit into a widget-local event:
  - `Tree` → `onToggleExpand(key)` if column intersects the disclosure
    glyph, else `onSelect(key)`; double-click → `onActivate(key)`.
  - `List` → `onSelect/onActivate(key)`.
  - `Button` → `onActivate`.
  - `TextInput` → cursor placement, selection drag.
- Drag and hover get first-class events:
  `onPress`, `onDrag(dx, dy)`, `onRelease`, `onHover(true|false)`. The
  host coalesces hover into one event per row change.
- Scroll: wheel events route to the deepest widget that declares
  `scrollable: true`. The host owns the scroll offset; widgets that
  need to know it (virtualized `List`, `Tree`) get it via
  `scrollOffset` callback.

This eliminates `dashboard.ts:rowActions`, `audit_mode.ts:entryPropsByRow`,
`git_log.ts:logRowByteOffsets` binary search, and the hundreds of
`buffer_row` arithmetic call-sites the brief mentioned.

---

## 6. State model

**Reactive on the Rust side, declarative on the TS side.** The plugin
re-emits a `Spec` whenever its model changes; the host runs a keyed
reconciler against the previous `Spec` for that panel and applies a
minimal patch.

This is structurally the React-virtual-DOM model, intentionally. It
matters because:

- The plugin author keeps writing the imperative code they already
  write (a `redraw()` function that builds a tree from current
  state). That style is what `search_replace.ts:render` and
  `dashboard.ts:emit` already do; we are only changing what they emit.
- Widget *internal* state (cursor position, scroll offset, expanded
  keys, pressed-but-not-yet-released) is owned by the **Rust** widget
  instance, keyed by stable `key`. The plugin never sees it. This is
  what makes per-keystroke editing free of round-trips: a keystroke in
  a `TextInput` mutates Rust-side state and emits `onChange(value)`
  back to the plugin once; if the plugin doesn't change the panel's
  Spec in response, no re-render IPC happens.
- The plugin can still drive widgets imperatively when needed:
  `editor.widget(key).setValue(s)` is a single command, processed in
  `process_async_messages` like every other plugin command
  (`crates/fresh-editor/src/app/mod.rs:101`).

Lifecycle alignment with `editor_tick`:

1. Plugin event handler runs (e.g., `mouse_click` or a debounce
   timeout).
2. Plugin updates its model and calls `editor.setPanelSpec(panelId, spec)`.
3. Spec is queued as `PluginCommand::SetPanelSpec`.
4. Next `editor_tick` processes the queue (`process_async_messages`),
   diffs against the panel's previous Spec, applies the minimum widget
   tree mutation, marks `needs_render = true`.
5. `render.rs` paints the next frame.

This is the same cadence the imperative API already uses; we are not
adding a new tick or a new render path.

### Spec as first-class state

The Spec is data: a typed, versioned JSON tree with stable `key`s.
Treat it as state, not as a transient render-output. That choice
unlocks five capabilities for free — none of them reachable in a
TS-only design where the rendered `TextPropertyEntry[]` is the only
artifact:

1. **Session restore.** Each panel's last-applied Spec, plus the
   widget-instance state the host owns (cursor offset, expanded keys,
   scroll, focus, hover), is persisted per workspace. Reopening the
   panel restores it without plugin involvement. The plugin's only
   responsibility is to emit a Spec when its model is rebuilt; the
   library re-mounts the persisted instance state against it by
   matching `key`s.
2. **Live theme switching.** When the user changes themes, the host
   re-renders every active Spec against the new theme. No plugin
   round-trip; no tearing. (Today every plugin would need to handle a
   `theme_changed` event and re-emit; in practice none do, so theme
   switches leave plugin-painted regions stale until the next event
   forces a re-render.)
3. **Deterministic replay for bug reports.** A
   `--record-spec-stream` flag captures every Spec mutation plus
   every event the host delivered, plus widget-instance state at
   each tick. Replays reproduce UI bugs without the originating
   plugin's runtime. Equivalent to a Redux time-travel debugger,
   essentially free given the Spec already exists.
4. **Headless rendering.** Specs render to a string buffer and an
   overlay map without an editor. Snapshot tests for plugin UI become
   `assert_eq!(render(spec, theme), expected)` — pure functions. The
   current path (`tests/common/harness.rs:1571` —
   `EditorTestHarness::screen_to_string()`) requires spawning the
   editor, driving keys, and screen-scraping; that stays available
   for end-to-end tests but is not the only option.
5. **Cross-plugin composition.** A finder plugin can embed another
   plugin's Spec as a preview: `{ kind: "embed", spec: otherSpec }`.
   Specs travel as data. The host validates and renders without the
   originating plugin's runtime being involved (the embedded Spec is
   read-only — events bounce back to whichever plugin owns the
   panel).

Robustness implications:

- **Versioning.** `spec.version: 1`. New widget kinds and props arrive
  without breaking old persisted Specs; the reconciler upgrades
  silently or reports a single `SpecVersionMismatch` event.
- **Fault isolation.** A panicking renderer for one widget kind does
  not kill the panel. The reconciler reports `RenderError` for the
  offending subtree, paints a placeholder in its rect, and the rest
  of the panel renders. The plugin gets an event to log.
- **Spec/instance separation.** A plugin can rebuild its Spec from
  scratch on every model change without losing focus, cursor
  position, scroll offset, or partially-typed input. The reconciler
  matches by `key` and preserves all instance state. This is exactly
  the property hand-rolled plugins fail to provide today: changing
  the visible row range in `audit_mode.ts` re-emits the buffer and
  loses the cursor.
- **Imperative escape hatch is bounded.** `editor.widget(key).setValue(s)`
  and `editor.widget(key).focus()` exist for the rare cases where the
  plugin wants to drive a widget without re-emitting the panel Spec
  (e.g. paste handling). They are queued through the same
  `process_async_messages` path; ordering with Spec updates is
  well-defined (FIFO).

---

## 7. Theming

Widgets carry **roles**, never colors. A `Button` with
`kind: "danger"` resolves at render time to
`Theme::action_danger_fg/bg/hover_bg` (we add these keys; they live
alongside the existing 200+ in `crates/fresh-editor/src/view/theme/types.rs:1116`).
Plugin-side overrides are limited to:

```
spec.theme = { "Button.danger.fg": "#ff4400" }   // RGB or another role key
```

— a per-spec map, validated by the host (unknown keys logged, dropped).
This preserves `OverlayColorSpec` semantics
(`fresh.d.ts:600–634`) and routes through `resolve_theme_key` so
high-contrast and color-blind themes Just Work. Plugins that today
hard-code `"syntax.keyword"` for unrelated UI affordances stop doing
that; the migration plan converts the worst offenders.

Crucially, the plugin **cannot** ship its own Theme; it can only
override roles within a panel. The active Theme is always the user's.

---

## 8. i18n

Widgets carry **default English labels** (`Confirm`, `Cancel`,
`Replace All`, `Toggle`, `Expand`, plus aria/screen-reader strings).
The plugin overrides per-instance via props:

```
{ kind: "widget", type: "Button", props: { label: t("replaceAll") } }
```

We do **not** invent a widget-level i18n manifest; per-plugin
`*.i18n.json` (`docs/i18n.md`) stays the authority. The library ships
its built-in defaults in a single `lib-widgets.i18n.json` so the
HintBar's "Tab to next field" string is translatable without touching
any plugin.

---

## 9. Accessibility

Required for v1:

- High-contrast themes flow through naturally because widgets use
  roles. We add an explicit
  `theme.accessibility.high_contrast = true` resolution path the
  widget renderer reads to bump borders and disable subtle hovers.
- Configurable keybindings: every widget action is a named command
  (`tree.toggleExpand`, `list.activate`, `tabs.next`). Users rebind in
  `keybindings.json` against the existing `KeybindingResolver`. Plugins
  do not redefine these.
- Screen-reader output via OSC 52 / IDE bridges: every widget has an
  `aria` string the host emits on focus change and on event. We add a
  `view/accessibility.rs` that consumes widget focus changes and emits
  the appropriate OSC-52 / IDE-bridge messages; this is the same place
  that already serializes selection for clipboard (so we don't fork
  the OSC path).
- Motion-reduction: animations are role-keyed. The library has
  exactly two animations (focus-flash, hover-fade); both are gated on
  `theme.accessibility.reduce_motion`.

Nice-to-have (deferred):

- A full ARIA-tree model (parent/child/level-of). v1 ships
  flat live-region announcements per focus change.
- Live-region throttling (we throttle at one announcement per 100 ms
  to avoid drowning a screen reader during typing).

---

## 10. Migration plan: `search_replace.ts`

`search_replace.ts` is the densest widget user (~1305 lines).
Migration in five passes; no flag day:

### Pass 1 — Mount as a `Panel`, body stays `RawBuffer`

The plugin keeps emitting its existing `TextPropertyEntry[]` but
through a `RawBuffer` widget mounted inside a `Panel`. The HintBar
moves to a real `HintBar` widget. The toolbar of toggles
(case/word/regex) becomes real `Toggle` widgets. Tab cycling between
the toolbar and the body is now host-owned. Net diff: ~150 LOC moved
out of plugin, no functional change. **This validates the panel /
widget infrastructure end-to-end on the most demanding plugin
without touching the parts that are subtly broken.**

### Pass 2 — Replace search/replace fields with `TextInput`

`buildFieldDisplay` and `cursorPos` byte-offset math
(`search_replace.ts:557–565`) delete entirely. Cursor management
becomes the host's. `onChange(value)` is the plugin's new event.
**This is where the per-keystroke IPC saving lands.** Closes §11
(paste support) and §11 (history persistence) as widget-shaped: a
`TextInput` ships a paste handler and an optional history ring; both
are widget props.

### Pass 3 — Replace match list with `Tree`

The hand-rolled `FlatItem` array (`search_replace.ts:249–268`),
expand/collapse arrow handlers (`search_replace.ts:1006, 1037`), and
per-row checkbox rendering (`search_replace.ts:1147–1151`) all delete.
The plugin supplies a `Tree` provider:
`{ getChildren(key), getItem(key) → { label, checked?, badge? } }`.
**This unblocks §4 (mouse expansion of files) — clicking the
disclosure glyph is now a `onToggleExpand(key)` event, and §14
(multi-line match-list rendering) — `Tree` items can be multi-row
because the layout knows how to size them.**

### Pass 4 — Glob filter input as `TextInput` with validator

Closes the remaining §11 item.

### Pass 5 — Delete dead code

The flatten/index plumbing, the byte-offset cache rebuild, the
focus enum, and the keymap entries that the host now owns all go.
Conservative estimate: 400-600 LOC out of `search_replace.ts`.

Each pass ships independently; each one is reviewable; the plugin
keeps working between them.

---

## 11. Smallest first PR

**Title**: `widgets: introduce Spec/Panel scaffolding and migrate
search_replace HintBar`

**Diff shape (no code yet, just the surface that lands)**:

- New Rust file `crates/fresh-editor/src/widgets/mod.rs` —
  `Spec` enum, `WidgetTree`, `Reconciler`, `WidgetHandle`. Re-exports
  the existing `view/controls/*` `*State` types; `render_*` functions
  are the renderers.
- New Rust file `crates/fresh-editor/src/widgets/dispatch.rs` — hooks
  into `app/event_dispatch.rs` for hit-test (extends the dispatcher
  proposed in `event-dispatch-architecture.md` Phase 2).
- New `PluginCommand` variants in `crates/fresh-core/src/api.rs`:
  `MountPanel { panel_id, spec }`, `UpdatePanel { panel_id, spec }`,
  `UnmountPanel { panel_id }`, plus the inverse events
  (`WidgetEvent::Activate { panel_id, key }`, `Toggle`, `Change`,
  `Submit`, `Hover`).
- New TS file `crates/fresh-editor/plugins/lib/widgets.ts` exporting
  `mountPanel`, `Spec`, helpers `Row`, `Col`, `Button`, `Toggle`,
  `Tree`, `List`, `TextInput`, `HintBar`, `RawBuffer`. ~300 LOC,
  declaration only.
- Update `fresh.d.ts` with the new commands and event payloads.
- Migrate **only** `search_replace.ts`'s HintBar to use the `HintBar`
  widget (Pass 1 partial). Lines deleted: ~30. Lines added: ~10.
- One integration test under `crates/fresh-editor/tests/` that spawns
  a stub plugin which mounts a `Panel { HintBar }` and asserts the
  rendered output — this is the test infrastructure every subsequent
  widget needs.

The PR is small enough to land cleanly, exercises the full IPC path
(mount, render, event, unmount), and changes one user-visible thing
(HintBar) so reviewers can verify in tmux.

---

## 12. Prior art — what we steal, what we reject

| System | Steal | Reject | Why |
|---|---|---|---|
| **VS Code TreeView** | Declarative `TreeDataProvider` shape: plugin returns data, host owns hit-test, virtualization, focus | Webview as a generic UI escape hatch — every webview is an extension-authored XSS sink with `postMessage` privilege | Webviews break the sandbox premise; TreeView's declarative shape is exactly the v1 widget-spec model |
| **Helix `Component` trait** | Layered z-ordered components; bubble-up `Consumed | Ignored`; host-owned `cursor()` and `required_size()` | Synchronous Rust trait across FFI | Translation: TS handlers are async with timeout; `Ignored` is the IPC default; the *protocol* survives |
| **nui.nvim** | Widget = "buffer + keymap + lifecycle (mount/unmount)" | "No widget library" stance | TS plugin authors are not Vimscript veterans; sandboxed JS plus opinionated widgets is a better default |
| **Sublime minihtml** | `on_navigate` href dispatch as the safe link primitive (already analogous to `mouse_click`) | HTML/CSS layout subset; no keyboard focus | We need real keyboard widgets, and CSS-flow on a terminal is the wrong fit |
| **Emacs widget.el** | Nothing | The whole library | The well-known critique (resists composition, imperative-by-side-effect) is exactly what we'd reproduce by exposing today's `setVirtualBufferContent` as the only model |
| **Magit transient.el** | Grouped key→command menu as a first-class widget | Lisp-y EIEIO subclassing | A `Transient` widget with `{groups: [{title, entries: [{keys, label, command}]}]}` covers `git_log` and unblocks discoverability per `plugin-usability-review.md` |

---

## 13. Risks and rejected alternatives

### Rejected alternatives

- **TS-only thin helper library** (the parallel proposal on
  `claude/design-plugin-ui-library-pxri8`). Engaged in detail in
  Appendix A. Summary: a coherent v1 *if* shipping speed is the
  binding constraint; the wrong end-state under the criterion stated
  at the top of this document.
- **Replace `setVirtualBufferContent` outright.** Forces a flag-day
  rewrite. Backwards compat is preserved as an *adoption enabler*,
  not a goal — `RawBuffer` lets every plugin keep its current
  renderer until it chooses to migrate.
- **Imperative widget handles only (no declarative Spec).** Considered.
  Plugins would call
  `editor.createButton({label}).onActivate(...).mount(parent)`. This
  is the React-without-JSX model. Rejected because every plugin would
  re-implement reconciliation by hand: `if (currentLabel != newLabel)
  button.setLabel(newLabel)`. The Spec/reconciler model centralizes
  this. Imperative `widget.setValue/setFocus/dismiss` survive only as
  bounded escape hatches (§6).
- **An `iframe`-equivalent (Webview) component.** Rejected on the same
  grounds VS Code itself documents — the security cost dominates the
  flexibility benefit, and we have zero of VS Code's CSP and process
  isolation infrastructure.
- **Per-subsystem ad-hoc layers** (keep `Popup`/`Prompt`/`showActionPopup`
  as separate paths, plugins pick whichever fits). Rejected: layering
  semantics (which dismisses what on Escape, who gets the next mouse
  click, focus-trap correctness) cannot be made coherent without one
  authoritative compositor (§3.5).

### Risks

| Risk | Mitigation |
|---|---|
| Reconciler complexity grows past what one engineer can hold | Keep Spec flat (no nested per-widget keys beyond `key: string`); cap recursion depth; ship the dirtiest plugin (`search_replace.ts`) as the regression test for every reconciler change |
| Per-keystroke event IPC still dominates if plugins re-emit Spec on every keystroke | Document the rule: in `onChange`, never re-emit Spec unless model state actually changed. The lint is "panel.update calls per second"; expose it on the dev HUD. |
| Capability creep through widget callbacks | Widgets only emit *events* the plugin can already subscribe to. Code review checklist item: a new widget MUST NOT introduce a new `PluginCommand`-equivalent capability. |
| Theme role explosion (`Button.danger.hover.fg`...) | Cap the role tree at three levels; review additions in PRs that touch `theme/types.rs` |
| Reach: Settings doesn't actually adopt the widget tree | Keep the *renderers* shared (`view/controls/*::render_*`) and the *Spec* shape compatible. Settings can live on its current direct calls indefinitely; if/when it migrates, the renderers do not move. |
| Plugin author confusion: Spec vs imperative | One way per use-case in the docs. `RawBuffer` exists for *escape hatches*, not for rendering rich UI. |
| Terminal-constraint violations (Shift+Enter etc.) | Static lint in TS: any `keys` string in a `HintBar` or `Transient` matching `^Shift\+(Enter|Alt\+Enter)` is a build error. |
| Drift from the four open plans (`UNIFIED_UI_FRAMEWORK_PLAN`, `unified-hit-test-theme-plan`, `unified-keybinding-resolution`, `event-dispatch-architecture`) | This proposal explicitly builds on them. Land the open dispatcher work *before* migrating Pass 2/3 of `search_replace.ts`. |

---

## 14. Order of landing (Rust-side)

1. `event-dispatch-architecture.md` Phase 2 (`hit_test(col, row)`
   dispatcher) — required by §5.
2. `unified-hit-test-theme-plan.md` `region_at` extension — adds
   widget regions to the dispatcher.
3. `unified-keybinding-resolution.md` collapse — required by §4.
4. `crates/fresh-editor/src/widgets/{mod,dispatch}.rs` — new module,
   re-uses `view/controls/*State`.
5. `crates/fresh-core/src/api.rs` — new `PluginCommand` variants
   (mount/update/unmount + events).
6. `crates/fresh-editor/plugins/lib/widgets.ts` — TS surface.
7. Smallest first PR (§11).
8. Pass 2–5 of `search_replace.ts` migration.
9. Settings migration to the same renderers (paid down opportunistically).
10. `view/settings/` adopts `Spec` for parts it owns (optional;
    the renderers are already shared).

---

## 15. Go / don't go

**Go.** Rust-resident widget runtime; declarative TypeScript Spec;
layered Compositor unifying every overlay subsystem; Spec as
first-class state with session restore, theme switching, deterministic
replay, headless rendering, and cross-plugin composition; theming by
role with central role→key resolution; widget-internal keymaps
claimed in core before plugin keymaps see the keystroke; one render
path serves the Settings UI, the file explorer, the prompt, the
completion popup, and every plugin panel.

Backwards compatibility is preserved — `setVirtualBufferContent`,
`defineMode`, `Popup`, `Prompt`, `showActionPopup` all keep working
exactly as they do today, on top of the Compositor instead of
beside it — but compatibility is treated as an adoption enabler, not
a design goal. No plugin is forced to migrate; every plugin that
does, removes hand-rolled hit-test math, byte-offset cursor
arithmetic, focus enums, and palette guesses, and inherits a
consistent accessibility, theming, and dismissal story.

The maximalist path. There is a smaller-scoped TS-only alternative
(Appendix A); it is the right answer to a different question. For
the question "what should this library *be* in the limit," this is
the answer.

---

## Appendix A — Rejected: TS-only thin helper library

A parallel proposal exists on `claude/design-plugin-ui-library-pxri8`
(`docs/internal/plugin-ui-library-design.md`, 1,231 lines) that takes
the opposite shape: ~800 LOC of TypeScript helpers
(`Widgets.list`, `Widgets.table`, `Widgets.keyValueForm`,
`Widgets.checkboxRow`, `Widgets.buttonRow`, `Widgets.textInput`,
`Widgets.helpFooter`), one `VirtualBufferBuilder` for byte-offset
bookkeeping, a `TextInputState` + `TextInputRouter` wrapping
`mode_text_input`, a `FocusRing<T>` cycle helper, and seven new theme
keys. **Zero new IPC.** Migrates `pkg.ts`, `search_replace.ts`,
`theme_editor.ts` in ~3 weeks, ~10 person-days.

It is a coherent v1 if shipping speed is the binding constraint. It
is the wrong end-state. Every advantage in that proposal collapses
once the criterion is end-state UX, robustness, and flexibility:

| Their advantage | Why it doesn't survive the criterion |
|---|---|
| "No new IPC" | Risk reduction, not UX. New IPC is the price of moving widget state into core, which is the precondition for everything below. |
| "Empirical 3+ plugin gating" | Half-right. Caps `Tree` out of v1 because today only `search_replace` builds one — the brief explicitly named tree expand/collapse as a target widget. The discipline is useful as an *additive filter on speculative widgets*; not as a *cap on the brief's stated requirements*. |
| "Adoption-failure lesson" (panel-manager exists, no plugin uses it) | Real and worth keeping. But it's an API-ergonomics concern, not a scope concern. The fix is "make the API match how plugins already think" (a `redraw()` that returns a Spec — exactly the shape `dashboard.ts:emit` and `search_replace.ts:render` already have); not "ship less." |
| "Bounded migration" (~10 person-days) | Irrelevant under this criterion. |
| "Smaller surface" | The cost — not the benefit — of every UX problem the proposal then defers (mouse routing, tree, accessibility, theme roles, reach). |

Five UX/robustness/flexibility wins the TS-only shape structurally
cannot reach:

1. **Widget-internal keymap claimed before plugin keymaps see it.**
   `TextInput` consumes Backspace, arrows, Home, End, IME composition
   uniformly across every plugin. The TS-only proposal's
   `TextInputRouter` requires each plugin to register these in its
   `defineMode` and forward calls to a per-plugin state object. Some
   plugins will forget. Some will register a slightly different set.
   Behavior drifts by plugin. Mine handles it once in the Component
   layer (§4): the host runs the widget keymap before the plugin's
   `defineMode` bindings are consulted, and only on `Ignored` does the
   plugin see the keystroke.
2. **Hit-testing owned by core.** The TS-only proposal's
   `Widgets.list` returns a `lineToItemIndex` map; plugins wire the
   lookup themselves. Plugins wire it inconsistently. Hover, drag,
   double-click, scroll multiply this. Mine is one dispatcher in
   Rust (§5) that emits semantic events (`onSelect(key)`,
   `onToggleExpand(key)`, `onHover(key, true|false)`) — the plugin
   never sees `(buffer_row, buffer_col)`.
3. **Per-keystroke cost has the right asymptote.** Today's
   `setVirtualBufferContent` is full delete-all + insert-all + rebuild
   overlay tree (`virtual_buffers.rs:356–405`). With widget state
   Rust-side, a keystroke in a `TextInput` mutates Rust state and
   emits one semantic event back; if the plugin's model doesn't
   change, no re-render IPC fires at all. The TS-only proposal
   replaces the whole buffer on every keystroke forever — fine for
   today's panel sizes, the wrong asymptote for inline-match
   decoration, large `Tree`s, or a search panel that streams results.
4. **Theme as roles, not colors.** The TS-only proposal adds 7 theme
   keys (`button_focused_bg`, `toggle_on_fg`, etc.). Plugins still
   pick which key to pass to which widget. Same drift, slightly
   smaller. Theme packs and accessibility variants (high-contrast,
   color-blind, motion-reduced) only stay consistent when the
   role→key mapping is centralized in the renderer, not in every
   plugin's call sites (§7). Mine: plugins pass `kind: "danger"`,
   never colors.
5. **Reach across built-in surfaces.** The Rust `view/controls/*`
   renderers already do button, dropdown, toggle, text_input,
   text_list, map_input, dual_list, keybinding_list. Under this
   proposal they paint plugin widgets too — Settings, file explorer,
   prompts, plugin panels share one render path. The TS-only proposal
   freezes the split forever (its §2.1 acknowledges and accepts: "8,162
   LOC of Rust controls… plugins cannot call any of this," and its
   answer is to build a parallel TS stack that *also* doesn't call any
   of this). Two render paths, two bug surfaces, two accessibility
   stories, two theming implementations, forever.

Three further capabilities the TS-only design forecloses:

- **Layered compositor.** No path to unifying `Popup`/`Prompt`/
  `showActionPopup`/hover/modals/context-menus/completion under one
  dismiss-and-focus model (§3.5). Each stays its own subsystem;
  precedence rules stay scattered.
- **Spec as first-class state.** Session restore, theme switch,
  deterministic replay, headless rendering, cross-plugin composition
  (§6, "Spec as first-class state") all require the Spec to *be*
  state, not transient render-output. The TS-only design has no
  Spec — its widgets return `TextPropertyEntry[]`, which by the time
  the host sees it has lost the structural information needed for
  any of these features.
- **Fault isolation.** A panicking widget renderer in the TS-only
  design takes down the panel render. With Rust-side widget kinds,
  the reconciler can paint a placeholder for the offending subtree
  and keep going.

Where the TS-only proposal is right and we keep its discipline:

- **Anchor every widget to a named plugin's hand-rolled code.** No
  speculative widgets. Their `pkg.ts:16-29` literal `TODO: Plugin UI
  Component Library` is a stronger motivator than the
  `UNIFIED_UI_FRAMEWORK_PLAN.md` Phase 7-8 generality argument —
  carry it forward as the v1-gating filter on *which* widgets land
  in the catalogue.
- **Don't ship retained widget-handle APIs as the primary model**
  (`button.setLabel(s)`). That's the panel-manager failure mode
  they correctly identify. Spec/reconciler is declarative;
  imperative `widget.setValue` exists only as a bounded escape
  hatch (§6).
- **Reuse `mode_text_input` and `defineMode` for the imperative
  escape hatch.** Their `TextInputRouter` is the right shape for
  plugins that opt out of widget-mounted text inputs entirely. Keep
  it as a fallback library, not as the primary path.

**Net.** The TS-only proposal answers "what is the minimum useful help
in the next three weeks?" cleanly and correctly. It does not answer
"what should this library be?" Under the criterion stated at the top
of this document — end-state UX, robustness, flexibility, with shipping
speed deliberately not a constraint — every one of its compromises
shows up later as a UX/robustness ceiling that requires undoing the
API to lift. Build the maximalist version.
