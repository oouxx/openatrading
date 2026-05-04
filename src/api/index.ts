declare global {
    interface Window {
        __TAURI__?: any;
    }
}

const isTauri = typeof window !== 'undefined' && window.__TAURI__;

export const api = isTauri ? await import('./tauri') : await import('./web');

export default api;
