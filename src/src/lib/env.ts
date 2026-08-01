/**
 * 当前是否运行在 Tauri 桌面环境中。
 *
 * Tauri 在运行时向 `window` 注入 `__TAURI_INTERNALS__`，
 * web 浏览器没有这个属性。
 */
export const isTauri = "__TAURI_INTERNALS__" in window
