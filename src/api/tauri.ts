export async function runReport(format: string = 'markdown'): Promise<string> {
    try {
        const result = await invoke('run_report', { format });
        return result as string;
    } catch (error) {
        console.error('Tauri invoke error:', error);
        throw error;
    }
}

export async function getSentiment(): Promise<any> {
    try {
        const result = await invoke('get_sentiment');
        return result;
    } catch (error) {
        console.error('Tauri invoke error:', error);
        throw error;
    }
}
