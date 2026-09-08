from pathlib import Path
r=Path(__file__).parent
for name in ['workspace','before']:
 p=r/name/'tools/tests/doc/html.rs'
 s=p.read_text(encoding='utf-8')
 old='nepl3_markup::html::serialize(&checked, &mut operation).map_err(err)'
 new='''let output = nepl3_markup::html::serialize(&checked, &mut operation).map_err(err)?;
        eprintln!("REVIEW_HTML {:?} bytes={}", operation.usage(), output.len());
        std::fs::write("../%s.html", &output).map_err(err)?;
        Ok(output)''' % name
 assert s.count(old)==1;s=s.replace(old,new);p.write_text(s,encoding='utf-8',newline='\n')
