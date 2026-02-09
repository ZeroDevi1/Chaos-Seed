// Minimal shim so TypeScript can import `.svelte` files.
declare module '*.svelte' {
  const component: any
  export default component
}

