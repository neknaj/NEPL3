//! Private ownership transfer shared by native render adapters. This does not
//! create render/admission evidence or expose mutable prepared documents.
use super::*;
use crate::doc::export::pages;
use nepl3_doc_core::pages::{FileBytes, PageDocument, PageFile, PageRegistration, PageSet};
use nepl3_doc_html::{RenderOptions, pages::PagesHtmlRequest};

pub(in crate::doc::export::pages) struct Owned<'a> {
    pub(in crate::doc::export::pages) compiled: &'a Compiled,
    pub(in crate::doc::export::pages) request: PagesHtmlRequest,
    pub(in crate::doc::export::pages) code: Vec<PreparedPageCode>,
    pub(in crate::doc::export::pages) origins: Vec<serde_json::Value>,
    pub(in crate::doc::export::pages) file_origins: Vec<serde_json::Value>,
    pub(in crate::doc::export::pages) aliases: Vec<pages::aliases::PageAliases>,
    pub(in crate::doc::export::pages) evidence: serde_json::Value,
    pub(in crate::doc::export::pages) preparation: Vec<PageAttempt>,
}
pub(in crate::doc::export::pages) struct Failure {
    pub(in crate::doc::export::pages) error: String,
    pub(in crate::doc::export::pages) usage: Usage,
    pub(in crate::doc::export::pages) preparation: Vec<PageAttempt>,
}
impl<'a> PreparedPages<'a> {
    pub(in crate::doc::export::pages) fn into_owned(
        self,
        options: RenderOptions,
    ) -> Result<(Owned<'a>, Running), Box<Failure>> {
        let Self { mut data, account } = self;
        match build(&mut data, account.usage(), options) {
            Ok(owned) => Ok((owned, account)),
            Err(error) => Err(Box::new(Failure {
                error,
                usage: account.usage(),
                preparation: data.attempts,
            })),
        }
    }
}
fn observation(value: &Observation) -> serde_json::Value {
    serde_json::json!({
        "stage":format!("{:?}", value.stage),
        "requested":pages::resources::limits(value.requested),
        "effective":pages::resources::limits(value.effective),
        "usage":pages::resources::usage(value.usage),
        "stop":value.stop.map(|s|format!("{s:?}")),
        "failed":value.failed,"accounting_rejected":value.accounting_rejected,
        "profile":value.profile.map(pages::digest_hex),
    })
}
pub(super) fn build<'a>(
    data: &mut Data<'a>,
    usage: Usage,
    options: RenderOptions,
) -> Result<Owned<'a>, String> {
    let mut documents = Vec::new();
    let mut code = Vec::new();
    let mut origins = Vec::new();
    documents
        .try_reserve_exact(data.prepared.len())
        .map_err(|_| "HostAllocationFailure")?;
    code.try_reserve_exact(data.prepared.len())
        .map_err(|_| "HostAllocationFailure")?;
    origins
        .try_reserve_exact(data.prepared.len())
        .map_err(|_| "HostAllocationFailure")?;
    for (index, ((entry, source), page)) in std::mem::take(&mut data.inputs)
        .into_iter()
        .zip(std::mem::take(&mut data.prepared))
        .enumerate()
    {
        let successful = data
            .attempts
            .iter()
            .rev()
            .find(|a| a.page_index == index)
            .ok_or("PreparedEvidenceMissing")?;
        origins.push(serde_json::json!({
                "id":entry.id,"source":entry.source,"route":entry.route,"input":pages::input_path(&entry)?,
                "source_sha256":pages::digest_hex(Digest::of(source.as_bytes())),
                "operations":successful.stages.iter().map(observation).collect::<Vec<_>>(),
            }));
        documents.push(PageDocument {
            registration: PageRegistration {
                id: entry.id,
                source: entry.source,
                route: entry.route,
            },
            document: page.document,
        });
        code.push(page._code);
    }
    let mut files = Vec::new();
    let mut file_origins = Vec::new();
    files
        .try_reserve_exact(data.files.len())
        .map_err(|_| "HostAllocationFailure")?;
    file_origins
        .try_reserve_exact(data.files.len())
        .map_err(|_| "HostAllocationFailure")?;
    for (entry, bytes) in std::mem::take(&mut data.files) {
        let physical = pages::input_path(&entry)?;
        file_origins.push(serde_json::json!({
            "id":entry.id,"source":entry.source,"route":entry.route,"input":physical,
            "sha256":pages::digest_hex(Digest::of(&bytes)),"bytes":bytes.len()
        }));
        files.push(PageFile {
            registration: PageRegistration {
                id: entry.id,
                source: entry.source,
                route: entry.route,
            },
            content: FileBytes(bytes),
        });
    }
    let evidence = serde_json::json!({
        "attempts":data.attempts.iter().map(|a|serde_json::json!({
            "page_index":a.page_index,"stages":a.stages.iter().map(observation).collect::<Vec<_>>()
        })).collect::<Vec<_>>(),

        "cumulative_usage":pages::resources::usage(usage),
        "scope":"Immutable owned documents and page-bound Code evidence; not whole-PageSet validity.",
    });
    Ok(Owned {
        compiled: data.compiled,
        request: PagesHtmlRequest {
            set: PageSet {
                pages: documents,
                files,
            },
            options,
        },
        code,
        origins,
        file_origins,
        aliases: std::mem::take(&mut data.aliases),
        evidence,
        preparation: std::mem::take(&mut data.attempts),
    })
}
