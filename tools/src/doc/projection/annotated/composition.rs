//! Borrow the same selected meaning owners used by page namespace resolution.
use super::*;
use crate::doc::export::pages::discovery;
use nepl3_sentence_core::{model::EmbedRef as SentenceEmbed, syntax::SentenceSyntax};

pub(super) struct Context<'a> {
    pub input: &'a discovery::Collected<'a>,
    /// Dense unique-owner order; node identifiers remain local to each owner.
    pub links: Vec<Vec<Option<String>>>,
    routes: Vec<Vec<Vec<Option<usize>>>>,
    owners: Vec<Option<std::sync::Arc<GuestOwner>>>,
}
impl<'a> Context<'a> {
    pub fn new(
        input: &'a discovery::Collected<'a>,
        page: u64,
        b: &mut Budget,
    ) -> Result<Self, Error> {
        let mut links = Vec::new();
        let mut routes = Vec::new();
        let mut owners: Vec<Option<std::sync::Arc<GuestOwner>>> = Vec::new();
        for member in input.members() {
            let owner = if let Some(parent) = member.parent() {
                b.charge(
                    Resource::AllocationUnits,
                    (core::mem::size_of::<GuestOwner>() + 2 * core::mem::size_of::<usize>()) as u64,
                )?;
                Some(std::sync::Arc::new(GuestOwner {
                    page,
                    parent: owners[parent.document.index()].clone(),
                    slot: parent.slot,
                    embed: parent.embed,
                }))
            } else {
                None
            };
            push(&mut owners, owner, b)?;
            let mut local_links = Vec::new();
            for _ in &member.document().value.nodes {
                push(&mut local_links, None, b)?;
            }
            push(&mut links, local_links, b)?;
            let mut slots = Vec::new();
            for sentence in member.sentences() {
                let mut row = Vec::new();
                if let Some(sentence) = sentence {
                    for _ in &sentence.value.embeds {
                        push(&mut row, None, b)?;
                    }
                }
                push(&mut slots, row, b)?;
            }
            for guest in member.guests() {
                b.charge(Resource::Work, 1)?;
                slots[guest.slot.0 as usize][guest.occurrence.embed.0 as usize] =
                    Some(guest.document.index());
            }
            push(&mut routes, slots, b)?;
        }
        Ok(Self {
            input,
            links,
            routes,
            owners,
        })
    }
    pub fn document(&self, member: usize) -> Result<&'a DocumentSyntax, Error> {
        self.input
            .members()
            .get(member)
            .map(|m| m.document())
            .ok_or(Error::NeedsResolution)
    }
    pub fn sentence(&self, member: usize, embed: EmbedRef) -> Result<&'a SentenceSyntax, Error> {
        self.input
            .members()
            .get(member)
            .and_then(|m| m.sentences().get(embed.0 as usize))
            .and_then(Option::as_ref)
            .ok_or(Error::NeedsResolution)
    }
    pub fn guest(
        &self,
        member: usize,
        slot: EmbedRef,
        embed: SentenceEmbed,
        b: &mut Budget,
    ) -> Result<usize, Error> {
        b.charge(Resource::Work, 1)?;
        self.routes
            .get(member)
            .and_then(|slots| slots.get(slot.0 as usize))
            .and_then(|row| row.get(embed.0 as usize))
            .copied()
            .flatten()
            .ok_or(Error::NeedsResolution)
    }
    pub fn link(&self, member: usize, node: u64, b: &mut Budget) -> Result<Option<&str>, Error> {
        b.charge(Resource::Work, 1)?;
        Ok(self
            .links
            .get(member)
            .and_then(|links| links.get(node as usize))
            .ok_or(Error::NeedsResolution)?
            .as_deref())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Located<'a> {
    owner: Option<&'a std::sync::Arc<GuestOwner>>,
    position: Position,
}
impl Located<'_> {
    pub fn node(self) -> u64 {
        self.position.node()
    }
    pub fn map(self, error: Error) -> Error {
        let error = self.position.map(error);
        match (self.owner, error) {
            (_, error @ Error::Stopped(_)) => error,
            (Some(owner), cause) => Error::Guest {
                owner: owner.clone(),
                cause: Box::new(cause),
            },
            (None, error) => error,
        }
    }
    pub fn text(self) -> Error {
        self.map(Error::Text { node: self.node() })
    }
    pub fn unsupported(self) -> Error {
        self.map(Error::Unsupported { node: self.node() })
    }
}

impl<'a> Annotated<'a, '_> {
    pub(super) fn located(&self, member: usize, position: Position) -> Located<'a> {
        Located {
            owner: self.composition.and_then(|c| c.owners[member].as_ref()),
            position,
        }
    }
    pub(super) fn document(&self, member: usize) -> Result<&'a DocumentSyntax, Error> {
        match self.composition {
            Some(context) => context.document(member),
            None if member == 0 => Ok(self.plain.doc),
            None => Err(Error::NeedsResolution),
        }
    }
    pub(super) fn sentence(
        &self,
        member: usize,
        embed: EmbedRef,
    ) -> Result<&'a SentenceSyntax, Error> {
        match self.composition {
            Some(context) => context.sentence(member, embed),
            None if member == 0 => self.plain.contents.get(embed),
            None => Err(Error::NeedsResolution),
        }
    }
}
