use super::*;
use nepl3_core::{
    budget::{StopReason, Usage},
    schema::SchemaDescriptor,
};
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn portable_plan_context_stops_before_encoding_at_each_schema_lookup() -> Result<(), String> {
    for padding in [0, 32] {
        let mut r = SchemaRegistry::default();
        let mut reader_cost = 1;
        for i in 0..padding {
            let package = format!("padding.{i:02}.{}", "x".repeat(64));
            reader_cost += (package.len() + "nepl3.reader".len() + 9) as u64;
            let d = SchemaDescriptor {
                package,
                revision: 1,
                types: vec![],
                operations: vec![],
            };
            r.register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
                .map_err(err)?;
        }
        for d in [
            nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
            nepl3_reader::schema::descriptor(&mut budget()).map_err(err)?,
        ] {
            reader_cost += (d.package.len() + "nepl3.reader".len() + 9) as u64;
            r.register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
                .map_err(err)?;
        }
        r.finalize(&mut budget()).map_err(err)?;
        let plan = ReaderPlan {
            schema: r.selected("nepl3.reader", 1).ok_or("schema")?.clone(),
            state_type: TypeDescriptor::Unit,
            expressions: vec![ReaderExpr::Literal("a".into())],
            rules: vec![ReaderRule {
                name: "entry".into(),
                root: ReaderId(0),
                output: TypeDescriptor::Unit,
            }],
            providers: vec![],
        };
        let proof = plan.check(&r, &mut budget()).map_err(err)?;
        let sources = SourceStore::default();
        for work in [1, reader_cost + 1] {
            let mut a = SourceAdmission::default();
            let mut codec = FoundationCodec::new(&r, &sources, &mut a).map_err(err)?;
            let mut limits = budget().limits();
            limits.work = work;
            let mut stopped = Budget::new(limits);
            assert!(matches!(
                nepl3_reader::portable::plan::to_value(&proof, &mut codec, &mut stopped),
                Err(PortableError::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
            assert_eq!(
                stopped.usage(),
                Usage {
                    work,
                    ..Usage::default()
                }
            );
            let encoded = nepl3_reader::portable::plan::to_value(&proof, &mut codec, &mut budget())
                .map_err(err)?;
            let decoded =
                nepl3_reader::portable::plan::from_value(&encoded, &r, &mut codec, &mut budget())
                    .map_err(err)?;
            assert_eq!(decoded, plan);
            assert_eq!(proof.plan(), &plan);
        }
    }
    Ok(())
}
