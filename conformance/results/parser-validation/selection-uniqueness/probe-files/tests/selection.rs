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
    for n in [0,1,4,16,64,128] {
        let original=make(n)?;
        for order in 0..3 {
            let mut tree=original.clone();let nodes=&mut tree.contexts[0].nodes;
            if order==1{nodes.reverse();}if order==2 {let len=nodes.len();for i in 0..len{nodes.swap(i,(i*19+7)%len);}}
            let saved=tree.clone();let mut b=budget();validate(&tree,&mut b).map_err(|e|format!("n{n} order{order}: {e:?}"))?;assert_eq!(tree,saved);
            println!("COST n={} order={} {:?}",tree.bundle.nodes.len(),order,b.usage());
        }
    }
    let tree=make(1)?;
    let mut bad=tree.clone();bad.bundle.root=NodeRef(2);bad.contexts[0].nodes.pop();assert_eq!(validate(&bad,&mut budget()),Err(TreeError::Unreachable));
    let mut bad=tree.clone();bad.bundle.nodes.clear();assert!(matches!(validate(&bad,&mut budget()),Err(TreeError::Syntax(_))));
    let mut bad=tree.clone();bad.contexts.clear();assert_eq!(validate(&bad,&mut budget()),Err(TreeError::Selection));
    for a in [0,1,2,3,u64::MAX] {for b in [0,1,2,3,u64::MAX] {for c in [0,1,2,3,u64::MAX] {
        let mut altered=tree.clone();for (i,id) in [a,b,c].iter().enumerate(){altered.contexts[0].nodes[i]=tree.contexts[0].nodes[usize::try_from(*id).ok().filter(|i|*i<3).unwrap_or(0)].clone();altered.contexts[0].nodes[i].node=NodeRef(*id);}
        let result=validate(&altered,&mut budget());println!("MATRIX {a},{b},{c}: {result:?}");
        if a<3&&b<3&&c<3&&a!=b&&a!=c&&b!=c{assert!(result.is_ok());}else{assert!(result.is_err());}
    }}}
    let mut bad=tree.clone();bad.contexts[0].nodes.pop();bad.contexts[0].nodes[0].node=NodeRef(u64::MAX);assert_eq!(validate(&bad,&mut budget()),Err(TreeError::Selection));
    let mut bad=tree.clone();bad.contexts[0].nodes=vec![tree.contexts[0].nodes[1].clone(),tree.contexts[0].nodes[1].clone(),tree.contexts[0].nodes[0].clone()];bad.contexts[0].nodes[1].entry.alias="missing".into();assert_eq!(validate(&bad,&mut budget()),Err(TreeError::Duplicate));
    bad.contexts[0].nodes[0].entry.alias="missing".into();let result=validate(&bad,&mut budget());assert!(matches!(result,Err(TreeError::Profile(_))));println!("PRECEDENCE {result:?}");
    let tree=make(16)?;let mut full=budget();validate(&tree,&mut full).map_err(|e|format!("{e:?}"))?;
    for cap in [0,1,10,100,1000,full.usage().work-1,full.usage().work,full.usage().work+1] {
        let mut limits=budget().limits();limits.work=cap;let mut b=Budget::new(limits);let result=validate(&tree,&mut b);
        if cap<full.usage().work {assert_eq!(result,Err(TreeError::Stopped(StopReason::WorkLimit)));assert_eq!(b.poll(),Err(StopReason::WorkLimit));}else{assert!(result.is_ok());}
    }
    for allocation in [0,1,10,100,1000] {let mut limits=budget().limits();limits.allocation_units=allocation;let mut b=Budget::new(limits);assert_eq!(validate(&tree,&mut b),Err(TreeError::Stopped(StopReason::AllocationLimit)));}
    let mut b=budget();b.cancel();assert_eq!(validate(&tree,&mut b),Err(TreeError::Stopped(StopReason::Cancelled)));assert_eq!(b.usage().allocation_units,0);
    let mut b=budget();b.charge(Resource::Work,9).map_err(|e|format!("{e:?}"))?;let before=b.usage();b.cancel();assert_eq!(validate(&tree,&mut b),Err(TreeError::Stopped(StopReason::Cancelled)));assert_eq!(b.usage(),before);
    Ok(())
}
