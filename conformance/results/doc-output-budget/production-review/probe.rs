
#[test]
fn reviewer_default_manifest() -> Result<(),String> {
 use nepl3_tools::doc::export::pages::{self,Entry};
 let c=compiled()?;
 let input=[("one", "First."),("two","Second.")].map(|(id,body)|(Entry{id:id.into(),source:format!("{id}.nepld"),route:format!("{id}/index.html")},format!("article en \"Title\" body cons paragraph cons \"{body}\" nil nil")));
 let result=pages::generate(&c,&input)?;
 std::fs::write("../observed-manifest.json",&result.manifest).map_err(super::err)?;
 std::fs::write("../observed-files.json",serde_json::to_vec(&result.files).map_err(super::err)?).map_err(super::err)?;
 Ok(())
}
