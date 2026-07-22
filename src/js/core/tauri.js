function tauri() {
    if (!window.__TAURI__) {
        throw new Error('window.__TAURI__ todavía no está listo (¿se llamó antes de DOMContentLoaded?)');
    }
    return window.__TAURI__;
}

export function invoke(cmd, args) {
    return tauri().core.invoke(cmd, args);
}

export function listen(event, handler) {
    return tauri().event.listen(event, handler);
}

export const updater = new Proxy({}, {
    get: (_target, prop) => tauri().updater?.[prop]
});

export const tauriProcess = new Proxy({}, {
    get: (_target, prop) => tauri().process?.[prop]
});