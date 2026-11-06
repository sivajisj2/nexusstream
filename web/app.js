/* No framework or runtime CDN dependency. API key lives only in this page's memory. */
"use strict";
const $ = (id) => document.getElementById(id);
let apiKey = "";
let cursor = null;
let nextCursor = null;
let loading = false;
let activeRequest = 0;
function showError(e) { $("error").textContent = e.message; $("error").hidden = false; }
async function api(path, options = {}) {
  const response = await fetch(path, { ...options, headers: { "X-API-Key": apiKey, "Content-Type": "application/json", ...options.headers }, signal: AbortSignal.timeout(10000) });
  if (!response.ok) {
    let message = `Request failed (${response.status})`;
    try { const body = await response.json(); message = body.error || message; } catch { /* A proxy or extractor may return text. */ }
    throw new Error(message);
  }
  return response.status === 204 ? null : response.json();
}
function cell(row, text, className) { const td = document.createElement("td"); const node = document.createElement("span"); node.textContent = text; if(className) node.className=className; td.append(node); row.append(td); }
async function refresh() {
  if (!apiKey || loading) return;
  loading = true;
  const request = ++activeRequest;
  $("refresh").disabled = true;
  try {
    const query = new URLSearchParams({ limit: "15" });
    if(cursor) query.set("before", cursor);
    if($("source").value.trim()) query.set("source", $("source").value.trim());
    if($("kind").value.trim()) query.set("kind", $("kind").value.trim());
    const [stats, page] = await Promise.all([api("/v1/stats"), api(`/v1/events?${query}`)]);
    if(request !== activeRequest) return;
    $("error").hidden = true;
    for(const key of ["total","done","pending","dead"]) $(key).textContent = stats[key].toLocaleString();
    $("status").textContent = "● Connected";
    $("status").style.color = "#097c62";
    $("count").textContent = `${page.events.length} events on this page`;
    $("events").replaceChildren();
    if(!page.events.length) { const tr=document.createElement("tr"),td=document.createElement("td");td.colSpan=5;td.className="empty";td.textContent="No events found. Publish your first event below.";tr.append(td);$("events").append(tr); }
    for(const event of page.events) {
      const row=document.createElement("tr");
      cell(row,event.id.slice(0,8)+"…");cell(row,event.source);cell(row,event.kind,"kind");cell(row,new Date(event.received_at).toLocaleString());
      const td=document.createElement("td"),button=document.createElement("button");button.textContent="Inspect ↗";button.className="secondary";button.onclick=()=>{$("detail-content").textContent=JSON.stringify(event,null,2);$("detail").showModal();};td.append(button);row.append(td);$("events").append(row);
    }
    nextCursor=page.next_cursor;
    $("older").disabled=!nextCursor;
    $("updated").textContent=`Updated ${new Date().toLocaleTimeString()}`;
  } catch(e) { showError(e); $("status").textContent="Connection error"; }
  finally { loading=false;$("refresh").disabled=false; }
}
$("connect").onsubmit=async(e)=>{e.preventDefault();apiKey=$("key").value;$("key").value="";cursor=null;await refresh();};
$("refresh").onclick=refresh;
$("filters").onsubmit=(e)=>{e.preventDefault();if(loading)return;cursor=null;refresh();};
$("clear").onclick=()=>{if(loading)return;$("source").value="";$("kind").value="";cursor=null;refresh();};
$("older").onclick=()=>{if(loading)return;cursor=nextCursor;refresh();};
$("newest").onclick=()=>{if(loading)return;cursor=null;refresh();};
// Keep one ID on an ambiguous network failure so clicking again is idempotent.
let pendingEvent = null;
$("publish-form").onsubmit=async(e)=>{
  e.preventDefault(); if(!apiKey){showError(new Error("Connect with your instance key first."));return;}
  $("send").disabled=true;
  try {
    const payload=JSON.parse($("payload").value);
    if(!payload || Array.isArray(payload) || typeof payload!=="object")throw new Error("Payload must be a JSON object.");
    const input={source:$("pub-source").value,kind:$("pub-kind").value,payload};
    if(!pendingEvent || JSON.stringify(pendingEvent.input)!==JSON.stringify(input))pendingEvent={id:crypto.randomUUID(),input};
    const event={id:pendingEvent.id,...input};
    const receipt=await api("/v1/events",{method:"POST",body:JSON.stringify({events:[event]})});
    $("receipt").textContent=`${receipt.accepted} accepted · ${receipt.duplicates} duplicates · ${event.id}`;
    pendingEvent=null;cursor=null;await refresh();
  }catch(e){showError(e);}finally{$("send").disabled=false;}
};
$("replay-form").onsubmit=async(e)=>{e.preventDefault();try{if(!apiKey)throw new Error("Connect first.");await api(`/v1/events/${encodeURIComponent($("replay-id").value.trim())}/replay`,{method:"POST"});$("replay-result").textContent="Dead delivery requeued.";await refresh();}catch(e){showError(e);}};
$("close-detail").onclick=()=>$("detail").close();
setInterval(()=>{if(!document.hidden && !cursor)refresh();},5000);
