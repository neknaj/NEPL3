use nepl3_core::{source::*, value::NdfValue};
use nepl3_doc_core::{
    model::*,
    print::{self, PrintMode, PrintRequest},
};
use nepl3_engine::profile::ResolvedParseProfile;
use nepl3_tools::doc::source::{budget, err, parse_source_as};
use nepl3_wire::foundation::FoundationCodec;
pub(super) fn guest_signature(
    bundle: &nepl3_core::syntax::SyntaxBundle,
) -> Vec<(String, Option<NdfValue>)> {
    bundle
        .nodes
        .iter()
        .map(|node| {
            (
                node.kind.clone(),
                node.token
                    .and_then(|id| bundle.tokens.get(id.0 as usize))
                    .map(|token| token.payload.clone()),
            )
        })
        .collect()
}
pub(super) fn host_request(
    doc: &DocumentSyntax,
    profile: &ResolvedParseProfile<'_>,
    mode: PrintMode,
) -> Result<PrintRequest, String> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
    let identity =
        print::identity(doc, profile.registry(), &mut codec, &mut budget()).map_err(err)?;
    let selected = [
        (GuestLanguage::Math, "Math", "Expr"),
        (GuestLanguage::Circuit, "Circuit", "Design"),
        (GuestLanguage::Grammar, "Grammar", "Root"),
        (GuestLanguage::Doc, "Doc", "Article"),
        (GuestLanguage::Sentence, "Sentence", "Sentence"),
        (GuestLanguage::Sentence, "Sentence", "Inline"),
    ];
    let bindings = selected
        .iter()
        .map(|(language, alias, category)| {
            Ok(print::GuestBinding {
                schema: profile
                    .language(alias, &mut budget())
                    .map_err(err)?
                    .schema
                    .clone(),
                category: (*category).into(),
                language: *language,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut guests = Vec::new();
    for target in &identity.guests {
        let embed = &doc.value.embeds[target.embed.0 as usize];
        let binding = bindings
            .iter()
            .find(|v| &v.schema == embed.schema() && v.category == embed.category())
            .ok_or("explicit guest binding")?;
        let (_, alias, category) = selected
            .iter()
            .find(|v| v.0 == binding.language && v.2 == binding.category)
            .ok_or("guest language")?;
        // Source retrieval alone is not accepted as printed source. The host
        // reparses it, checks the selected tree, and runs the real engine printer.
        let raw = print::original_guest_source(
            doc,
            target.embed,
            profile.registry(),
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?
        .ok_or("source-less guest in source-backed fixture")?;
        let source = SourceSnapshot::new(
            SourceId(format!("host-guest-{}", target.embed.0)),
            0,
            format!("memory:host-guest/{}", target.embed.0),
            raw.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(err)?;
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let tree = parse_source_as(&source, profile, alias, category, &mut b, &mut a)?;
        let proof = tree.validate(profile, &mut b, &mut a).map_err(err)?;
        if binding.language == GuestLanguage::Sentence {
            // Literal payloads retain the source identity of each parse.
            // Compare meaning after both source contexts pass validation.
            let before = nepl3_suite::adapters::document::sentence::lower(
                embed,
                &binding.schema,
                &[],
                profile.registry(),
                &mut codec,
                &mut b,
            )
            .map_err(err)?;
            let checked = tree
                .bundle
                .validate_with_sources(profile.registry(), &mut b, &mut a)
                .map_err(err)?;
            let after = nepl3_sentence_core::lower::presentation::sentence_with_foreign(
                &checked,
                &binding.schema,
                &[],
                profile.registry(),
                &mut codec,
                &mut b,
            )
            .map_err(err)?;
            assert_eq!(before.value, after.value);
        } else {
            assert_eq!(
                guest_signature(
                    &embed
                        .syntax()
                        .ok_or("expected source-backed syntax")?
                        .syntax
                        .bundle
                ),
                guest_signature(&tree.bundle)
            );
        }
        let reply = nepl3_engine::parse::print::source_tree(&proof, &mut b, &mut a).map_err(err)?;
        let nepl3_engine::parse::print::PrintOutcome::Complete(text) = reply.outcome else {
            return Err("host guest print stopped".into());
        };
        guests.push(print::PrintedGuest {
            document_digest: identity.document_digest,
            embed: target.embed,
            guest_digest: target.guest_digest,
            text,
        });
    }
    Ok(PrintRequest {
        document: doc.clone(),
        mode,
        bindings,
        guests,
    })
}
