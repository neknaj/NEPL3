from pathlib import Path
import shutil
r=Path(__file__).resolve().parent;dest=r/'probe';shutil.copytree(r/'source',dest,dirs_exist_ok=True)
p=dest/'crates/foundation/reader/src/lib.rs';p.write_bytes(p.read_bytes()+b'\n#[cfg(test)] extern crate std;\n#[cfg(test)] extern crate self as nepl3_reader;\n')
p=dest/'crates/foundation/reader/src/tokenizer/session.rs';p.write_bytes(p.read_bytes()+b'\n#[cfg(test)] mod independent_review;\n')
original=(dest/'crates/foundation/reader/tests/runtime.rs').read_text(encoding='utf-8');helpers=[]
for name in ['budget','reader_type','registry','plan','context','check_context','source','signature','provider_plan','terminal','annotated_terminal']:
 start=original.index('\nfn '+name+'(')+1 if '\nfn '+name+'(' in original else original.index('\nfn '+name+'<')+1
 brace=original.index('{',start);depth=1;end=brace+1
 while depth:depth+=(original[end]=='{')-(original[end]=='}');end+=1
 helpers.append(original[start:end])
prefix='use super::*;\nuse alloc::{vec,vec::Vec,format,string::String,boxed::Box};\nuse std::println;\nuse nepl3_core::{budget::*,diagnostic::*,schema::*,source::*,syntax::*,value::*,view::*};\nuse crate::{model::*,plan::*,runtime::*};\n'
(dest/'crates/foundation/reader/src/tokenizer/session/independent_review.rs').write_text(prefix+'\n'.join(helpers)+'\n'+(r/'extra.rs').read_text(),encoding='utf-8',newline='\n')
