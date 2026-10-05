use super::*;
#[path = "code_pages/native.rs"]
mod native;
use nepl3_tools::doc::export::pages::{self, Entry};

fn input(id: &str, source: &str) -> (Entry, String) {
    (
        Entry {
            id: id.into(),
            source: format!("{id}.nepld"),
            route: format!("{id}/index.html"),
            input: None,
        },
        source.into(),
    )
}

#[test]
fn code_page_reuses_frontend_and_keeps_cross_page_links() -> Result<(), String> {
    let c = compiled()?;
    let guest = r#"article en sentence nil body cons paragraph cons sentence cons ruby text "" text "r" nil nil nil"#;
    let a = format!(
        r#"article en "Host A" body cons paragraph cons sentence cons link page "b" some "target" text "B" nil nil cons paragraph cons code Doc {guest} cons "host-tail" nil nil"#
    );
    let b = r#"article en "Host B" body cons section target "Target" body cons paragraph cons sentence cons link page "a" none text "A" nil nil nil nil"#;
    let out = pages::generate(&c, &[input("a", &a), input("b", b)])?;
    let html =
        core::str::from_utf8(out.files.get("a/index.html").ok_or("a missing")?).map_err(err)?;
    assert!(html.contains("nepl-code-marker"), "{html}");
    assert!(html.contains("data-nepl-id=\"pc-"), "{html}");
    assert!(html.contains("../b/index.html#"), "{html}");
    assert!(html.contains("host-tail"));
    let block = html
        .split("<code>")
        .nth(1)
        .ok_or("code missing")?
        .split("</code></pre>")
        .next()
        .ok_or("code end")?;
    assert!(!block.contains("Host A") && !block.contains("host-tail"));
    assert!(block.contains("article") && block.contains("ruby"));
    let html_b =
        core::str::from_utf8(out.files.get("b/index.html").ok_or("b missing")?).map_err(err)?;
    assert!(html_b.contains("../a/index.html"));
    assert!(
        out.files.contains_key("a/assets/doc.css") && out.files.contains_key("b/assets/doc.css")
    );
    Ok(())
}

#[test]
fn code_pages_reject_other_guests_and_preserve_output_stops() -> Result<(), String> {
    let c = compiled()?;
    let code = input(
        "a",
        r#"article en "Host" body cons paragraph cons code Doc article en "Guest" body nil nil nil"#,
    );
    for reason in [
        StopReason::Cancelled,
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
    ] {
        let mut limits = budget().limits();
        if reason == StopReason::WorkLimit {
            limits.work = 0;
        }
        if reason == StopReason::AllocationLimit {
            limits.allocation_units = 0;
        }
        if reason == StopReason::OutputLimit {
            limits.output_bytes = 0;
        }
        if reason == StopReason::NodeLimit {
            limits.nodes = 0;
        }
        if reason == StopReason::DepthLimit {
            limits.depth = 0;
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert!(pages::generate_with_output_budget(&c, &[input("a", &code.1)], &mut b).is_err());
        assert_eq!(b.poll(), Err(reason));
    }
    let math = input(
        "b",
        r#"article en "Math" body cons display Math frac 1 0 nil"#,
    );
    assert!(pages::generate(&c, &[code, math]).is_err_and(|e| e.contains("NeedsResolution")));
    Ok(())
}
