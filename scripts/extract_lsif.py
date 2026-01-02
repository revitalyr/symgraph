#!/usr/bin/env python3
import json, urllib.parse, sys
infile='compile_commands.json'
out_json='sources.json'
out_txt='sources.txt'
uris=[]
try:
    with open(infile, 'r', encoding='utf-8') as f:
        for line in f:
            line=line.strip()
            if not line: continue
            try:
                obj=json.loads(line)
            except Exception:
                continue
            if obj.get('type')=='vertex' and obj.get('label')=='document':
                uri=obj.get('uri')
                if uri:
                    if uri.startswith('file://'):
                        u=uri[len('file://'):]
                        if u.startswith('/') and len(u)>=3 and u[2]==':':
                            u=u[1:]
                        path=urllib.parse.unquote(u)
                    else:
                        path=uri
                    uris.append(path)
    with open(out_json,'w',encoding='utf-8') as f:
        json.dump(uris,f,indent=2,ensure_ascii=False)
    with open(out_txt,'w',encoding='utf-8') as f:
        for p in uris: f.write(p+"\n")
    print(f"written {len(uris)} sources to {out_json} and {out_txt}")
except FileNotFoundError:
    print(f"Input file {infile} not found", file=sys.stderr)
    sys.exit(1)
