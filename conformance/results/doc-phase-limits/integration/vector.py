import hashlib,struct,pathlib,json
record=bytes([0x22])*32+struct.pack('>8Q',*range(1,9))+bytes(64)+struct.pack('>8Q',*range(9,17))+bytes(64)
record2=bytes([0x33])*32+record[32:]
payload=b'nepl3.local-doc-pages.phases/1\0'+bytes([0x11])*32+struct.pack('>Q',2)+record+record2
value={'sha256':hashlib.sha256(payload).hexdigest(),'bytes':len(payload),'hex':payload.hex()}
pathlib.Path(__file__).with_suffix('.json').write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8')
print(value['sha256'],value['bytes'])
