// Minimal browser APIs that some app code expects.

if (!('matchMedia' in window)) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ;(window as any).matchMedia = (query: string) => {
    return {
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false
    } as MediaQueryList
  }
}

