//! Explicit native-host compatibility decoration, separate from Doc identity.
//! Bounded JSON and receipt bookkeeping are host work, not runtime Usage.
use crate::doc::{projection::annotated::Alias, source::err};
use nepl3_core::{
    budget::{Budget, Resource},
    source::Digest,
};
use nepl3_doc_html::{ElementOrigin, RenderedFragment};
use nepl3_markup::html::{HtmlAttribute, HtmlNode, HtmlTag};

pub(crate) struct PageAliases {
    pub page: String,
    pub raw: Vec<u8>,
    pub values: Vec<Alias>,
}

impl PageAliases {
    pub(crate) fn parse(page: String, raw: Vec<u8>) -> Result<Self, String> {
        if raw.len() > 1_048_576 {
            return Err("AliasInputLimit".into());
        }
        crate::repository::json::validate(std::str::from_utf8(&raw).map_err(err)?).map_err(err)?;
        let values: Vec<Alias> = serde_json::from_slice(&raw).map_err(err)?;
        if values.len() > 4096 {
            return Err("AliasCountLimit".into());
        }
        Ok(Self { page, raw, values })
    }
}

pub(crate) fn check_binding(
    inputs: &[(super::Entry, String)],
    aliases: &[PageAliases],
    budget: &mut Budget,
) -> Result<(), String> {
    if aliases.len() > 128 {
        return Err("AliasPageLimit".into());
    }
    let mut total = 0usize;
    for (i, alias) in aliases.iter().enumerate() {
        charge(budget, Resource::Work, 1)?;
        total = total
            .checked_add(alias.raw.len())
            .ok_or("AliasInputLimit")?;
        if total > 10_000_000 || alias.page.len() > 4096 {
            return Err("AliasInputLimit".into());
        }
        for prior in &aliases[..i] {
            charge(
                budget,
                Resource::Work,
                prior.page.len() + alias.page.len() + 1,
            )?;
            if prior.page == alias.page {
                return Err("AliasPageBinding".into());
            }
        }
        let mut matches = 0usize;
        for (entry, _) in inputs {
            charge(
                budget,
                Resource::Work,
                entry.id.len() + alias.page.len() + 1,
            )?;
            if entry.id == alias.page {
                matches += 1;
            }
        }
        if matches != 1 {
            return Err("AliasPageBinding".into());
        }
    }
    Ok(())
}

fn charge(b: &mut Budget, resource: Resource, size: usize) -> Result<(), String> {
    b.charge(resource, size as u64).map_err(err)
}
fn push<T>(values: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), String> {
    charge(b, Resource::Work, 1)?;
    charge(b, Resource::AllocationUnits, 2 * std::mem::size_of::<T>())?;
    values.push(value);
    Ok(())
}
fn semantic_id(section: &str, b: &mut Budget) -> Result<String, String> {
    let len = section
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(2))
        .ok_or("AliasIdLimit")?;
    charge(b, Resource::Work, len)?;
    charge(b, Resource::AllocationUnits, len)?;
    let mut id = String::with_capacity(len);
    id.push_str("n-");
    for byte in section.bytes() {
        id.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
        id.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
    }
    Ok(id)
}

pub(crate) fn apply(
    fragment: &mut RenderedFragment,
    aliases: &[Alias],
    b: &mut Budget,
) -> Result<(), String> {
    b.poll().map_err(err)?;
    // Process in reverse so prepending retains the author-supplied order.
    for alias in aliases.iter().rev() {
        charge(b, Resource::Work, alias.name.len() + 1)?;
        if alias.name.is_empty()
            || !alias
                .name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err("InvalidCompatibilityAnchor".into());
        }
        let selected = alias
            .section
            .as_deref()
            .map(|s| semantic_id(s, b))
            .transpose()?;
        let mut target = None;
        for (i, node) in fragment.markup.fragment.nodes.iter().enumerate() {
            charge(b, Resource::Work, 1)?;
            if let HtmlNode::Element {
                tag, attributes, ..
            } = node
            {
                let mut matches = selected.is_none()
                    && i as u64 == fragment.markup.fragment.root
                    && *tag == HtmlTag::Article;
                for attr in attributes {
                    charge(b, Resource::Work, 1)?;
                    if let HtmlAttribute::Id { value } = attr {
                        charge(b, Resource::Work, value.len() + alias.name.len())?;
                        if value == &alias.name {
                            return Err("CompatibilityAnchorCollision".into());
                        }
                        if let Some(id) = &selected {
                            charge(b, Resource::Work, value.len() + id.len())?;
                            matches |= *tag == HtmlTag::Section && value == id;
                        }
                    }
                }
                if matches && target.replace(i).is_some() {
                    return Err("AmbiguousCompatibilitySection".into());
                }
            }
        }
        let target = target.ok_or("MissingCompatibilitySection")?;
        let mut cause = None;
        for origin in &fragment.origins {
            charge(b, Resource::Work, 1)?;
            if origin.element == target as u64 && cause.replace(origin.node).is_some() {
                return Err("AmbiguousCompatibilityOrigin".into());
            }
        }
        let cause = cause.ok_or("MissingCompatibilityOrigin")?;
        charge(b, Resource::Nodes, 1)?;
        charge(b, Resource::AllocationUnits, alias.name.len())?;
        let mut attributes = Vec::new();
        push(
            &mut attributes,
            HtmlAttribute::Id {
                value: alias.name.clone(),
            },
            b,
        )?;
        let index = fragment.markup.fragment.nodes.len() as u64;
        push(
            &mut fragment.markup.fragment.nodes,
            HtmlNode::Element {
                tag: HtmlTag::Span,
                attributes,
                children: Vec::new(),
            },
            b,
        )?;
        push(
            &mut fragment.origins,
            ElementOrigin {
                element: index,
                node: cause,
            },
            b,
        )?;
        let Some(HtmlNode::Element { children, .. }) =
            fragment.markup.fragment.nodes.get_mut(target)
        else {
            return Err("CompatibilityTargetShape".into());
        };
        charge(b, Resource::Work, children.len() + 1)?;
        charge(b, Resource::AllocationUnits, 2 * std::mem::size_of::<u64>())?;
        children.insert(0, index);
    }
    Ok(())
}

pub(crate) fn record(
    generated: &mut super::GeneratedPages,
    aliases: &[PageAliases],
) -> Result<(), String> {
    if aliases.is_empty() {
        return Ok(());
    }
    if generated.manifest.len() > 8 * 1024 * 1024 {
        return Err("AliasReceiptLimit".into());
    }
    let mut manifest: serde_json::Value = serde_json::from_str(&generated.manifest).map_err(err)?;
    let mut bytes = b"nepl3.canonical-html.compatibility/1\0".to_vec();
    bytes.extend_from_slice(&(aliases.len() as u64).to_be_bytes());
    let mut pages = Vec::new();
    for alias in aliases {
        let digest = Digest::of(&alias.raw);
        bytes.extend_from_slice(&(alias.page.len() as u64).to_be_bytes());
        bytes.extend_from_slice(alias.page.as_bytes());
        bytes.extend_from_slice(&digest.0);
        pages.push(serde_json::json!({"page":alias.page,"input_sha256":super::digest_hex(digest),"aliases":alias.values.len()}));
    }
    let identity = super::digest_hex(Digest::of(&bytes));
    let execution = manifest["execution_identity"]
        .as_str()
        .ok_or("MissingExecutionIdentity")?;
    let mut composed = b"nepl3.canonical-html.compatibility-execution/1\0".to_vec();
    for field in [execution, identity.as_str()] {
        composed.extend_from_slice(&(field.len() as u64).to_be_bytes());
        composed.extend_from_slice(field.as_bytes());
    }
    manifest["compatibility"] = serde_json::json!({
        "renderer":"nepl3-tools.canonical-html-aliases/1","input_identity":identity,
        "execution_identity":super::digest_hex(Digest::of(&composed)),"pages":pages,
        "scope":"Native host decoration; document/PageSet identities remain semantic identities. Bounded JSON/file/receipt work is excluded from runtime Usage; typed decoration and final markup validation/serialization share the existing output Budget."
    });
    let updated = serde_json::to_string_pretty(&manifest).map_err(err)? + "\n";
    if updated.len() > 16 * 1024 * 1024 {
        return Err("AliasReceiptLimit".into());
    }
    generated.manifest = updated;
    Ok(())
}

#[cfg(test)]
mod tests;
