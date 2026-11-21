# Accessibility

Issue #698. Target: **WCAG 2.1 Level AA**.

Sources: `frontend/src/a11y/` (colour tokens, contrast maths, focus helpers,
global styles) and `frontend/src/tests/accessibility.test.tsx`.

## Accessibility statement

Stellar Nebula Nomad aims to be usable by everyone, including people who use
screen readers, keyboard-only navigation, screen magnification, or reduced
motion.

**Conformance status:** the shared component library (`frontend/src/components`),
the Market Oracle dashboard and the Treasure Vault panel are designed and
tested against WCAG 2.1 AA. They are *partially conformant* for the
application as a whole, because the larger feature views listed under
[Known limitations](#known-limitations) have not been audited yet.

**Compatibility:** the target environments are current Chrome, Firefox and
Safari, with NVDA (Windows), VoiceOver (macOS and iOS) and TalkBack (Android).
Automated checks run in CI; the screen-reader checks are part of the manual
release checklist below.

**Feedback:** if something is hard or impossible to use, open a GitHub issue
with the `accessibility` label. Describe the page, what you tried, and your
browser and assistive technology.

## What was fixed (audit results)

| Area | Problem found | Fix | WCAG |
| --- | --- | --- | --- |
| Colour contrast | White text on `#3B82F6`, `#10B981`, `#F59E0B`, `#EF4444`, `#6366F1`, `#8B5CF6`, `#EC4899` failed 4.5:1 (as low as 2.1:1). `#64748B` text and icons on dark surfaces failed. | Shared AA-checked tokens (`a11y/colors`). Darker fills behind white text, dark text on amber. Muted text raised to `#94A3B8`. | 1.4.3, 1.4.11 |
| Dialogs | `role="dialog"` sat on the backdrop. No focus moved in, no Tab trap, hardcoded `id="modal-title"`. | Dialog role on the panel. Focus moves in, Tab/Shift+Tab are trapped, Escape and backdrop close it, focus returns to the opener. Unique ids. | 2.1.2, 2.4.3, 4.1.2 |
| Tabs | No arrow-key support. Ids collided between instances. | Roving tabindex, Arrow/Home/End (mirrored in RTL), unique ids, focusable panel. | 2.1.1, 4.1.1 |
| Form fields | Hardcoded input ids (duplicates on a page). Error text not linked. Group labels not associated with their buttons. | `useId`, `aria-describedby` and `role="alert"` on errors, `role="group"` with `aria-labelledby`, `aria-pressed` on toggles. | 1.3.1, 3.3.1, 4.1.2 |
| Ship card | A focusable `div` acting as a button, Enter only. | A real "Select {name}" button that appears on focus. The card stays clickable for mouse users. | 2.1.1, 4.1.2 |
| Live regions | Every toast was `role="alert"`. Static badges were `role="status"`. | Only warnings and errors are assertive; the rest are polite. Badges are labelled groups. | 4.1.3 |
| Images and icons | Decorative emoji were read aloud. The nebula label omitted the anomaly. | `aria-hidden` on decoration. The label now includes all visual information. | 1.1.1 |
| Data tables | `role="table"` on a `<table>`. No header scopes. | Native semantics, `<caption>`, `scope="col"` and `scope="row"`. | 1.3.1 |
| Colour only | The current leaderboard row was marked by tint only. | `aria-current`, an inline edge and a "(you)" label. Status badges pair an icon with text. | 1.4.1 |
| Capacity bar | Purely visual. | `role="progressbar"` with values and a label. | 1.3.1 |
| Landmarks and bypass | No skip link. Unlabelled `aside`. | A "Skip to main content" link, a focusable `<main>`, and a labelled `<nav>`. | 2.4.1 |
| Focus visibility | Browser defaults only. | A 3px `:focus-visible` ring in a 3:1 colour. | 2.4.7 |
| Motion | Pulsing skeletons and transitions ignored user settings. | `prefers-reduced-motion` disables them. | 2.3.3 (AAA, best effort) |
| Language | `<html lang>` was never set. | Set and kept in sync by the i18n layer, together with `dir`. Language names carry `lang`. | 3.1.1, 3.1.2 |
| Disabled actions | Disabled buttons give screen-reader users no reason. | The vault claim button uses `aria-disabled` plus a description, so it stays focusable and explains itself. | 3.3.2 |

## How it is tested

Automated (`npm test` in `frontend/`):

* **axe-core** (`jest-axe`) audits every shared component and both new views,
  once in English and once in Arabic (RTL). The suite fails on any violation.
* **Contrast:** jsdom cannot lay out pages, so axe skips its colour-contrast
  rule there. `accessibility.test.tsx` checks every token pair directly against
  4.5:1 (text) and 3:1 (borders, icons, focus ring). It also proves that the
  old colours failed.
* **Keyboard and focus:** dialog focus-in, Tab trapping, Escape and focus
  restore; tab arrow, Home and End keys, including RTL; unique ids; the skip
  link and the main landmark.
* **Screen-reader semantics:** accessible names and descriptions, pressed
  states, error announcement, live-region roles, progress bars, table names
  and `aria-current`.

Manual checks before each release:

1. **Keyboard only:** complete the core flows (navigate, open and close a
   dialog, switch tabs, discover and claim a vault, change language) without
   a mouse. Focus must always be visible and follow a logical order.
2. **Screen readers:** NVDA + Firefox and VoiceOver + Safari. Check headings,
   landmarks, form labels and errors, live announcements (toasts, key balance)
   and dialogs.
3. **Zoom and reflow:** 200% browser zoom and a 320px-wide viewport, with no
   loss of content and no two-dimensional scrolling outside data tables.
4. **Contrast in context:** run the axe browser extension on the running app,
   where real layout makes its contrast rule work.
5. **Reduced motion:** with the OS setting on, nothing should pulse or
   animate.

## Rules for contributors

* Use the colour tokens from `a11y`. Add any new text/background pair to the
  contrast test.
* Prefer native elements (`<button>`, `<table>`, `<label>`) over ARIA roles.
* Every input needs a `<label htmlFor>`, and ids come from `useId()`.
* Never convey state by colour alone; add text or an icon with text.
* Decorative icons get `aria-hidden="true"`. Meaningful icons need text.
* A button's visible text must appear in its accessible name (WCAG 2.5.3).
* Use logical CSS properties so RTL works.
* Add the component to the axe gallery in `accessibility.test.tsx`.

## Known limitations

* The large feature views (`Marketplace`, `GuildManagement`,
  `AnalyticsDashboard`, `ShipManagement`, `Tournaments`, the analytics panels,
  `GuildQuestBoard`, `ContentEditor`, `NebulaRenderer`) have not yet been
  audited or moved to the shared tokens. They are the next step.
* The 3D `NebulaRenderer` canvas has no text alternative for its interactive
  content yet.
* Contrast is verified on token pairs, not on every rendered gradient.
  Text over the nebula gradients relies on the dark end of the gradient.
