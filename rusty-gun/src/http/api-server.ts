import { GunDB } from "../core/database.ts";
import { loadConfig, saveConfig } from "../config.ts";

export interface ApiServerHandle {
  url: string;
  close: () => void;
}

export function startApiServer(opts: { port: number; db: GunDB }): ApiServerHandle {
  const { port, db } = opts;
  const STATIC_ROOT = "web/dist";

  const handler = async (req: Request): Promise<Response> => {
    try {
      const url = new URL(req.url);
      const path = url.pathname;
      if (path === "/api/events") {
        const stream = new ReadableStream({
          start(controller) {
            const enc = new TextEncoder();
            const send = (evt: { id: string; node: unknown | null }) => {
              const line = `data: ${JSON.stringify(evt)}\n\n`;
              controller.enqueue(enc.encode(line));
            };
            const cb = (e: { id: string; node: unknown | null }) => send(e);
            db.onAny(cb as any);
            // Send initial list snapshot
            (async () => {
              for await (const n of db.list()) send({ id: n.id, node: { id: n.id, data: n.data } });
            })();
            return () => db.offAny(cb as any);
          },
        });
        return new Response(stream, { headers: { "content-type": "text/event-stream" } });
      }
      if (path.startsWith("/api/")) {
        switch (path) {
          case "/api/config": {
            if (req.method === "GET") {
              const cfg = await loadConfig();
              return json(cfg);
            }
            if (req.method === "POST") {
              const body = await req.json().catch(() => null) as Record<string, unknown> | null;
              if (!body) return json({ error: "missing body" }, 400);
              // Merge shallow
              const current = await loadConfig();
              const next = { ...current, ...body } as any;
              await saveConfig(next);
              return json({ ok: true });
            }
            return json({ error: "method" }, 405);
          }
          case "/api/get": {
            const id = url.searchParams.get("id");
            if (!id) return json({ error: "missing id" }, 400);
            const val = await db.get<Record<string, unknown>>(id);
            return json(val);
          }
          case "/api/put": {
            if (req.method !== "POST") return json({ error: "method" }, 405);
            const body = await req.json().catch(() => null) as { id?: string; data?: Record<string, unknown> } | null;
            if (!body?.id || !body?.data) return json({ error: "missing body {id,data}" }, 400);
            await db.put(body.id, body.data);
            return json({ ok: true });
          }
          case "/api/delete": {
            const id = url.searchParams.get("id");
            if (!id) return json({ error: "missing id" }, 400);
            await db.delete(id);
            return json({ ok: true });
          }
          case "/api/search": {
            const q = url.searchParams.get("q") ?? "";
            const k = Number(url.searchParams.get("k") ?? "5");
            const nodes = await db.vectorSearch(q, Number.isFinite(k) ? k : 5);
            return json(nodes.map((n) => ({ id: n.id, data: n.data })));
          }
          case "/api/list": {
            const out: Array<{ id: string; data: Record<string, unknown> }> = [];
            for await (const n of db.list()) out.push({ id: n.id, data: n.data as Record<string, unknown> });
            return json(out);
          }
          case "/api/instances": {
            const typeName = url.searchParams.get("type");
            if (!typeName) return json({ error: "missing type" }, 400);
            const nodes = await db.instancesOf(typeName);
            return json(nodes.map((n) => ({ id: n.id, data: n.data })));
          }
          case "/api/history": {
            const id = url.searchParams.get("id");
            if (!id) return json({ error: "missing id" }, 400);
            const history = await db.getNodeHistory(id);
            return json(history.map((h) => ({ 
              id: h.id, 
              data: h.data, 
              timestamp: h.timestamp,
              vectorClock: h.vectorClock,
              state: h.state
            })));
          }
          case "/api/restore": {
            const id = url.searchParams.get("id");
            const timestamp = url.searchParams.get("timestamp");
            if (!id || !timestamp) return json({ error: "missing id or timestamp" }, 400);
            await db.restoreNodeVersion(id, parseInt(timestamp));
            return json({ success: true });
          }
          default:
            return json({ error: "not found" }, 404);
        }
      }
      // Static UI
      if (req.method === "GET") {
        // Map URL to local file under web/dist
        const mapPath = path === "/" ? "/index.html" : path;
        const filePath = `${STATIC_ROOT}${mapPath}`;
        try {
          const data = await Deno.readFile(filePath);
          const contentType = mapPath.endsWith(".html") ? "text/html; charset=utf-8" :
                             mapPath.endsWith(".js") ? "application/javascript" :
                             mapPath.endsWith(".css") ? "text/css" :
                             mapPath.endsWith(".json") ? "application/json" :
                             mapPath.endsWith(".svg") ? "image/svg+xml" :
                             mapPath.endsWith(".png") ? "image/png" :
                             "application/octet-stream";
          return new Response(data, { headers: { "content-type": contentType } });
        } catch {
          // fallback to inline minimal UI
          if (path === "/" || path === "/index.html") {
            return new Response(INDEX_HTML, { headers: { "content-type": "text/html; charset=utf-8" } });
          }
        }
      }
      return new Response("Not Found", { status: 404 });
    } catch (e) {
      const msg = (e && typeof e === 'object' && 'message' in e) ? String((e as any).message) : String(e);
      return json({ error: msg }, 500);
    }
  };

  const server = Deno.serve({ port, onListen: () => {} }, handler);
  return {
    url: `http://localhost:${port}`,
    close: () => { try { server.shutdown(); } catch { /* ignore */ } },
  };
}

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), { status, headers: { "content-type": "application/json" } });
}

const INDEX_HTML = `<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Rusty Gun</title>
  <style>
  body { font-family: system-ui, sans-serif; margin: 24px; }
  h1 { margin-top: 0; }
  section { border: 1px solid #ddd; padding: 12px; margin-bottom: 16px; border-radius: 8px; }
  input, textarea, button, select { font: inherit; }
  code { background:#f5f5f5; padding:2px 4px; border-radius:4px; }
  pre { background:#f9f9f9; padding:8px; border-radius:6px; overflow:auto; }
  label { display:block; margin:6px 0 4px; }
  </style>
 </head>
 <body>
  <h1>Rusty Gun</h1>
  <p>Reactive UI. Data updates automatically.</p>
  <div style="display:flex; gap:16px; align-items:flex-start;">
    <section style="flex:1; min-width:300px;">
      <h3>Nodes</h3>
      <input id="filter" placeholder="filter by id..." oninput="render()" />
      <ul id="list" style="list-style:none; padding-left:0; max-height:60vh; overflow:auto;"></ul>
      <button onclick="createNode()">Create node</button>
    </section>
    <section style="flex:2;">
      <h3>Details</h3>
      <div id="detail-empty">Select a node on the left</div>
      <div id="detail" style="display:none;">
        <label>Id</label>
        <input id="d-id" disabled />
        <label>JSON data</label>
        <textarea id="d-json" rows="12" oninput="debouncedSave()"></textarea>
        <div style="margin-top:8px; display:flex; gap:8px;">
          <button onclick="delSelected()">Delete</button>
        </div>
      </div>
      <h3>Vector search</h3>
      <input id="q" placeholder="search text" oninput="debouncedSearch()" />
      <pre id="search-out"></pre>
    </section>
  </div>
  <script>
    const state = { items: new Map(), selected: null, timer: null, stimer: null };
    const ev = new EventSource('/api/events');
    ev.onmessage = (e) => {
      const evt = JSON.parse(e.data);
      if(evt.node){ state.items.set(evt.id, evt.node.data); } else { state.items.delete(evt.id); }
      if(state.selected === evt.id && evt.node) {
        // Avoid clobber when user is actively editing: basic heuristic
        const el = document.getElementById('d-json');
        if(document.activeElement !== el) el.value = JSON.stringify(evt.node.data,null,2);
      }
      render();
    };
    function render(){
      const list = document.getElementById('list');
      const f = (document.getElementById('filter').value||'').toLowerCase();
      list.innerHTML = '';
      const ids = Array.from(state.items.keys()).filter(id=>id.toLowerCase().includes(f)).sort();
      for(const id of ids){
        const li = document.createElement('li');
        li.textContent = id;
        li.style.cursor='pointer';
        li.style.padding='4px 6px';
        if(id===state.selected) { li.style.background='#eef'; }
        li.onclick=()=>select(id);
        list.appendChild(li);
      }
      const hasSel = !!state.selected;
      document.getElementById('detail-empty').style.display = hasSel? 'none':'block';
      document.getElementById('detail').style.display = hasSel? 'block':'none';
    }
    async function select(id){
      state.selected = id;
      document.getElementById('d-id').value = id;
      const data = state.items.get(id) ?? {};
      document.getElementById('d-json').value = JSON.stringify(data,null,2);
      render();
    }
    function debouncedSave(){
      clearTimeout(state.timer);
      state.timer = setTimeout(saveNow, 350);
    }
    async function saveNow(){
      const id = state.selected; if(!id) return;
      let data; try{ data = JSON.parse(document.getElementById('d-json').value); }catch{ return }
      await fetch('/api/put', { method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify({id, data}) });
    }
    async function delSelected(){
      const id = state.selected; if(!id) return;
      await fetch('/api/delete?id='+encodeURIComponent(id));
      state.selected = null; render();
    }
    async function createNode(){
      const id = prompt('New node id:'); if(!id) return;
      await fetch('/api/put', { method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify({id, data:{}}) });
      select(id);
    }
    function debouncedSearch(){ clearTimeout(state.stimer); state.stimer = setTimeout(searchNow,300); }
    async function searchNow(){
      const q = document.getElementById('q').value || '';
      const res = await fetch('/api/search?q='+encodeURIComponent(q)+'&k=5');
      const j = await res.json();
      document.getElementById('search-out').textContent = JSON.stringify(j,null,2);
    }
  </script>
 </body>
 </html>`;


