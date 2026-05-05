let wasmModule: any = null

async function loadWasm() {
  if (!wasmModule) {
    wasmModule = await import('../src/wasm/trending_agent_wasm')
    // Inject OpenRouter API key if configured
    if (typeof __OPENROUTER_KEY__ !== 'undefined' && __OPENROUTER_KEY__) {
      wasmModule.set_openrouter_key(__OPENROUTER_KEY__)
    }
  }
  return wasmModule
}

declare const __OPENROUTER_KEY__: string

export async function runReport(): Promise<string> {
  const wasm = await loadWasm()
  const result = await wasm.run_report()
  return result as string
}
