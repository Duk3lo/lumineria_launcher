const { listen } = window.__TAURI__.event;

const panel = document.getElementById('console-panel');
const output = document.getElementById('console-output');
const MAX_LINES = 3000;
let lineCount = 0;

function appendLine(text) {
    output.textContent += text + '\n';
    lineCount++;
    if (lineCount > MAX_LINES) {
        const lines = output.textContent.split('\n');
        output.textContent = lines.slice(lines.length - MAX_LINES).join('\n');
        lineCount = MAX_LINES;
    }
    output.scrollTop = output.scrollHeight;
}

export function initConsole() {
    document.getElementById('console-toggle-btn')?.addEventListener('click', () => {
        panel.classList.toggle('hidden');
    });
    document.getElementById('console-close-btn')?.addEventListener('click', () => {
        panel.classList.add('hidden');
    });
    document.getElementById('console-clear-btn')?.addEventListener('click', () => {
        output.textContent = '';
        lineCount = 0;
    });

    const parsePayload = (event) => {
        if (typeof event.payload === 'string') return event.payload;
        if (event.payload && event.payload.line) return event.payload.line;
        return JSON.stringify(event.payload);
    };

    listen('process-log', (event) => appendLine(parsePayload(event)));
    listen('game-log', (event) => appendLine(parsePayload(event)));
}