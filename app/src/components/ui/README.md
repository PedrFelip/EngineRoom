# Shared controls

Use `Button` for ordinary actions and `Input` / `Textarea` for text entry.
Use `SwitchField` for immediately applied on/off settings with a title and
optional description. It owns unique IDs, label association and hint wiring.
Use bare `Switch` in compact toolbars, with an explicit label or `aria-label`.
Keep checkboxes for selection in a form rather than immediate settings.

Use `SegmentedControl` for the existing segmented selections. Its selection
changes immediately; do not add transitions at a call site.

- Controls own focus, hover, disabled and motion behavior. Their CSS lives
  beside the component and is imported by that component.
- Pages choose variants, sizes and layout. Do not style descendants with
  selectors such as `.home-screen button` or `[role='dialog'] input`.
- Add a shared variant when an interaction treatment is needed in multiple
  places; avoid copying its classes into each page.
- `Input embedded` is for a surrounding field with an icon. The wrapper must
  provide a visible `focus-within` indicator; the input keeps its accessible
  label. Use `htmlFor` and a unique input ID for custom-component labels.
- Native elements remain appropriate for board interactions, move rows,
  overlay hit surfaces, file inputs and range/checkbox controls. These own
  their interaction styling; the global base focus is only a fallback.
- Preserve the existing palette. Check keyboard focus, disabled states,
  reduced motion and touch targets when changing shared controls.
