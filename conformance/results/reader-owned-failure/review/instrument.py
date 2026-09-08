from pathlib import Path
import shutil,json,hashlib
r=Path(__file__).resolve().parent
# Separate test-only crate copy exposes no production API; original frozen source remains untouched.
dest=r/'probe';shutil.copytree(r/'source',dest,dirs_exist_ok=True)
lib=dest/'crates/foundation/reader/src/lib.rs';lib.write_bytes(lib.read_bytes()+b'\n#[cfg(test)] extern crate std;\n#[cfg(test)] extern crate self as nepl3_reader;\n#[cfg(test)] #[path="../tests/review_owned.rs"] mod review_owned;\n')
original=(dest/'crates/foundation/reader/tests/runtime.rs').read_text(encoding='utf-8')
helpers=[]
for name in ['budget','reader_type','registry','plan','context','check_context','source','signature','provider_plan','terminal','annotated_terminal']:
 start=original.index('\nfn '+name+'(')+1 if '\nfn '+name+'(' in original else original.index('\nfn '+name+'<')+1
 brace=original.index('{',start);depth=1;end=brace+1
 while depth:
  depth+=(original[end]=='{')-(original[end]=='}');end+=1
 helpers.append(original[start:end])
prefix='use alloc::{vec,vec::Vec,format,string::String,boxed::Box};\nuse std::println;\nuse nepl3_core::{budget::*,diagnostic::*,schema::*,source::*,syntax::*,value::*,view::*};\nuse crate::{model::*,plan::*,runtime::*};\n'
test=dest/'crates/foundation/reader/tests/review_owned.rs';test.write_text(prefix+'\n'.join(helpers)+'\n'+(r/'extra.rs').read_text(),encoding='utf-8',newline='\n')
print('test-only instrumented crate',dest)
