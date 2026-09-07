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

fn main(){use nepl3_core::budget::*;use nepl3_markup::html::*;
 let mut v=Vec::<(u64,u64)>::new();v.push((0,3));println!("initial actual capacity bytes={}, precharge bytes={}",v.capacity()*std::mem::size_of::<(u64,u64)>(),2*std::mem::size_of::<(u64,u64)>());
 for width in [1u64,10000]{let mut nodes=vec![HtmlNode::Element{tag:HtmlTag::Div,attributes:vec![],children:(1..=width).collect()}];nodes.extend((0..width).map(|_|HtmlNode::Text{text:"x".into()}));let f=HtmlFragment{root:0,nodes};let mut b=Budget::new(Limits{work:1,allocation_units:10_000_000,output_bytes:1_000_000,source_bytes:1_000_000,nodes:1_000_000,depth:1000,diagnostics:1000,events:1000});let e=check_shell_depth(&f,&mut b);println!("width={width} result={e:?} work={} allocation={}",b.usage().work,b.usage().allocation_units);assert_eq!(b.poll(),Err(StopReason::WorkLimit));}
}
