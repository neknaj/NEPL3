use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::{html, katex::fragment::*};
fn b() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        nodes: 100_000,
        depth: 1000,
        output_bytes: 1_000_000,
        ..Limits::default()
    })
}
fn policy() -> Policy<'static> {
    Policy {
        classes: &["cjk_fallback", "katex", "mord", "unused"],
        scope: "nepl-math-project",
    }
}
fn span(classes: &str, style: &str, aria_hidden: Option<bool>, children: &[u64]) -> Node {
    Node::Span {
        classes: classes.into(),
        style: style.into(),
        aria_hidden,
        children: children.into(),
    }
}
fn input() -> Fragment {
    Fragment {
        nodes: vec![
            Node::Text("x<&\r日本".into()),
            span("mord mord cjk_fallback", "height:1em;", None, &[0]),
            Node::Path {
                data: "M0 0L1 1Z".into(),
            },
            Node::Line {
                x1: Endpoint::Zero,
                y1: Endpoint::Full,
                x2: Endpoint::Full,
                y2: Endpoint::Zero,
                stroke_width: ".046em".into(),
            },
            Node::Svg {
                width: "100%".into(),
                height: "1em".into(),
                view_box: Some("0 0 1 1".into()),
                aspect: Some(Aspect::None),
                children: vec![2, 3],
            },
            span("", "top:-.2em;", Some(false), &[4]),
            span("katex", "position:relative;", Some(true), &[1, 5]),
        ],
    }
}
#[test]
fn projection_preserves_indices_buffers_css_preorder_and_aria_wrapper()
-> Result<(), ProjectionError> {
    let f = input();
    let Node::Text(original) = &f.nodes[0] else {
        return Err(ProjectionError::Visual(Error::Root));
    };
    let pointer = original.as_ptr();
    let Node::Span { children, .. } = &f.nodes[6] else {
        return Err(ProjectionError::Visual(Error::Root));
    };
    let root_buffer = (children.as_ptr(), children.capacity());
    let Node::Svg {
        width, children, ..
    } = &f.nodes[4]
    else {
        return Err(ProjectionError::Visual(Error::Root));
    };
    let svg_buffer = (
        children.as_ptr(),
        children.capacity(),
        width.as_ptr(),
        width.capacity(),
    );
    let Node::Path { data } = &f.nodes[2] else {
        return Err(ProjectionError::Visual(Error::Root));
    };
    let path_buffer = (data.as_ptr(), data.capacity());

    let old = serialize(&validate(&f, &policy(), &mut b())?, &mut b())?;
    let prepared = prepare_owned(f, &policy(), &mut b())?;
    let mut budget = b();
    let projected = prepared.into_html(&mut budget)?;
    assert_eq!(projected.stylesheet(), old.stylesheet());
    assert_eq!(
        projected.stylesheet(),
        ".nepl-math-project .nepl-math-project-n6{position:relative!important;}\n.nepl-math-project .nepl-math-project-n1{height:1em!important;}\n.nepl-math-project .nepl-math-project-n5{top:-.2em!important;}\n"
    );
    assert_eq!(
        budget.usage().output_bytes,
        projected.stylesheet().len() as u64
    );
    let request = projected.request();
    assert_eq!(request.fragment.root, 7);
    assert_eq!(request.slot, html::HtmlSlot::Phrasing);
    assert_eq!(request.fragment.nodes.len(), 8);
    assert!(
        matches!(&request.fragment.nodes[6], html::HtmlNode::Element { children, .. }
        if (children.as_ptr(), children.capacity()) == root_buffer)
    );
    assert!(
        matches!(&request.fragment.nodes[4], html::HtmlNode::SvgElement { element: html::HtmlSvgElement::Svg { width, .. }, children }
        if (children.as_ptr(), children.capacity(), width.as_ptr(), width.capacity()) == svg_buffer)
    );
    assert!(
        matches!(&request.fragment.nodes[2], html::HtmlNode::SvgElement { element: html::HtmlSvgElement::Path { data }, .. }
        if (data.as_ptr(), data.capacity()) == path_buffer)
    );

    assert!(request.policy.classes.iter().any(|c| c == "unused"));
    assert!(
        matches!(&request.fragment.nodes[0],html::HtmlNode::Text {text} if text.as_ptr()==pointer)
    );
    assert!(
        matches!(&request.fragment.nodes[4],html::HtmlNode::SvgElement {children,..} if children==&[2,3])
    );
    assert!(
        matches!(&request.fragment.nodes[6],html::HtmlNode::Element {children,..} if children==&[1,5])
    );
    assert!(
        matches!(&request.fragment.nodes[1],html::HtmlNode::Element {attributes,..} if attributes.iter().any(|a|matches!(a,html::HtmlAttribute::Class {values} if values==&["mord","cjk_fallback","nepl-math-project-n1"])))
    );
    let checked = html::validate(
        &request.fragment,
        request.slot,
        &request.policy,
        &mut budget,
    )?;
    let html = html::serialize(&checked, &mut budget)?;
    assert!(html.starts_with("<span aria-hidden=\"true\" class=\"nepl-math-project\">"));
    assert!(html.contains("aria-hidden=\"false\" class=\"nepl-math-project-n5\""));
    assert!(html.contains("x&lt;&amp;&#xD;日本"));
    assert!(html.contains("<path d=\"M0 0L1 1Z\"></path>"));
    assert!(!html.contains(" style="));
    assert_eq!(
        budget.usage().output_bytes,
        (projected.stylesheet().len() + html.len()) as u64
    );
    let xml = html::serialize_xhtml(&checked, &mut b())?;
    assert!(xml.starts_with("<span xmlns=\"http://www.w3.org/1999/xhtml\""));
    Ok(())
}
#[test]
fn styleless_projection_omits_empty_class_and_emits_zero_output() -> Result<(), ProjectionError> {
    let prepared = prepare_owned(
        Fragment {
            nodes: vec![span("", "", None, &[])],
        },
        &policy(),
        &mut b(),
    )?;
    let mut budget = Budget::new(Limits {
        output_bytes: 0,
        ..b().limits()
    });
    let projected = prepared.into_html(&mut budget)?;
    assert_eq!(projected.stylesheet(), "");
    assert_eq!(budget.usage().output_bytes, 0);
    assert!(
        matches!(&projected.request().fragment.nodes[0],html::HtmlNode::Element {attributes,..} if attributes.is_empty())
    );
    let (request, css) = projected.into_parts();
    assert!(css.is_empty());
    assert_eq!(request.fragment.root, 1);
    Ok(())
}
#[test]
fn projection_limits_include_final_validation_and_preserve_stops() -> Result<(), ProjectionError> {
    let mut measured = b();
    prepare_owned(input(), &policy(), &mut b())?.into_html(&mut measured)?;
    let usage = measured.usage();
    for (reason, amount) in [
        (StopReason::WorkLimit, usage.work),
        (StopReason::AllocationLimit, usage.allocation_units),
        (StopReason::NodeLimit, usage.nodes),
        (StopReason::OutputLimit, usage.output_bytes),
    ] {
        for short in [false, true] {
            let prepared = prepare_owned(input(), &policy(), &mut b())?;
            let mut limits = b().limits();
            let amount = amount - u64::from(short);
            match reason {
                StopReason::WorkLimit => limits.work = amount,
                StopReason::AllocationLimit => limits.allocation_units = amount,
                StopReason::NodeLimit => limits.nodes = amount,
                StopReason::OutputLimit => limits.output_bytes = amount,
                _ => return Err(ProjectionError::Visual(Error::Policy)),
            }
            let mut limited = Budget::new(limits);
            let result = prepared.into_html(&mut limited);
            if short {
                assert!(matches!(result,Err(ProjectionError::Stopped(s)) if s==reason));
                if reason == StopReason::NodeLimit {
                    assert_eq!(limited.usage().nodes, amount);
                    assert_eq!(limited.usage().output_bytes, usage.output_bytes);
                }
                let before = limited.usage();
                let retry = prepare_owned(input(), &policy(), &mut b())?.into_html(&mut limited);
                assert!(matches!(retry, Err(ProjectionError::Stopped(s)) if s == reason));
                assert_eq!(limited.usage(), before);
                limited.cancel();
                assert_eq!(limited.poll(), Err(reason));
                assert_eq!(limited.usage(), before);
            } else {
                result?;
            }
        }
    }
    // Five levels: outer, root span, child span, svg, path/line.
    for depth in [9, 10] {
        let prepared = prepare_owned(input(), &policy(), &mut b())?;
        let mut limited = Budget::new(Limits {
            depth,
            ..b().limits()
        });
        let result = limited.with_depth_at_least(5, |b| prepared.into_html(b));
        if depth == 9 {
            assert!(matches!(
                result,
                Err(ProjectionError::Stopped(StopReason::DepthLimit))
            ));
        } else {
            result?;
        }
    }
    let prepared = prepare_owned(input(), &policy(), &mut b())?;
    let mut zero_work = Budget::new(Limits {
        work: 0,
        ..b().limits()
    });
    assert!(matches!(
        prepared.into_html(&mut zero_work),
        Err(ProjectionError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(zero_work.usage().allocation_units, 0);
    let prepared = prepare_owned(input(), &policy(), &mut b())?;
    let mut stopped = b();
    stopped.cancel();
    assert!(matches!(
        prepared.into_html(&mut stopped),
        Err(ProjectionError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}
