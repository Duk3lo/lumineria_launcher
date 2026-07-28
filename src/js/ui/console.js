import { listen } from '../core/tauri.js';

const panel = document.getElementById('console-panel');
const output = document.getElementById('console-output');
const MAX_LINES = 2000;

const GEOMETRY_KEY = 'lumineria.console.geometry';
const DEFAULT_SIZE = { width: 480, height: 320 };
const MIN_SIZE = { width: 300, height: 160 };
const MARGIN = 16;
const MIN_VISIBLE = 80;

let lines = [];
let pending = [];
let flushQueued = false;
let isPanelOpen = false;
let saveQueued = false;

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

function readGeometry() {
    try {
        const raw = localStorage.getItem(GEOMETRY_KEY);
        if (!raw) return null;
        const g = JSON.parse(raw);
        const numeros = [g.left, g.top, g.width, g.height];
        if (!numeros.every(n => typeof n === 'number' && Number.isFinite(n))) return null;
        return g;
    } catch (e) {
        console.warn('No se pudo leer la geometría de la consola:', e);
        return null;
    }
}

function saveGeometry() {
    const g = {
        left: panel.offsetLeft,
        top: panel.offsetTop,
        width: panel.offsetWidth,
        height: panel.offsetHeight,
    };
    try {
        localStorage.setItem(GEOMETRY_KEY, JSON.stringify(g));
    } catch (e) {
        console.warn('No se pudo guardar la geometría de la consola:', e);
    }
}

function queueSaveGeometry() {
    if (saveQueued) return;
    saveQueued = true;
    requestAnimationFrame(() => {
        saveQueued = false;
        saveGeometry();
    });
}

function clampIntoViewport() {
    const maxWidth = Math.max(200, window.innerWidth - MARGIN * 2);
    const maxHeight = Math.max(120, window.innerHeight - MARGIN * 2);
    const width = Math.min(panel.offsetWidth, maxWidth);
    const height = Math.min(panel.offsetHeight, maxHeight);
    panel.style.width = `${width}px`;
    panel.style.height = `${height}px`;

    const maxLeft = window.innerWidth - MIN_VISIBLE;
    const maxTop = window.innerHeight - MIN_VISIBLE;
    const left = Math.min(Math.max(panel.offsetLeft, MIN_VISIBLE - width), maxLeft);
    const top = Math.min(Math.max(panel.offsetTop, 0), maxTop);
    panel.style.left = `${left}px`;
    panel.style.top = `${top}px`;
}

function applyStoredGeometry() {
    const g = readGeometry();
    if (g) {
        panel.style.width = `${g.width}px`;
        panel.style.height = `${g.height}px`;
        panel.style.left = `${g.left}px`;
        panel.style.top = `${g.top}px`;
    } else {
        panel.style.width = `${DEFAULT_SIZE.width}px`;
        panel.style.height = `${DEFAULT_SIZE.height}px`;
        panel.style.left = `${window.innerWidth - DEFAULT_SIZE.width - MARGIN}px`;
        panel.style.top = `${window.innerHeight - DEFAULT_SIZE.height - MARGIN}px`;
    }
}

function initDragAndResize() {
    const header = panel.querySelector('.console-header');

    header?.addEventListener('pointerdown', (e) => {
        if (e.target.closest('button')) return;
        if (e.button !== 0) return;

        const startX = e.clientX;
        const startY = e.clientY;
        const startLeft = panel.offsetLeft;
        const startTop = panel.offsetTop;
        panel.classList.add('dragging');
        header.setPointerCapture(e.pointerId);

        const onMove = (ev) => {
            panel.style.left = `${startLeft + ev.clientX - startX}px`;
            panel.style.top = `${startTop + ev.clientY - startY}px`;
        };
        const onUp = () => {
            header.removeEventListener('pointermove', onMove);
            header.removeEventListener('pointerup', onUp);
            header.removeEventListener('pointercancel', onUp);
            panel.classList.remove('dragging');
            clampIntoViewport();
            saveGeometry();
        };

        header.addEventListener('pointermove', onMove);
        header.addEventListener('pointerup', onUp);
        header.addEventListener('pointercancel', onUp);
    });

    const handle = panel.querySelector('.console-resize-handle');

    handle?.addEventListener('pointerdown', (e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();

        const startX = e.clientX;
        const startY = e.clientY;
        const startWidth = panel.offsetWidth;
        const startHeight = panel.offsetHeight;
        panel.classList.add('dragging');
        handle.setPointerCapture(e.pointerId);

        const onMove = (ev) => {
            const maxWidth = window.innerWidth - panel.offsetLeft - MARGIN;
            const maxHeight = window.innerHeight - panel.offsetTop - MARGIN;
            const width = startWidth + ev.clientX - startX;
            const height = startHeight + ev.clientY - startY;
            panel.style.width = `${Math.min(Math.max(width, MIN_SIZE.width), maxWidth)}px`;
            panel.style.height = `${Math.min(Math.max(height, MIN_SIZE.height), maxHeight)}px`;
        };
        const onUp = () => {
            handle.removeEventListener('pointermove', onMove);
            handle.removeEventListener('pointerup', onUp);
            handle.removeEventListener('pointercancel', onUp);
            panel.classList.remove('dragging');
            saveGeometry();
        };

        handle.addEventListener('pointermove', onMove);
        handle.addEventListener('pointerup', onUp);
        handle.addEventListener('pointercancel', onUp);
    });

    window.addEventListener('resize', () => {
        if (!isPanelOpen) return;
        clampIntoViewport();
        queueSaveGeometry();
    });
}

export function initConsole() {
    applyStoredGeometry();
    initDragAndResize();

    document.getElementById('console-toggle-btn')?.addEventListener('click', () => {
        panel.classList.toggle('hidden');
        isPanelOpen = !panel.classList.contains('hidden');
        if (isPanelOpen) {
            clampIntoViewport();
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