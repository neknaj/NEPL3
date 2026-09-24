//! HTML member preparation for explicit page namespaces. The composing host
//! inserts each member at its actual guest occurrence before final page checks.
use super::*;
use core::convert::Infallible;
use nepl3_doc_core::{
    model::{DocEmbed, EmbedRef},
    pages::namespace::{CheckedPageNamespaces, Owner},
    prepare::DocRequirement,
};
use nepl3_markup::html::{HtmlRequest, check_part};

pub mod output;

struct Member<'a> {
    input: crate::prepare::PreparedRendering<'a>,
    links: Vec<(u64, HtmlHref)>,
}
/// Borrowed native plan and immutable options, with rendering state per exact
/// occurrence. No guest is invoked during preparation.
pub struct PreparedPages<'p, 'n, 'm, 'a> {
    checked: &'p CheckedPageNamespaces<'n, 'm, 'a>,
    options: &'a RenderOptions,
    pages: Vec<Vec<Member<'a>>>,
}
impl<'p, 'n, 'm, 'a> PreparedPages<'p, 'n, 'm, 'a> {
    pub fn checked(&self) -> &'p CheckedPageNamespaces<'n, 'm, 'a> {
        self.checked
    }
    pub fn options(&self) -> &'a RenderOptions {
        self.options
    }
}
#[derive(Debug)]
pub enum PreparationError<'p, 'a> {
    Stopped(StopReason),
    NeedsResolution {
        owner: Owner,
        requirement: &'p DocRequirement,
    },
    Local {
        owner: Owner,
        error: LocalPreparationError<'a, Infallible>,
    },
    InternalShape,
}
impl From<StopReason> for PreparationError<'_, '_> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Reuse the namespace's checked identities and page links. Assets and generic
/// guests retain unresolved requirements; supported guest roles require an
/// explicit adapter when rendering. The host must validate guest meaning,
/// including hidden language variants, before publishing complete page output.
pub fn prepare<'p, 'n, 'm, 'a>(
    checked: &'p CheckedPageNamespaces<'n, 'm, 'a>,
    options: &'a RenderOptions,
    b: &mut Budget,
) -> Result<PreparedPages<'p, 'n, 'm, 'a>, PreparationError<'p, 'a>> {
    b.poll()?;
    let mut pages = Vec::new();
    for _ in &checked.set().pages {
        push(&mut pages, Vec::new(), b)?;
    }
    for plan in checked.members() {
        b.charge(Resource::Work, 1)?;
        let owner = plan.owner();
        for requirement in plan.remaining() {
            b.charge(Resource::Work, 1)?;
            if !matches!(requirement, DocRequirement::Foreign { kind, .. } if crate::prepare::supports_foreign(*kind))
            {
                return Err(PreparationError::NeedsResolution { owner, requirement });
            }
        }
        let document = checked
            .document(owner)
            .ok_or(PreparationError::InternalShape)?;
        let input = crate::prepare::prepare_rendering(document, options, plan.document_digest(), b)
            .map_err(|error| match error {
                LocalPreparationError::Stopped(reason) => PreparationError::Stopped(reason),
                error => PreparationError::Local { owner, error },
            })?;
        let mut links = Vec::new();
        for link in plan.links() {
            b.charge(Resource::Work, 1)?;
            push(
                &mut links,
                (link.node, link_href(checked.set(), link, b)?),
                b,
            )?;
        }
        let members = usize::try_from(owner.page)
            .ok()
            .and_then(|page| pages.get_mut(page))
            .ok_or(PreparationError::InternalShape)?;
        if owner.member.0 != members.len() as u64 {
            return Err(PreparationError::InternalShape);
        }
        push(members, Member { input, links }, b)?;
    }
    Ok(PreparedPages {
        checked,
        options,
        pages,
    })
}

/// Structurally checked member output. Its references remain pending until the
/// host composes the entire page. Element coordinates are local to this member;
/// the host must offset both Doc origins and foreign placements on insertion.
pub struct PendingMember {
    owner: Owner,
    namespace_identity: Digest,
    output: RenderedWithForeign,
}
impl PendingMember {
    pub fn owner(&self) -> Owner {
        self.owner
    }
    pub fn namespace_identity(&self) -> Digest {
        self.namespace_identity
    }
    pub fn output(&self) -> &RenderedWithForeign {
        &self.output
    }
    pub fn into_parts(self) -> (Owner, Digest, RenderedWithForeign) {
        (self.owner, self.namespace_identity, self.output)
    }
}
#[derive(Debug)]
pub enum MemberError<E> {
    Selection(Owner),
    Render {
        owner: Owner,
        error: ForeignRenderError<E>,
    },
    Stopped(StopReason),
}
/// Render exactly one occurrence using its checked route/fragment resolution.
/// Callbacks receive the immutable guest owned by that member. Failure returns
/// no partial member and invokes no subsequent callback.
pub fn render_member<E>(
    prepared: &PreparedPages<'_, '_, '_, '_>,
    owner: Owner,
    adapter: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    b: &mut Budget,
) -> Result<PendingMember, MemberError<E>> {
    b.poll().map_err(MemberError::Stopped)?;
    let result = (|| {
        let member = usize::try_from(owner.page)
            .ok()
            .and_then(|page| prepared.pages.get(page))
            .and_then(|members| {
                usize::try_from(owner.member.0)
                    .ok()
                    .and_then(|member| members.get(member))
            })
            .ok_or(MemberError::Selection(owner))?;
        let output = crate::build::render_prepared_with_foreign(
            &member.input,
            &member.links,
            adapter,
            b,
            false,
        )
        .map_err(|error| MemberError::Render { owner, error })?;
        let markup = &output.fragment.markup;
        check_part(&markup.fragment, markup.slot, &markup.policy, b).map_err(|error| {
            MemberError::Render {
                owner,
                error: RenderError::from(error).into(),
            }
        })?;
        Ok(PendingMember {
            owner,
            namespace_identity: prepared.checked.identity(),
            output,
        })
    })();
    b.poll().map_err(MemberError::Stopped)?;
    result
}
