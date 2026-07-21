// Punto único de acceso a la API de Tauri.
// Antes cada módulo hacía su propio "const { invoke } = window.__TAURI__.core;".
// Centralizarlo acá evita repetirlo en 8 archivos distintos y deja un solo lugar
// para adaptar el código si algún día cambia la forma de acceder a Tauri.

const TAURI = window.__TAURI__;

export const invoke = TAURI.core.invoke;
export const listen = TAURI.event.listen;
export const updater = TAURI.updater;
export const tauriProcess = TAURI.process;
