use nepl3_markup::html::HtmlError;
use nepl3_core::budget::{Budget,Resource,StopReason,Limits};
fn allocate(n: usize, b: &mut Budget) -> Result<(), HtmlError> {
    if n > isize::MAX as usize {
        return Err(b.stop(StopReason::AllocationLimit).into());
    }
    b.charge(Resource::Work, n as u64)?;
    b.charge(Resource::AllocationUnits, n as u64)?;
    Ok(())
}

pub fn check(){
let limits=Limits{work:u64::MAX,allocation_units:u64::MAX,..Limits::default()};
for n in [isize::MAX as usize+1,usize::MAX]{let mut b=Budget::new(limits);assert!(matches!(allocate(n,&mut b),Err(HtmlError::Stopped(StopReason::AllocationLimit))));assert_eq!(b.usage().work,0);assert_eq!(b.usage().allocation_units,0);assert_eq!(b.poll(),Err(StopReason::AllocationLimit));}
let mut b=Budget::new(limits);assert!(allocate(isize::MAX as usize,&mut b).is_ok());assert_eq!(b.usage().allocation_units,isize::MAX as u64);
let mut b=Budget::new(limits);b.stop(StopReason::Cancelled);assert!(matches!(allocate(0,&mut b),Err(HtmlError::Stopped(StopReason::Cancelled))));
println!("final exact allocation guard: max+1/usizeMAX rejected before usage; max accepted accounting only; sticky cancel");
}
