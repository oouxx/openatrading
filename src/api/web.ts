let wasmModule: any = null;

async function loadWasm() {
    if (!wasmModule) {
        wasmModule = await import('../wasm/trending_agent_wasm');
    }
    return wasmModule;
}

export async function runReport(format: string = 'markdown'): Promise<string> {
    const wasm = await loadWasm();
    try {
        const result = await wasm.run_report(format);
        return result;
    } catch (error) {
        console.error('WASM call error:', error);
        throw error;
    }
}

export async function getSentiment(): Promise<any> {
    const wasm = await loadWasm();
    try {
        const result = await wasm.get_sentiment();
        return result;
    } catch (error) {
        console.error('WASM call error:', error);
        throw error;
    }
}
