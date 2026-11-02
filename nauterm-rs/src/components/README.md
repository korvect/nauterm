# Product components

Application controls own their appearance, interaction states and motion over
GPUI Base's focus, activation, editing, IME and positioning primitives. Workspace
and settings use distinct skins over shared behavior; terminal tools pass their
terminal palette explicitly.

```rust,ignore
use crate::components::{Button, Field, Input};
use crate::components::settings::{Button as SettingsButton, Input as SettingsInput};
use gpui_kit::{component::Sizable, px};

Button::new("retry").small().primary().label("Retry");
SettingsButton::new("done").accent().label("Done");
Input::new(&search).aria_label("Search");
Input::multiline(&script).h(px(100.)).aria_label("Script");
Field::input(&password, "Password").secret(true);
```

## Composition

- Reuse the application Input, Select, Button, Menu, Overlay, Modal and scrollbar
  components. Multiline editing is an Input variant, not a separate public
  Textarea component. Workspace fields use floating labels; settings fields
  use external labels.
- Reuse `theme_preview::ThemePreview` for terminal theme samples in tools,
  host editing, galleries and settings. Use the terminal control skin for both
  terminal appearance fields and inline snippet forms.
- Use Lucide for standard icons, including X, Plus, ChevronDown and CircleCheck.
  Keep only product-specific shapes in the application glyph catalog.
- Use stable resource IDs for retained controls and management cards. Selection,
  hover and focus must not change their layout dimensions.

## Interaction

- Buttons activate on release inside the control. Ordinary
  buttons use ripple feedback; resizing titlebar tabs do not.
- Menus and Selects must not implicitly highlight the first option. Selects
  retain the committed selection independently from hover and keyboard focus.
  Multi-select options remain visible and the menu stays open after selection.
- Read-only Selects do not show an editing cursor. Editable and multi-select
  fields can clear; filtering implies editing. Affix buttons retain pointer
  cursors and do not steal editor focus. Chevrons rotate when menus open.
- All Select lists show scrollbars only while hovered or dragging. Shared scroll
  components own nested scrolling and platform wheel conversion.
- Overlay owns layering, positioning, dismissal and focus return. Dialogs use
  Modal and DialogFrame; clicking their backdrop does not dismiss by default.
  Preserve the opaque surface while animating the backdrop and surface motion.
- Keep validation beside the relevant form field using `Field::error` or
  `validation_message`. Operational failures use the owning window's Message
  host. Use `message::watch_many` for independent error sources.
- Use the shared typography roles and reduced-motion-aware transitions. Test
  pure state and layout calculations automatically; verify visuals, focus,
  keyboard, IME and pointer interactions manually.
