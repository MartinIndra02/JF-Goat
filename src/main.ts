import App from "./App.svelte";
import { mount } from "svelte";
import "./styles.css";

const target = document.getElementById("app");

if (!target) {
  throw new Error("Missing #app root element");
}

let app: ReturnType<typeof mount>;

try {
  app = mount(App, {
    target,
  });
} catch (error) {
  console.error("Fatal startup error:", error);

  const message = error instanceof Error ? error.message : String(error);
  target.innerHTML = `
    <main style="min-height:100vh;display:flex;align-items:center;justify-content:center;background:#0b1224;color:#e5e7eb;padding:24px;font-family:Segoe UI,Arial,sans-serif;">
      <section style="max-width:640px;width:100%;background:#111827;border:1px solid #374151;border-radius:12px;padding:20px;box-shadow:0 10px 30px rgba(0,0,0,0.35);">
        <h1 style="margin:0 0 8px;font-size:20px;color:#f9fafb;">jfgoat failed to start</h1>
        <p style="margin:0 0 12px;color:#cbd5e1;">The app hit a fatal startup error and rendered this fallback instead of a blank window.</p>
        <pre style="margin:0;background:#0f172a;border:1px solid #334155;border-radius:8px;padding:12px;white-space:pre-wrap;word-break:break-word;color:#fca5a5;">${message}</pre>
      </section>
    </main>
  `;
}

export default app;
