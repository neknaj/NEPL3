fn err(e:impl std::fmt::Debug)->String{format!("{e:?}")}
fn check_shell_depth(
    fragment: &nepl3_markup::html::HtmlFragment,
    budget: &mut nepl3_core::budget::Budget,
) -> Result<(), String> {
    use nepl3_core::budget::Resource;
    use nepl3_markup::html::HtmlNode;
    let mut pending = Vec::new();
    push_pending(&mut pending, (fragment.root, 3), budget)?;
    while let Some((node, depth)) = pending.pop() {
        budget.charge(Resource::Work, 1).map_err(err)?;
        if depth > 256 {
            return Err(format!(
                "OutputDepth {{ element: {node}, shell_depth: {depth} }}"
            ));
        }
        budget.observe_depth(depth).map_err(err)?;
        if let HtmlNode::Element { children, .. } = &fragment.nodes[node as usize] {
            for child in children.iter().rev() {
                push_pending(&mut pending, (*child, depth + 1), budget)?;
            }
        }
    }
    Ok(())
}

fn push_pending(
    pending: &mut Vec<(u64, u64)>,
    value: (u64, u64),
    budget: &mut nepl3_core::budget::Budget,
) -> Result<(), String> {
    use nepl3_core::budget::{Resource, StopReason};
    budget.charge(Resource::Work, 1).map_err(err)?;
    if pending.len() == pending.capacity() {
        let size = std::mem::size_of::<(u64, u64)>();
        let Some((capacity, bytes)) = pending
            .capacity()
            .checked_mul(2)
            .map(|v| v.max(1))
            .and_then(|c| c.checked_mul(size).map(|bytes| (c, bytes)))
        else {
            return Err(err(budget.stop(StopReason::AllocationLimit)));
        };
        budget
            .charge(Resource::AllocationUnits, bytes as u64)
            .map_err(err)?;
        pending.reserve_exact(capacity - pending.len());
    }
    pending.push(value);
    Ok(())
}

fn budget()->nepl3_core::budget::Budget{use nepl3_core::budget::*;Budget::new(Limits{work:100000,allocation_units:1000000,output_bytes:1000,source_bytes:1000,nodes:1000,depth:1000,diagnostics:100,events:100})}
fn main(){use nepl3_core::budget::*;let mut p=Vec::new();let mut b=budget();let mut charged=0;
for i in 0..1000{let previous=p.capacity();push_pending(&mut p,(i,3),&mut b).unwrap();if p.capacity()!=previous{charged+=p.capacity()as u64*std::mem::size_of::<(u64,u64)>()as u64;}assert_eq!(b.usage().allocation_units,charged);assert_eq!(b.usage().work,i+1);}
for cap in [0,15,16]{let mut l=budget().limits();l.allocation_units=cap;let mut b=Budget::new(l);let mut p=Vec::new();let r=push_pending(&mut p,(0,3),&mut b);if cap<16{assert!(r.is_err());assert!(p.is_empty());assert_eq!(p.capacity(),0);assert_eq!(b.poll(),Err(StopReason::AllocationLimit));}else{assert!(r.is_ok());assert_eq!(p.capacity(),1);assert_eq!(b.usage().allocation_units,16);}}
for cancelled in [false,true]{let mut l=budget().limits();l.work=0;let mut b=Budget::new(l);if cancelled{b.cancel();}let mut p=vec![(7,3)];let old=p.clone();assert!(push_pending(&mut p,(8,3),&mut b).is_err());assert_eq!(p,old);assert_eq!(b.usage().allocation_units,0);assert_eq!(b.poll(),Err(if cancelled{StopReason::Cancelled}else{StopReason::WorkLimit}));}
println!("Fixed private queue: 1000 growth steps actual capacities fully precharged; 0/15/16 allocation boundary, Work0/cancel preserves existing frontier");}
