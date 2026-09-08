import hashlib,json,pathlib
p=pathlib.Path(__file__).resolve().parent
def head(major,n):
    if n<24:return bytes([major*32+n])
    size=next(s for s in [1,2,4,8] if n<1<<(s*8))
    return bytes([major*32+{1:24,2:25,4:26,8:27}[size]])+n.to_bytes(size,'big')
rows=[]
def add(name,expr,cbor):
    rows.append(dict(name=name,expression=expr,cbor_hex=cbor.hex(),hashes=[hashlib.sha256(d+cbor).hexdigest() for d in [b'',b'probe\0',bytes([0,255,128,65])]]))
add('unit','NdfValue::Unit',b'\x81\x00')
add('false','NdfValue::Bool(false)',b'\x82\x01\xf4')
add('true','NdfValue::Bool(true)',b'\x82\x01\xf5')
for n in [0,23,24,255,256,65535,65536,4294967295,4294967296,18446744073709551615]:add('u64-'+str(n),f'NdfValue::U64({n})',b'\x82\x02'+head(0,n))
for n in [0,1,23,24,55,56,63,64,65,255,256,65535,65536,100000]:
    add('text-'+str(n),f'NdfValue::Text("x".repeat({n}))',b'\x82\x05'+head(3,n)+b'x'*n)
    add('bytes-'+str(n),f'NdfValue::Bytes(vec![0xff; {n}])',b'\x82\x06'+head(2,n)+b'\xff'*n)
add('unicode','NdfValue::Text(String::from("文😀\\0\\r\\n"))',b'\x82\x05'+head(3,10)+'文😀\0\r\n'.encode())
for negative in [False,True]:
    for n in [1,24,256,1024]:
        mag=b'\x80'+bytes(n-1)
        add(f'integer-{negative}-{n}',f'NdfValue::Integer(Integer::from_canonical({str(negative).lower()}, &magnitude({n})).map_err(err)?)',b'\x83\x03'+(b'\xf5' if negative else b'\xf4')+head(2,n)+mag)
add('integer-zero','NdfValue::Integer(Integer::from(0_i64))',bytes.fromhex('8303f440'))
for n in [1,24,256,1024]:
    add(f'rational-{n}',f'NdfValue::Rational(Rational::from_canonical(Integer::from(-1_i64), &magnitude({n})).map_err(err)?)',bytes.fromhex('83048303f54101')+head(2,n)+b'\x80'+bytes(n-1))
add('none','NdfValue::None',bytes.fromhex('8108'))
add('some','NdfValue::Some(Box::new(NdfValue::Bool(true)))',bytes.fromhex('82098201f5'))
add('list','NdfValue::List(vec![NdfValue::Unit, NdfValue::None])',bytes.fromhex('82078281008108'))
schema=bytes.fromhex('836170025820')+bytes([7])*32
add('record','NdfValue::Record(Record {schema: schema(), kind: "K".into(), fields: vec![NdfValue::U64(24)]})',bytes.fromhex('840a')+schema+bytes.fromhex('614b8182021818'))
add('variant','NdfValue::Variant(Variant {schema: schema(), type_name: "T".into(), variant: "V".into(), fields: vec![NdfValue::Unit]})',bytes.fromhex('850b')+schema+bytes.fromhex('61546156818100'))
(p/'vectors.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
rust='fn vectors() -> Result<Vec<(&\'static str, NdfValue, Vec<u8>, [&\'static str; 3])>, String> { Ok(vec![\n'
for row in rows:
    rust+=f'({json.dumps(row["name"])}, {row["expression"]}, unhex("{row["cbor_hex"]}")?, [{", ".join(json.dumps(x) for x in row["hashes"])}]),\n'
rust+=']) }\n'
(p/'vectors.rs').write_text(rust,encoding='utf-8',newline='\n')
print(len(rows))
