// Minimal typing for Fluent Web Components custom elements in Svelte templates.
//
// We intentionally keep these as `any` to avoid having to mirror the full FAST/Fluent attribute surface.
// Runtime behavior is still validated via existing unit tests and manual Tauri runs.
declare namespace svelteHTML {
  interface IntrinsicElements {
    'fluent-button': any
    'fluent-card': any
    'fluent-text-field': any
    'fluent-number-field': any
    'fluent-select': any
    'fluent-option': any
  }
}

