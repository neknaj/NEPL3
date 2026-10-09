// Observe the entire warning channel despite targeted stderr suppression.
// This is host bootstrap policy, not a promise that vm is a security sandbox.
const EXPECTED='VM Modules is an experimental feature and might change at any time';
export function observeWarnings() {
  let expected=0,unexpected=false;
  const observe=warning=>{
    if(warning.name==='ExperimentalWarning'&&warning.message===EXPECTED&&expected===0){expected++;return;}
    unexpected=true;
  };
  process.on('warning',observe);
  return {snapshot:()=>({expected,unexpected})};
}
