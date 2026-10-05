//! Operation-local Code inputs built alongside the exact parsed page. Never
//! populated from a serialized cache or an unrelated provider's markup.
use super::*;
use nepl3_core::{
    budget::{Resource, StopReason},
    schema::SchemaRegistry,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::model::{DocEmbed, DocumentSyntax, EmbedKind, EmbedRef};
use nepl3_engine::{profile::ResolvedParseProfile, tree::ValidatedParseTree};
use nepl3_markup::html::{HtmlAttribute, HtmlFragment, HtmlNode, HtmlPolicy, HtmlRequest, HtmlTag};

struct Entry {
    index: EmbedRef,
    digest: Digest,
    markup: HtmlRequest,
}
pub(super) struct PreparedPageCode {
    entries: Vec<Entry>,
}

fn digest(embed: &DocEmbed, registry: &SchemaRegistry, b: &mut Budget) -> Result<Digest, String> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let value = codec
        .encode_foreign_closure(&embed.closure, b)
        .map_err(err)?;
    codec
        .canonical_value_digest(nepl3_doc_core::prepare::GUEST_DOMAIN, &value, b)
        .map_err(err)
}

impl PreparedPageCode {
    pub(super) fn prepare(
        document: &DocumentSyntax,
        tree: &ValidatedParseTree<'_>,
        profile: &ResolvedParseProfile<'_>,
        b: &mut Budget,
    ) -> Result<Self, String> {
        let mut entries = Vec::new();
        for (index, embed) in document.value.embeds.iter().enumerate() {
            b.charge(Resource::Work, 1).map_err(err)?;
            if embed.kind != EmbedKind::Code {
                continue;
            }
            let digest = digest(embed, profile.registry(), b)?;
            let markup = super::super::code::render(embed, tree, profile, b)?;
            b.charge(
                Resource::AllocationUnits,
                (2 * core::mem::size_of::<Entry>()) as u64,
            )
            .map_err(err)?;
            entries.push(Entry {
                index: EmbedRef(index as u64),
                digest,
                markup,
            });
        }
        Ok(Self { entries })
    }

    pub(super) fn render(
        &self,
        embed: &DocEmbed,
        index: EmbedRef,
        registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<HtmlRequest, String> {
        b.poll().map_err(err)?;
        if embed.kind != EmbedKind::Code {
            return Err("CodeKind".into());
        }
        let mut selected = None;
        for entry in &self.entries {
            b.charge(Resource::Work, 1).map_err(err)?;
            if entry.index == index {
                selected = Some(entry);
                break;
            }
        }
        let entry = selected.ok_or("CodeInputMissing")?;
        if entry.digest != digest(embed, registry, b)? {
            return Err("CodeInputMismatch".into());
        }
        // Shared embeds may appear more than once. Precharge the exact owned
        // flat Span/Text representation before cloning it for each occurrence.
        // A future adapter expansion must extend this explicit shape contract.
        let mut cost = core::mem::size_of::<HtmlRequest>();
        let mut add = |amount: usize| -> Result<(), String> {
            cost = cost.checked_add(amount).ok_or("CodeCloneLimit")?;
            Ok(())
        };
        for class in &entry.markup.policy.classes {
            add(core::mem::size_of::<String>())?;
            add(class.len())?;
        }
        for node in &entry.markup.fragment.nodes {
            b.charge(Resource::Work, 1).map_err(err)?;
            add(core::mem::size_of::<HtmlNode>())?;
            match node {
                HtmlNode::Text { text } => add(text.len())?,
                HtmlNode::Element {
                    tag: HtmlTag::Span,
                    attributes,
                    children,
                } => {
                    add(children
                        .len()
                        .checked_mul(core::mem::size_of::<u64>())
                        .ok_or("CodeCloneLimit")?)?;
                    for attribute in attributes {
                        b.charge(Resource::Work, 1).map_err(err)?;
                        add(core::mem::size_of::<HtmlAttribute>())?;
                        match attribute {
                            HtmlAttribute::Class { values } => {
                                for value in values {
                                    add(core::mem::size_of::<String>())?;
                                    add(value.len())?;
                                }
                            }
                            HtmlAttribute::DataId { value } => add(value.len())?,
                            _ => return Err("UnexpectedCodeAttribute".into()),
                        }
                    }
                }
                _ => return Err("UnexpectedCodeMarkup".into()),
            }
        }
        let cost = u64::try_from(cost).map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
        b.charge(Resource::Work, cost).map_err(err)?;
        b.charge(Resource::AllocationUnits, cost).map_err(err)?;
        Ok(HtmlRequest {
            fragment: HtmlFragment {
                root: entry.markup.fragment.root,
                nodes: entry.markup.fragment.nodes.clone(),
            },
            slot: entry.markup.slot,
            policy: HtmlPolicy {
                classes: entry.markup.policy.classes.clone(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_page_code_rejects_another_page_and_accounts_repeated_copies() -> Result<(), String>
    {
        let compiled = crate::doc::source::compiled()?;
        let mut cases = Vec::new();
        for (name, source) in [
            (
                "a",
                r#"article en "A" body cons paragraph cons code Doc article en "One" body nil nil nil"#,
            ),
            (
                "b",
                r#"article en "B" body cons paragraph cons code Doc article en "Two" body nil nil nil"#,
            ),
        ] {
            let result = crate::doc::source::with_named_input(
                true,
                &compiled,
                source,
                name,
                "Article",
                |tree, profile, _, _| {
                    let store = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &store, &mut admission)
                            .map_err(err)?;
                    let document = lower::document(
                        tree.syntax(),
                        &compiled.doc.package.schema,
                        Category::Article,
                        profile.registry(),
                        &mut budget(),
                        &mut codec,
                    )
                    .map_err(err)?;
                    let prepared =
                        PreparedPageCode::prepare(&document, tree, profile, &mut budget())?;
                    Ok((document, prepared))
                },
            )?;
            cases.push(result);
        }
        let (document, prepared) = &cases[0];
        let embed = document.value.embeds.first().ok_or("embed missing")?;
        let foreign = cases[1]
            .0
            .value
            .embeds
            .first()
            .ok_or("other embed missing")?;
        assert_eq!(
            prepared.render(foreign, EmbedRef(0), &compiled.doc.registry, &mut budget()),
            Err("CodeInputMismatch".into())
        );
        assert_eq!(
            prepared.render(
                embed,
                EmbedRef(u64::MAX),
                &compiled.doc.registry,
                &mut budget()
            ),
            Err("CodeInputMissing".into())
        );
        let mut b = budget();
        let first = prepared.render(embed, EmbedRef(0), &compiled.doc.registry, &mut b)?;
        let usage = b.usage();
        let second = prepared.render(embed, EmbedRef(0), &compiled.doc.registry, &mut b)?;
        assert_eq!(first, second);
        assert!(b.usage().allocation_units > usage.allocation_units);
        assert!(b.usage().work > usage.work);
        for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
            let mut limits = budget().limits();
            if reason == StopReason::WorkLimit {
                limits.work = usage.work - 1;
            } else {
                limits.allocation_units = usage.allocation_units - 1;
            }
            let mut limited = Budget::new(limits);
            assert!(
                prepared
                    .render(embed, EmbedRef(0), &compiled.doc.registry, &mut limited)
                    .is_err()
            );
            assert_eq!(limited.poll(), Err(reason));
        }
        Ok(())
    }
}
