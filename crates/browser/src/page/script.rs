pub(crate) const MOVED: &str = "(() => {
        const moved = () => {
            const bridge = window.ipc ?? window.webkit?.messageHandlers?.ipc ?? window.chrome?.webview;
            bridge?.postMessage('moved');
        };
        for (const name of ['pushState', 'replaceState']) {
            const original = history[name];
            history[name] = function (...args) {
                const result = original.apply(this, args);
                moved();
                return result;
            };
        }
        addEventListener('popstate', moved);
        addEventListener('hashchange', moved);
    })();";

pub(crate) const CONSOLE: &str = "(() => {
        const handler = window.webkit?.messageHandlers?.ipc ?? window.chrome?.webview;
        if (!handler) return;
        const text = (value) => {
            if (typeof value === 'string') return value;
            if (value instanceof Error) return value.stack || String(value);
            try {
                return JSON.stringify(value) ?? String(value);
            } catch {
                return String(value);
            }
        };
        const send = (level, values) => {
            try {
                handler.postMessage(JSON.stringify({ level, text: values.map(text).join(' ') }));
            } catch {}
        };
        for (const level of ['debug', 'log', 'info', 'warn', 'error']) {
            const original = console[level];
            console[level] = function (...values) {
                send(level, values);
                return original.apply(this, values);
            };
        }
        addEventListener('error', (event) => send('error', [event.error ?? event.message]));
        addEventListener('unhandledrejection', (event) =>
            send('error', ['Uncaught (in promise)', event.reason]));
    })();";
