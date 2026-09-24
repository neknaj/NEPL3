//! Discover selected Doc/Sentence composition before resolving page names.
//! Unique meaning owners and structural occurrences remain separate tables.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    check::ForeignOccurrence,
    model::{DocumentSyntax, EmbedRef},
};
use nepl3_sentence_core::{lower::ForeignInlineForm, syntax::SentenceSyntax};
use nepl3_suite::adapters::{document::sentences, sentence::document_guests};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentId(usize);
impl DocumentId {
    pub fn index(self) -> usize {
        self.0
    }
}
enum Input<'a> {
    Root(&'a DocumentSyntax),
    Guest(DocumentSyntax),
}
impl Input<'_> {
    fn document(&self) -> &DocumentSyntax {
        match self {
            Self::Root(input) => input,
            Self::Guest(input) => input,
        }
    }
}
/// An occurrence inside one independently owned Sentence slot. The containing
/// Doc's occurrence table determines how often that slot appears in the page.
pub struct Guest {
    pub slot: EmbedRef,
    pub occurrence: document_guests::Occurrence,
    pub document: DocumentId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parent {
    pub document: DocumentId,
    pub slot: EmbedRef,
    pub embed: nepl3_sentence_core::model::EmbedRef,
}
pub struct Member<'a> {
    input: Input<'a>,
    parent: Option<Parent>,
    depth: u64,
    sentences: Vec<Option<SentenceSyntax>>,
    occurrences: Vec<ForeignOccurrence>,
    guests: Vec<Guest>,
}
impl Member<'_> {
    pub fn document(&self) -> &DocumentSyntax {
        self.input.document()
    }
    pub fn sentences(&self) -> &[Option<SentenceSyntax>] {
        &self.sentences
    }
    pub fn occurrences(&self) -> &[ForeignOccurrence] {
        &self.occurrences
    }
    pub fn guests(&self) -> &[Guest] {
        &self.guests
    }
    pub fn depth(&self) -> u64 {
        self.depth
    }
    pub fn parent(&self) -> Option<Parent> {
        self.parent
    }
}
pub struct Collected<'a> {
    members: Vec<Member<'a>>,
}
impl Collected<'_> {
    /// The first owner is the borrowed page root. Subsequent owners are unique
    /// per parent Sentence embed, not a flattened namespace occurrence list.
    pub fn members(&self) -> &[Member<'_>] {
        &self.members
    }
}
#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    InternalShape,
    Sentence {
        document: DocumentId,
        error: sentences::Error<E>,
    },
    Guests {
        document: DocumentId,
        slot: EmbedRef,
        error: document_guests::Error<E>,
    },
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Retain raw source owners for diagnostics without allocating after a stop.
/// Incomplete discovery grants no collection, namespace or output proof.
pub struct Failure<'a, E> {
    error: Error<E>,
    members: Vec<Member<'a>>,
}
impl<E: core::fmt::Debug> core::fmt::Debug for Failure<'_, E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DiscoveryFailure")
            .field("error", &self.error)
            .field("source_owners", &self.members.len())
            .finish()
    }
}
impl<E> Failure<'_, E> {
    pub fn cause(&self) -> &Error<E> {
        &self.error
    }
    pub fn document(&self, id: DocumentId) -> Option<&DocumentSyntax> {
        self.members.get(id.0).map(Member::document)
    }
    pub fn parent(&self, id: DocumentId) -> Option<Parent> {
        self.members.get(id.0).and_then(Member::parent)
    }
    pub fn owners(&self) -> impl Iterator<Item = (DocumentId, &DocumentSyntax, Option<Parent>)> {
        self.members
            .iter()
            .enumerate()
            .map(|(index, member)| (DocumentId(index), member.document(), member.parent()))
    }
}
impl<E> From<StopReason> for Failure<'_, E> {
    fn from(reason: StopReason) -> Self {
        Self {
            error: Error::Stopped(reason),
            members: Vec::new(),
        }
    }
}
fn push<T>(items: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        (core::mem::size_of::<T>() as u64).saturating_mul(2),
    )?;
    items
        .try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    items.push(value);
    Ok(())
}
/// Iteratively follow Doc -> Sentence -> explicitly selected Doc Inline edges.
/// Other guests remain in the owned Sentence/Doc model for their own adapters.
/// No guest is evaluated and no namespace or complete-output proof is granted.
pub fn collect<'a, C: FoundationValueCodec>(
    root: &'a DocumentSyntax,
    sentence_surface: &SchemaRef,
    doc_surface: &SchemaRef,
    forms: &[ForeignInlineForm<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Collected<'a>, Failure<'a, C::Error>> {
    b.poll()?;
    let mut members = Vec::new();
    let result = (|| {
        push(
            &mut members,
            Member {
                input: Input::Root(root),
                parent: None,
                depth: b.current_depth(),
                sentences: Vec::new(),
                occurrences: Vec::new(),
                guests: Vec::new(),
            },
            b,
        )?;
        let mut index = 0;
        while index < members.len() {
            b.charge(Resource::Work, 1)?;
            let depth = members[index].depth;
            let selected = b.with_depth_at_least(depth, |b| {
                sentences::collect(
                    members[index].document(),
                    sentence_surface,
                    forms,
                    registry,
                    codec,
                    b,
                )
                .map_err(|error| Error::Sentence {
                    document: DocumentId(index),
                    error,
                })
            })?;
            let (slots, occurrences) = selected.into_parts();
            let mut depths = Vec::new();
            for _ in &slots {
                push(&mut depths, 0u64, b)?;
            }
            for occurrence in &occurrences {
                b.charge(Resource::Work, 1)?;
                let item = &mut depths[occurrence.embed.0 as usize];
                *item = (*item).max(occurrence.depth);
            }
            let mut guests = Vec::new();
            for (slot, sentence) in slots.iter().enumerate() {
                b.charge(Resource::Work, 1)?;
                let Some(sentence) = sentence else {
                    continue;
                };
                let sentence_depth = depth.saturating_add(depths[slot]);
                let selected = b.with_depth_at_least(sentence_depth, |b| {
                    document_guests::collect(sentence, doc_surface, registry, codec, b).map_err(
                        |error| Error::Guests {
                            document: DocumentId(index),
                            slot: EmbedRef(slot as u64),
                            error,
                        },
                    )
                })?;
                let (documents, occurrences) = selected.into_parts();
                let mut guest_owners = Vec::new();
                for _ in &documents {
                    push(
                        &mut guest_owners,
                        None::<(u64, nepl3_sentence_core::model::EmbedRef)>,
                        b,
                    )?;
                }
                for occurrence in &occurrences {
                    b.charge(Resource::Work, 1)?;
                    let item = &mut guest_owners[occurrence.document.index()];
                    match item {
                        Some((depth, _)) => *depth = (*depth).max(occurrence.depth),
                        None => *item = Some((occurrence.depth, occurrence.embed)),
                    }
                }
                let start = members.len();
                for (document, owner) in documents.into_iter().zip(guest_owners) {
                    let (relative, embed) = owner.ok_or(Error::InternalShape)?;
                    push(
                        &mut members,
                        Member {
                            input: Input::Guest(document),
                            parent: Some(Parent {
                                document: DocumentId(index),
                                slot: EmbedRef(slot as u64),
                                embed,
                            }),
                            depth: sentence_depth.saturating_add(relative),
                            sentences: Vec::new(),
                            occurrences: Vec::new(),
                            guests: Vec::new(),
                        },
                        b,
                    )?;
                }
                for occurrence in occurrences {
                    let document = DocumentId(start + occurrence.document.index());
                    push(
                        &mut guests,
                        Guest {
                            slot: EmbedRef(slot as u64),
                            occurrence,
                            document,
                        },
                        b,
                    )?;
                }
            }
            members[index].sentences = slots;
            members[index].occurrences = occurrences;
            members[index].guests = guests;
            index += 1;
        }
        Ok(())
    })();
    let result = match b.poll() {
        Ok(()) => result,
        Err(reason) => Err(Error::Stopped(reason)),
    };
    match result {
        Ok(()) => Ok(Collected { members }),
        Err(error) => Err(Failure { error, members }),
    }
}
