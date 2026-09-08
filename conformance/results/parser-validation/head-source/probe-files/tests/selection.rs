use nepl3_core::{
    budget::{Budget, Limits},
    schema::*,
    value::KindRef,
};
use nepl3_engine::package::*;
use nepl3_reader::{
    builtin::BuiltinReader,
    plan::ReaderPlan,
    tokenizer::{ReaderMode, TakeRule, TokenReader},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn field(name: &str, ty: TypeDescriptor) -> FieldDescriptor {
    FieldDescriptor {
        name: name.into(),
        ty,
    }
}
fn node_ref() -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.foundation".into(),
        revision: 1,
        name: "NodeRef".into(),
    })
}
fn record(name: &str, fields: Vec<FieldDescriptor>) -> NamedType {
    NamedType {
        name: name.into(),
        shape: TypeShape::Record { fields },
        constraints: vec![],
    }
}
fn fixture() -> Result<(LanguagePackage, SchemaRegistry), String> {
    let mut budget = budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget),
        nepl3_reader::schema::descriptor(&mut budget),
        nepl3_engine::schema::descriptor(&mut budget),
    ] {
        let descriptor = descriptor.map_err(|e| format!("{e:?}"))?;
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("{e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("{e:?}"))?;
    }
    let descriptor = SchemaDescriptor {
        package: "fixture.syntax".into(),
        revision: 1,
        operations: vec![OperationDescriptor {
            name: "facts".into(),
            input: TypeDescriptor::Text,
            output: TypeDescriptor::Unit,
            pure: true,
        }],
        types: vec![
            record(
                "Form:Let",
                vec![field("name", node_ref()), field("body", node_ref())],
            ),
            record("Builtin:Name", vec![]),
            record("Leaf:Name", vec![]),
            record("Token:Word", vec![field("payload", TypeDescriptor::Text)]),
            record(
                "List:Cons",
                vec![
                    field(
                        "head",
                        TypeDescriptor::Named(TypeRef {
                            package: "nepl3.foundation".into(),
                            revision: 1,
                            name: "ForeignSyntax".into(),
                        }),
                    ),
                    field("tail", node_ref()),
                ],
            ),
            record("List:Nil", vec![]),
        ],
    };
    let schema = descriptor
        .reference(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .register(schema.clone(), descriptor, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let kind = |name: &str| -> Result<KindRef, String> {
        Ok(KindRef {
            schema: schema.clone(),
            local_kind: registry
                .kind_id(&schema, name)
                .map_err(|e| format!("{e:?}"))?,
        })
    };
    let package = LanguagePackage {
        schema: schema.clone(),
        payload_schemas: vec![],
        root: "Expr".into(),
        reader: ReaderPlan {
            schema: schema.clone(),
            state_type: TypeDescriptor::Unit,
            expressions: vec![],
            rules: vec![],
            providers: vec![],
        },
        modes: vec![ReaderMode {
            name: "Code".into(),
            skip: vec![],
            take: vec![TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Name),
                kind: kind("Token:Word")?,
            }],
        }],
        categories: vec![Category {
            name: "Expr".into(),
            mode: "Code".into(),
        }],
        reads: vec![
            ReadSpec::Builtin {
                reader: BuiltinReader::Name,
                kind: kind("Builtin:Name")?,
                token_kind: kind("Token:Word")?,
            },
            ReadSpec::Local {
                category: "Expr".into(),
            },
        ],
        forms: vec![Form {
            category: "Expr".into(),
            kind: kind("Form:Let")?,
            spelling: "let".into(),
            fields: vec![
                FieldSpec {
                    name: "name".into(),
                    read: ReadSpecId(0),
                },
                FieldSpec {
                    name: "body".into(),
                    read: ReadSpecId(1),
                },
            ],
            binding: BindingId(2),
            selection_rules: vec![],
            styles: vec![],
        }],
        leaves: vec![Leaf {
            category: "Expr".into(),
            kind: kind("Leaf:Name")?,
            token_kind: kind("Token:Word")?,
            payload: TypeDescriptor::Text,
            binding: BindingId(3),
            selection_rules: vec![],
            styles: vec![],
        }],
        namespaces: vec![Namespace {
            name: "Value".into(),
            policy: NamespacePolicy::Lexical,
        }],
        bindings: vec![
            Binding::Bind {
                namespace: "Value".into(),
                name: NameSelector::Field("name".into()),
            },
            Binding::Visit("body".into()),
            Binding::Group(vec![BindingId(0), BindingId(1)]),
            Binding::Reference {
                namespace: "Value".into(),
                name: NameSelector::SelfValue,
            },
        ],
        extensions: vec![],
        recovery: nepl3_engine::recovery::RecoveryPlan {
            default_unexpected: nepl3_engine::recovery::UnexpectedPolicy::PreserveRemainder,
            rules: vec![],
        },
        provenance: PackageProvenance {
            sources: vec![],
            origins: vec![],
            source_maps: vec![],
            declarations: vec![],
        },
    };
    Ok((package, registry))
}

#[test]
fn independent_selection_matrix() -> TestResult {
    use nepl3_core::{origin::*, source::*, syntax::*, value::NdfValue, view::*};
    use nepl3_engine::{profile::*, recovery::*, selection::*, tree::*};
    let (package, registry) = fixture()?;
    let identity = package
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?
        .semantic_identity(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let profile = ParseProfile {
        id: "tree-test".into(),
        languages: vec![
            LanguageRegistration {
                alias: "A".into(),
                package: identity.clone(),
                default_category: "Expr".into(),
            },
            LanguageRegistration {
                alias: "B".into(),
                package: identity,
                default_category: "Expr".into(),
            },
        ],
        schemas: vec![
            package.schema.clone(),
            registry
                .selected("nepl3.foundation", 1)
                .ok_or("foundation")?
                .clone(),
        ],
        head_providers: vec![],
        category_modes: vec![],
        providers: vec![],
        allowlist: vec![],
        resources: vec![],
        limits: budget().limits(),
    };
    let packages = [&package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let entry = resolved
        .entry("A", None, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let execution = resolved
        .execution_digest("A", &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    use nepl3_core::budget::{Resource,StopReason};
    let make=|n:usize| -> Result<ParseTree,String> {
        let text="let x ".repeat(n)+"y";
        let source=SourceSnapshot::new(SourceId("tree-\u{65e5}\u{672c}".into()),u64::MAX,"memory:tree".into(),text.as_bytes().to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?;
        let mut nodes=vec![];let mut tokens=vec![];let mut origins=vec![];let mut selected=vec![];
        for id in 0..=n*2 {
            let is_form=id<n*2 && id%2==0;let is_name=id%2==1;
            let start=if is_form {id/2*6}else if is_name {id/2*6+4}else{n*6};let end=start+if is_form{3}else{1};
            let span=source.span(start as u64,end as u64).map_err(|e|format!("{e:?}"))?;
            let fields=if is_form{vec![FieldValue::Child(NodeRef(id as u64+1)),FieldValue::Child(NodeRef(id as u64+2))]}else{vec![]};
            nodes.push(SyntaxNode{schema:package.schema.clone(),kind:if is_form{"Form:Let"}else if is_name{"Builtin:Name"}else{"Leaf:Name"}.into(),fields,head:Some(span.clone()),cover:Some(if is_form{source.span(start as u64,text.len() as u64).map_err(|e|format!("{e:?}"))?}else{span.clone()}),origin:OriginId(id as u64),token:Some(TokenRef(id as u64))});
            tokens.push(Token{kind:package.leaves[0].token_kind.clone(),head:span.clone(),payload:NdfValue::Text(text[start..end].into()),views:ViewBundle{elements:vec![],roots:vec![]},leading_trivia:vec![]});
            origins.push(Origin::Direct(span));selected.push(NodeSelection{node:NodeRef(id as u64),entry:entry.clone(),execution_digest:execution,shape:if is_form{ShapeSelection::Form{index:0}}else if is_name{ShapeSelection::Builtin{read:ReadSpecId(0)}}else{ShapeSelection::Leaf{index:0}}});
        }
        Ok(ParseTree{profile_digest:resolved.digest(),bundle:SyntaxBundle{sources:vec![source],nodes,origins,root:NodeRef(0),environments:vec![],tokens,source_maps:vec![]},recovery:vec![],contexts:vec![BundleContext{path:vec![],nodes:selected}]})
    };
    let validate=|tree:&ParseTree,b:&mut Budget|tree.validate(&resolved,b,&mut SourceAdmission::default()).map(|_|());
    for count in [1,2,8,32] {
        let original=make(16)?;
        let mut all=vec![original.bundle.sources[0].clone()];
        for revision in 0..count-1 {all.push(SourceSnapshot::new(original.bundle.sources[0].identity().source.clone(),revision,"memory:unrelated".into(),b"not a form".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?);}
        for position in [0,count/2,count-1] {
            let mut tree=original.clone();tree.bundle.sources=all.clone();tree.bundle.sources.swap(0,position as usize);
            let saved=tree.clone();let mut b=budget();validate(&tree,&mut b).map_err(|e|format!("count{count} pos{position}: {e:?}"))?;assert_eq!(tree,saved);
            println!("COST count={count} position={position} {:?}",b.usage());
            for cap in [0,1,100,b.usage().work-1,b.usage().work,b.usage().work+1] {
                let mut limits=budget().limits();limits.work=cap;let mut limited=Budget::new(limits);let result=validate(&tree,&mut limited);
                if cap<b.usage().work {assert_eq!(result,Err(TreeError::Stopped(StopReason::WorkLimit)));assert_eq!(limited.poll(),Err(StopReason::WorkLimit));}else{assert!(result.is_ok());}
            }
        }
    }
    let original=make(1)?;
    let mut fallback=original.clone();fallback.bundle.sources.insert(0,SourceSnapshot::new(original.bundle.sources[0].identity().source.clone(),0,"memory:wrong-revision".into(),b"not a form".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?);
    let mut full=budget();validate(&fallback,&mut full).map_err(|e|format!("{e:?}"))?;
    for cap in (0..=full.usage().work+1).step_by(17) {
        let mut limits=budget().limits();limits.work=cap;let mut b=Budget::new(limits);let result=validate(&fallback,&mut b);
        assert!(result.is_ok() || result==Err(TreeError::Stopped(StopReason::WorkLimit)));
        println!("STOP cap={cap} result={result:?} work={}",b.usage().work);
    }
    for mode in 0..6 {
        let mut tree=original.clone();let source=&original.bundle.sources[0];
        match mode {
            0=>tree.bundle.sources.clear(),
            1=>tree.bundle.sources[0]=SourceSnapshot::new(source.identity().source.clone(),source.identity().revision,"memory:tree".into(),b"bad x y".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?,
            2=>tree.bundle.sources[0]=SourceSnapshot::new(source.identity().source.clone(),0,"memory:tree".into(),source.text().as_bytes().to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?,
            3=>tree.bundle.sources.insert(0,SourceSnapshot::new(source.identity().source.clone(),source.identity().revision,"memory:other-uri".into(),source.text().as_bytes().to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?),
            4=>tree.bundle.sources.push(source.clone()),
            _=>tree.bundle.sources.insert(0,SourceSnapshot::new(source.identity().source.clone(),source.identity().revision,"memory:tree".into(),b"bad x y".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?),
        }
        let result=validate(&tree,&mut budget());assert!(result.is_err());println!("ERROR mode={mode}: {result:?}");
    }
    let mut b=budget();b.charge(Resource::Work,9).map_err(|e|format!("{e:?}"))?;let usage=b.usage();b.cancel();assert_eq!(validate(&original,&mut b),Err(TreeError::Stopped(StopReason::Cancelled)));assert_eq!(usage,b.usage());
    for cap in [0,1,100] {let mut limits=budget().limits();limits.allocation_units=cap;let mut b=Budget::new(limits);assert_eq!(validate(&original,&mut b),Err(TreeError::Stopped(StopReason::AllocationLimit)));}
    Ok(())
}
