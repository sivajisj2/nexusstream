#!/usr/bin/env python3
"""Black-box acceptance checks against a running instance; standard library only."""
import json, os, time, uuid, urllib.request, urllib.error
base=os.environ.get("NEXUS_URL","http://127.0.0.1:8080")
key=os.environ["NEXUS_API_KEY"]
def request(path,body=None,auth=True,method=None):
    headers={"Content-Type":"application/json"}
    if auth: headers["X-API-Key"]=key
    req=urllib.request.Request(base+path,data=None if body is None else json.dumps(body).encode(),headers=headers,method=method)
    try:
        with urllib.request.urlopen(req,timeout=10) as r:return r.status,r.read()
    except urllib.error.HTTPError as e:return e.code,e.read()
assert request("/healthz",auth=False)[0]==200
assert request("/readyz",auth=False)[0]==200
assert request("/v1/events",auth=False)[0]==401
assert request("/api-docs/openapi.json",auth=False)[0]==200
assert request("/docs/",auth=False)[0]==200
assert request("/v1/events",{"events":[]})[0]==400
event={"id":str(uuid.uuid4()),"source":"smoke","kind":"check.created","payload":{"test":True}}
status,body=request("/v1/events",{"events":[event]});assert status==202,(status,body)
assert json.loads(body)["accepted"]==1
assert json.loads(request("/v1/events",{"events":[event]})[1])["duplicates"]==1
assert request("/v1/events/"+event["id"])[0]==200
changed=dict(event,payload={"changed":True})
assert request("/v1/events",{"events":[changed]})[0]==409
for _ in range(30):
    stats=json.loads(request("/v1/stats")[1])
    if stats["pending"]==0:break
    time.sleep(.2)
assert stats["done"]>=1,stats
assert request("/metrics")[0]==200
print("PASS: health, readiness, auth, docs, validation, ingestion, deduplication, lookup, conflict, worker, metrics")
