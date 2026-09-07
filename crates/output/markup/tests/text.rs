use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::text::{TextContext, TextError, escape, is_xml_character};

fn budget() -> Budget {
    Budget::new(limits())
}
fn limits() -> Limits {
    Limits {
        source_bytes: 1000000,
        work: 1000000,
        depth: 1000,
        nodes: 1000000,
        allocation_units: 1000000,
        output_bytes: 1000000,
        diagnostics: 1000,
        events: 1000,
    }
}

#[test]
fn canonical_escape_preserves_text_and_attribute_whitespace() -> Result<(), TextError> {
    let input = "日本語🙂<&>\"'\r\n\t";
    let mut content = budget();
    assert_eq!(
        escape(input, TextContext::Content, &mut content)?,
        "日本語🙂&lt;&amp;&gt;\"'&#xD;\n\t"
    );
    let mut attribute = budget();
    assert_eq!(
        escape(input, TextContext::Attribute, &mut attribute)?,
        "日本語🙂&lt;&amp;&gt;&quot;'&#xD;&#xA;&#x9;"
    );
    assert_eq!(content.usage().source_bytes, 0);
    assert_eq!(attribute.usage().source_bytes, 0);
    assert_eq!(escape("", TextContext::Content, &mut budget())?, "");
    Ok(())
}

#[test]
fn xml_character_ranges_and_error_byte_offsets_are_exact() {
    for scalar in 0..=0x10ffff {
        let Some(c) = char::from_u32(scalar) else {
            continue;
        };
        let expected = [9, 10, 13].contains(&scalar)
            || (32..=0xd7ff).contains(&scalar)
            || (0xe000..=0xfffd).contains(&scalar)
            || (0x10000..=0x10ffff).contains(&scalar);
        assert_eq!(is_xml_character(c), expected, "scalar {scalar}");
    }
    for c in ['\0', '\u{b}', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
        let input = format!("あ🙂{c}");
        let mut b = budget();
        assert_eq!(
            escape(&input, TextContext::Content, &mut b),
            Err(TextError::InvalidCharacter {
                byte: 7,
                scalar: u64::from(c)
            })
        );
        assert_eq!(b.usage().allocation_units, 0);
        assert_eq!(b.usage().output_bytes, 0);
        assert!(b.poll().is_ok());
    }
}

#[test]
fn escaping_limits_precede_result_allocation_and_stops_are_sticky() -> Result<(), TextError> {
    let input = "<&🙂";
    // &lt; (4), &amp; (5), and the unchanged four-byte scalar.
    let size = 13;
    let mut exact = Budget::new(Limits {
        output_bytes: size,
        allocation_units: size,
        ..limits()
    });
    assert_eq!(
        escape(input, TextContext::Content, &mut exact)?.len(),
        size as usize
    );
    assert_eq!(exact.usage().output_bytes, size);
    assert_eq!(exact.usage().allocation_units, size);
    for reason in [
        StopReason::WorkLimit,
        StopReason::OutputLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::OutputLimit => limits.output_bytes = size - 1,
            StopReason::AllocationLimit => limits.allocation_units = size - 1,
            _ => {}
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert_eq!(
            escape(input, TextContext::Content, &mut b),
            Err(TextError::Stopped(reason))
        );
        assert_eq!(
            escape("", TextContext::Content, &mut b),
            Err(TextError::Stopped(reason))
        );
        assert_eq!(b.usage().allocation_units, 0);
    }
    Ok(())
}
