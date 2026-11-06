#!/usr/bin/env python3
"""Small reproducible workload; reports measured latency, never invented throughput."""
import concurrent.futures,json,os,time,urllib.request,uuid,statistics,argparse
p=argparse.ArgumentParser();p.add_argument("--events",type=int,default=1000);p.add_argument("--concurrency",type=int,default=8);a=p.parse_args()
def send(_):
    body={"events":[{"id":str(uuid.uuid4()),"source":"loadtest","kind":"benchmark.event","payload":{"value":42}}]}
    req=urllib.request.Request(os.environ.get("NEXUS_URL","http://127.0.0.1:8080")+"/v1/events",data=json.dumps(body).encode(),headers={"Content-Type":"application/json","X-API-Key":os.environ["NEXUS_API_KEY"]})
    start=time.perf_counter()
    with urllib.request.urlopen(req,timeout=30) as r: assert r.status==202
    return (time.perf_counter()-start)*1000
start=time.perf_counter()
with concurrent.futures.ThreadPoolExecutor(max_workers=a.concurrency) as pool:latencies=sorted(pool.map(send,range(a.events)))
elapsed=time.perf_counter()-start
print(json.dumps({"events":a.events,"concurrency":a.concurrency,"elapsed_seconds":round(elapsed,3),"events_per_second":round(a.events/elapsed,2),"p50_ms":round(statistics.median(latencies),2),"p95_ms":round(latencies[min(len(latencies)-1,int(len(latencies)*.95))],2)},indent=2))
