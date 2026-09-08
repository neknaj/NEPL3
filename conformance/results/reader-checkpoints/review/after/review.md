# Correction addendum: 0c583b7

Final fixed commit0c583b7af13431afc2be8f78ba6a05aaa5a208a9 contains exactly two changes over92d7cd2: runtime/mod.rs and the managed checkpoints test. Snapshot bytes initially captured while root's working tree was dirty subsequently matched both committed Git blobs. delta.json records the exact hashes; the production file SHA is c6dd18e12bee362795e987817e38a0c328d2023764bab38077bbd617d00171a5. Original before evidence remains unchanged.

## Correction

resume_saved now charges Work(frame count+1) before Vec slot Allocation, checked storage allocation, Vec construction and mem::take/Frame::owned iteration. The former constant Work1 from slot is retained in this batch; n additional visits are counted. The frame count is private Vec length, bounded by nonzero Frame allocation and isize capacity, so the u64 +1 cannot wrap on the supported32/64bit targets. Insufficient Work exits before consuming c.frames or altering c.current; the existing stopped path retains accepted collector data. Allocation or subsequent stops still retain that same accepted closure.

The first intermediate proposal of charging Work(n) and subsequently calling slot::<Vec<Frame>> would also have satisfied precharge: a stop after n was charged but before the slot can make adjacent-cap work differences appear1. That proposal was not independently built or classified as a product failure. The final batching makes a useful observable regression but is not the only correct accounting order. Do not generalize largest adjacent Allocation jump / Work jump into a universal proof of precharge. The direct code-order check, actual unchanged-source before/after total cost, typed stops and semantic regressions jointly support the conclusion.

## Unchanged original probe comparison

review_frame_work.rs is byte-identical between before-final and after. Public Call nested in32/128 Discard frames, same Budget Work consumption/sweep, same source and plan:

| Target/depth | Full resume Work before→after | Allocation unchanged | First vector allocation cap before→after | Current adjacent successful Work difference |
|---|---:|---:|---:|---:|
| native32 | 621→653 | 66,859 | 588→620 | 33 |
| native128 | 1965→2093 | 244,267 | 1836→1964 | 129 |
| wasm32 32 | 621→653 | 44,471 | 588→620 | 33 |
| wasm32 128 | 1965→2093 | 161,591 | 1836→1964 | 129 |

Thus full resume and allocation-entry Work increase by exactly n, with no allocation change. The windows previously allowing full vector conversion under one additional Work now pay n+1 before allocation. The regression observes expected WorkLimit outcomes rather than pretending all swept caps complete.

Full fixed reader package tests were rerun independently: managed59 plus independent semantic2 and Work-window1, total62 per native/WASI target. Managed includes39 runtime,17 builtin,1 context and2 portable tests. The42 semantic routes,10 echo mutations before normal retry and20 typed resource stops per target remain successful. The growing-view allocation improvement and original accepted sources/maps/Report closure remain unchanged. Managed Transform tests execute actual NDF and CBOR conversion, not just native values.

No additional blocking issue remains in this bounded checkpoint slice. This is not a universal performance claim, whole Doc acceptance, arbitrary usize::MAX allocation test, proof of a newly implemented generic continuation wire adapter, or claim that root's large chapter Work/Output failures completed. Production and docs were only edited by root.
