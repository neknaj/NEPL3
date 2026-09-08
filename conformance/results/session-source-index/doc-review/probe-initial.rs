use nepl3_tools::doc::source::{budget, compiled, err, with_input_route};
use nepl3_core::source::{SourceAdmission,SourceStore};
use nepl3_wire::foundation::FoundationCodec;
use nepl3_doc_core::{lower,check::Category};
fn main()->Result<(),String>{
 let args:Vec<_>=std::env::args().collect();
 let input=std::fs::read_to_string(&args[1]).map_err(err)?;
 let out=std::path::Path::new(&args[2]);std::fs::create_dir_all(out).map_err(err)?;
 let c=compiled()?;
 with_input_route(true,&c,&input,"Article",|tree,profile,b,_|{
  std::fs::write(out.join("tree.txt"),format!("{:#?}",tree.tree())).map_err(err)?;
  std::fs::write(out.join("profile.txt"),format!("{:?}",profile.digest())).map_err(err)?;
  std::fs::write(out.join("parse-usage.txt"),format!("{:?}",b.usage())).map_err(err)?;
  let empty=SourceStore::default();let mut a=SourceAdmission::default();
  let mut codec=FoundationCodec::new(profile.registry(),&empty,&mut a).map_err(err)?;
  let d=lower::document(tree.syntax(),&c.doc.package.schema,Category::Article,profile.registry(),&mut budget(),&mut codec).map_err(err)?;
  std::fs::write(out.join("document.txt"),format!("{d:#?}")).map_err(err)?;
  let v=nepl3_doc_core::portable::to_value(&d,profile.registry(),&mut codec,&mut budget()).map_err(err)?;
  std::fs::write(out.join("document-ndf.txt"),format!("{v:#?}")).map_err(err)?;
  Ok(())
 })
}
