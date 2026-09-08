from pathlib import Path
import shutil
r=Path(__file__).resolve().parent;dest=r/'probe';shutil.copytree(r/'source',dest,dirs_exist_ok=True)
original=(dest/'crates/foundation/reader/tests/runtime.rs').read_text(encoding='utf-8');helpers=[]
for name in ['budget','reader_type','registry','plan','context','check_context','source','signature','provider_plan','terminal','annotated_terminal']:
 start=original.index('\nfn '+name+'(')+1 if '\nfn '+name+'(' in original else original.index('\nfn '+name+'<')+1
 brace=original.index('{',start);depth=1;end=brace+1
 while depth:depth+=(original[end]=='{')-(original[end]=='}');end+=1
 helpers.append(original[start:end])
prefix='use nepl3_core::{budget::*,diagnostic::*,schema::*,source::*,syntax::*,value::*,view::*,origin::{Mapping,MappingKind}};\nuse nepl3_reader::{model::*,plan::*,runtime::*,tokenizer::*,builtin::BuiltinReader};\n'
(dest/'crates/foundation/reader/tests/independent_recover.rs').write_text(prefix+'\n'.join(helpers)+'\n'+(r/'extra.rs').read_text(),encoding='utf-8',newline='\n')
p=r/'run.py';s=p.read_text();s=s.replace("cmd+=['--lib','independent_review::independent_']","cmd+=['--test','independent_recover']");p.write_text(s,encoding='utf-8',newline='\n')
