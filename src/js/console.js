// console.js
const { listen } = window.__TAURI__.event;

const panel = document.getElementById('console-panel');
const output = document.getElementById('console-output');
const MAX_LINES = 2000;

let lines = [];
let pending = [];
let flushQueued = false;
let isPanelOpen = false;

function queueFlush() {
    if (flushQueued) return;
    flushQueued = true;
    requestAnimationFrame(flush);
}

function flush() {
    flushQueued = false;
    if (pending.length === 0) return;

    lines.push(...pending);
    pending = [];
    if (lines.length > MAX_LINES) {
        lines = lines.slice(lines.length - MAX_LINES);
    }
    if (isPanelOpen) {
        output.textContent = lines.join('\n') + '\n';
        output.scrollTop = output.scrollHeight;
    }
}

function appendLine(text) {
    pending.push(text);
    queueFlush();
}

export function initConsole() {
    document.getElementById('console-toggle-btn')?.addEventListener('click', () => {
        panel.classList.toggle('hidden');
        isPanelOpen = !panel.classList.contains('hidden');
        if (isPanelOpen) {
            output.textContent = lines.join('\n') + (lines.length ? '\n' : '');
            output.scrollTop = output.scrollHeight;
        }
    });
    document.getElementById('console-close-btn')?.addEventListener('click', () => {
        panel.classList.add('hidden');
        isPanelOpen = false;
    });
    document.getElementById('console-clear-btn')?.addEventListener('click', () => {
        lines = [];
        pending = [];
        output.textContent = '';
    });

    const parsePayload = (event) => {
        if (typeof event.payload === 'string') return event.payload;
        if (event.payload && event.payload.line) return event.payload.line;
        return JSON.stringify(event.payload);
    };

    listen('process-log', (event) => appendLine(parsePayload(event)));
    listen('game-log', (event) => appendLine(parsePayload(event)));
}