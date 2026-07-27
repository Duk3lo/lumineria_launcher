export function setSafeBackgroundImage(el, url, fallback = 'assets/logo.png') {
    try {
        const parsed = new URL(url, window.location.href);
        if (!['http:', 'https:', 'asset:', 'file:'].includes(parsed.protocol)) {
            throw new Error('esquema no permitido');
        }
        el.style.backgroundImage = `url("${CSS.escape(parsed.href)}")`;
    } catch {
        el.style.backgroundImage = `url("${fallback}")`;
    }
}