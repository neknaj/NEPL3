use super::*;
#[test]
fn equivalent_source_generation_with_different_allocation_order_has_identical_usage(){
 fn make(reverse:bool)->Vec<SourceSnapshot>{let mut slots:Vec<Option<SourceSnapshot>>=(0..64).map(|_|None).collect();for j in 0..64{let i=if reverse{63-j}else{j};slots[i]=Some(source(&format!("s-{i:03}"),&format!("memory:s-{i:03}"),"abc"));}slots.into_iter().map(|v|v.expect("all slots")).collect()}
 fn run(snapshots:&[SourceSnapshot])->Usage{let mut ledger=SourceAdmission::default();let mut b=budget();for source in snapshots{ledger.admit_existing(source,&mut b).expect("insert");}for i in (0..64).rev(){ledger.admit_existing(&snapshots[i],&mut b).expect("reverse hit");}for j in 0..64{let i=(j*17)%64;ledger.admit_existing(&snapshots[i],&mut b).expect("permuted hit");}b.usage()}
 let forward=make(false);let reverse=make(true);assert_eq!(forward,reverse);let a=run(&forward);let b=run(&reverse);println!("forward allocation: {a:?}; reverse allocation: {b:?}");assert_eq!(a,b,"allocation addresses must not determine logical operation usage");
}
