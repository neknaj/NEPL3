//! Inspect root declarations, independently of comments and formatting.
//! This is not macro expansion or a proof of target portability; builds remain required.
use crate::Result;
use syn::{Attribute, Item, Meta, Token, punctuated::Punctuated};

fn gated(meta: &Meta) -> syn::Result<bool> {
    if meta.path().is_ident("cfg") {
        return Ok(true);
    }
    if let Meta::List(list) = meta
        && list.path.is_ident("cfg_attr")
    {
        // The first entry is the predicate. Only attributes after it can gate
        // the declaration. Conditional doc/lint attributes do not disable it.
        let entries = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for entry in entries.iter().skip(1) {
            if gated(entry)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn unconditional(attrs: &[Attribute]) -> syn::Result<bool> {
    for attr in attrs {
        if gated(&attr.meta)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn check(source: &str) -> Result<()> {
    let file = syn::parse_file(source)?;
    let no_std = file
        .attrs
        .iter()
        .any(|attr| matches!(&attr.meta, Meta::Path(path) if path.is_ident("no_std")));
    let mut alloc = false;
    for item in &file.items {
        if let Item::ExternCrate(item) = item
            && item.ident == "alloc"
            && unconditional(&item.attrs)?
        {
            alloc = true;
        }
    }
    if !no_std || !alloc || !unconditional(&file.attrs)? {
        return Err("requires unconditional root no_std and alloc declarations".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_are_syntax_not_lines() -> Result<()> {
        for source in [
            "#![no_std]\nextern crate alloc;",
            "#! [ no_std ]\nextern /* comment */ crate\nalloc;",
            "#![no_std]\n#[macro_use] extern crate alloc;",
            "#![no_std]\n#[cfg_attr(feature=\"docs\", doc=\"allocation\")] extern crate alloc;",
        ] {
            check(source)?;
        }
        Ok(())
    }

    #[test]
    fn comments_literals_nested_items_and_conditional_declarations_do_not_count() {
        for source in [
            "/*\n#![no_std]\nextern crate alloc;\n*/\npub fn f() {}",
            "const TEXT: &str = r#\"\n#![no_std]\nextern crate alloc;\n\"#;",
            "mod child { #![no_std] extern crate alloc; }",
            "#![cfg_attr(not(test), no_std)]\nextern crate alloc;",
            "#![no_std]\n#[cfg(any())] extern crate alloc;",
            "#![no_std]\n#[cfg_attr(all(), cfg(any()))] extern crate alloc;",
            "#![no_std]\n#[cfg_attr(all(), cfg_attr(all(), cfg(any())))] extern crate alloc;",
            "#![no_std]\n#![cfg(any())]\nextern crate alloc;",
            "#![no_std]\nmacro_rules! declarations { () => { extern crate alloc; } }",
            "#![no_std]\nextern crate",
        ] {
            assert!(check(source).is_err(), "{source}");
        }
    }
}
