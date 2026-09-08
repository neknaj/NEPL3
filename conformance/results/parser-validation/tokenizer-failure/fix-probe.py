from pathlib import Path
r=Path(__file__).resolve().parent;p=r/'extra.rs';s=p.read_text().replace('let bad=NdfValue::Text("bad".into());','let bad=NdfValue::Text("bad".into());let unit=NdfValue::Unit;').replace('state:if mode==3{&bad}else{&NdfValue::Unit}','state:if mode==3{&bad}else{&unit}');p.write_text(s,encoding='utf-8')
