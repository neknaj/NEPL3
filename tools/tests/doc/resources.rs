use nepl3_tools::doc::{
    export::pages::{self, Entry, resources::PhaseLimits},
    source,
};
fn entry(id: &str, source: &str, route: &str) -> Entry {
    Entry {
        id: id.into(),
        source: source.into(),
        route: route.into(),
        input: None,
    }
}
#[test]
fn actual_doc_links_to_exact_registered_file_bytes() -> Result<(), String> {
    let compiled = source::compiled()?;
    let inputs = vec![(entry("guide", "doc/guide.md", "docs/guide/index.html"),
        "article en \"Guide\" body cons paragraph cons sentence cons link relative \"../design/data.json\" none text \"Contract\" nil nil nil".into())];
    let payload = b"{\"version\":1}\r\n".to_vec();
    let resources = vec![(
        entry("data", "design/data.json", "data/data.json"),
        payload.clone(),
    )];
    let output = pages::generate_with_resources(
        &compiled,
        &inputs,
        &resources,
        PhaseLimits::default(),
        &mut source::budget(),
    )?;
    assert_eq!(output.files["data/data.json"], payload);
    let html = std::str::from_utf8(&output.files["docs/guide/index.html"]).map_err(super::err)?;
    assert!(html.contains("href=\"../../data/data.json\""), "{html}");
    let manifest: serde_json::Value = serde_json::from_str(&output.manifest).map_err(super::err)?;
    assert_eq!(
        manifest["registered_files"][0]["source"],
        "design/data.json"
    );
    assert!(
        pages::generate_with_resources(
            &compiled,
            &inputs,
            &[],
            PhaseLimits::default(),
            &mut source::budget()
        )
        .is_err()
    );
    let mut changed = resources;
    changed[0].1.push(b' ');
    let new = pages::generate_with_resources(
        &compiled,
        &inputs,
        &changed,
        PhaseLimits::default(),
        &mut source::budget(),
    )?;
    let other: serde_json::Value = serde_json::from_str(&new.manifest).map_err(super::err)?;
    assert_ne!(manifest["identity"], other["identity"]);
    assert_ne!(manifest["execution_identity"], other["execution_identity"]);
    Ok(())
}
#[test]
fn registered_resources_obey_output_stop_and_reserved_routes() -> Result<(), String> {
    let compiled = source::compiled()?;
    let inputs = vec![(
        entry("guide", "doc/guide.md", "index.html"),
        "article en \"Guide\" body nil".into(),
    )];
    for route in ["index.html", "manifest.json", "assets/doc.css", "data/CON"] {
        let resources = vec![(entry("file", "data/item", route), b"x".to_vec())];
        assert!(
            pages::generate_with_resources(
                &compiled,
                &inputs,
                &resources,
                PhaseLimits::default(),
                &mut source::budget()
            )
            .is_err(),
            "{route}"
        );
    }
    let resources = vec![(entry("file", "data/item", "data/item"), vec![0, 255])];
    let mut limits = source::budget().limits();
    limits.output_bytes = 1;
    let mut budget = nepl3_core::budget::Budget::new(limits);
    let failure = pages::generate_with_resources(
        &compiled,
        &inputs,
        &resources,
        PhaseLimits::default(),
        &mut budget,
    )
    .err()
    .ok_or("unexpected success")?;
    assert!(failure.contains("OutputLimit"), "{failure}");
    assert!(
        pages::generate_with_resources(
            &compiled,
            &inputs,
            &resources,
            PhaseLimits::default(),
            &mut budget
        )
        .is_err()
    );
    Ok(())
}
