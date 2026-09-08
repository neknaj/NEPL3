use nepl3_tools::doc::{source,export::pages::{self,Entry,resources::PhaseLimits}};
use nepl3_core::budget::{Budget,Resource};
#[test]
fn independent_two_page_phase_and_sticky_output() -> Result<(),String> {
 let c=source::compiled()?;
 let inputs:Vec<_>=(0..2).map(|n|(Entry{id:format!("p{n}"),source:format!("p{n}.md"),route:format!("p{n}/index.html"),input:None},"article ja \"Title\" body cons paragraph cons \"abc\" nil nil".into())).collect();
 let first=pages::generate(&c,&inputs)?;
 let m:serde_json::Value=serde_json::from_str(&first.manifest).map_err(|e|e.to_string())?;
 let mut phases=PhaseLimits::default();
 phases.lower.work=m["pages"].as_array().unwrap().iter().map(|p|p["lower_usage"]["work"].as_u64().unwrap()).max().unwrap();
 let mut output=source::budget();output.charge(Resource::Work,17).map_err(|e|format!("{e:?}"))?;
 let successful=pages::generate_with_phase_limits(&c,&inputs,phases,&mut output)?;
 assert_eq!(successful.files,first.files);
 let result:serde_json::Value=serde_json::from_str(&successful.manifest).unwrap();
 assert_eq!(result["output_budget"]["initial_usage"]["work"],17);
 assert_eq!(result["pages"][0]["lower_initial_usage"]["work"],0);
 assert_eq!(result["pages"][1]["lower_initial_usage"]["work"],0);
 phases.lower.work-=1;
 let mut untouched=source::budget();let before=untouched.usage();
 assert!(pages::generate_with_phase_limits(&c,&inputs,phases,&mut untouched).unwrap_err_or_string().contains("WorkLimit"));
 assert_eq!(before,untouched.usage());
 let mut cancelled=source::budget();cancelled.cancel();let before=cancelled.usage();
 assert!(pages::generate_with_phase_limits(&c,&inputs,PhaseLimits::default(),&mut cancelled).is_err());assert!(cancelled.poll().is_err());assert_eq!(before,cancelled.usage());
 let mut limits=source::budget().limits();limits.work=0;let mut stopped=Budget::new(limits);assert!(stopped.charge(Resource::Work,1).is_err());
 assert!(pages::generate_with_phase_limits(&c,&inputs,PhaseLimits::default(),&mut stopped).is_err());assert!(stopped.poll().is_err());
 Ok(())
}
trait ErrorString{fn unwrap_err_or_string(self)->String;}
impl<T> ErrorString for Result<T,String>{fn unwrap_err_or_string(self)->String{match self{Err(e)=>e,Ok(_)=>panic!("unexpected success")}}}
