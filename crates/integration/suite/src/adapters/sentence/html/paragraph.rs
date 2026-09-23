//! Compose Sentence occurrences in a paragraph, retaining their separate source
//! owners and resolving HTML references over the complete paragraph.
use super::*;
use alloc::vec;

pub struct Placement<'a> {
    input: &'a SentenceSyntax,
    first_element: u64,
    elements: u64,
    origins: Vec<ElementOrigin>,
    foreign: Vec<ForeignPlacement>,
}
impl<'a> Placement<'a> {
    pub fn input(&self) -> &'a SentenceSyntax {
        self.input
    }
    pub fn first_element(&self) -> u64 {
        self.first_element
    }
    pub fn elements(&self) -> u64 {
        self.elements
    }
    pub fn origins(&self) -> &[ElementOrigin] {
        &self.origins
    }
    pub fn foreign(&self) -> &[ForeignPlacement] {
        &self.foreign
    }
}
pub struct Paragraph<'a> {
    markup: HtmlRequest,
    placements: Vec<Placement<'a>>,
}
impl<'a> Paragraph<'a> {
    pub fn markup(&self) -> &HtmlRequest {
        &self.markup
    }
    pub fn placements(&self) -> &[Placement<'a>] {
        &self.placements
    }
    /// Parts remain coupled to their original Sentence syntax. Mutating or
    /// inserting this markup requires validation of the complete destination.
    pub fn into_parts(self) -> (HtmlRequest, Vec<Placement<'a>>) {
        (self.markup, self.placements)
    }
}

/// Consume checked Sentence parts in display order. Repeated Sentence inputs
/// retain distinct placements. Missing fragment targets, duplicate IDs and
/// combined-depth exhaustion reject the entire result. No guest is re-executed.
pub fn compose<'a>(
    parts: impl IntoIterator<Item = PendingSentence<'a>>,
    b: &mut Budget,
) -> Result<Paragraph<'a>, Error> {
    b.poll()?;
    let mut nodes = Vec::new();
    b.charge(Resource::Nodes, 1)?;
    push(
        &mut nodes,
        HtmlNode::Element {
            tag: HtmlTag::P,
            attributes: vec![],
            children: vec![],
        },
        b,
    )?;
    let mut classes: Vec<String> = Vec::new();
    let mut placements = Vec::new();
    for part in parts {
        b.charge(Resource::Work, 1)?;
        let (input, part, mut origins, mut foreign) = part.into_parts();
        let offset = nodes.len() as u64;
        let elements = part.fragment.nodes.len() as u64;
        let add = |index: u64| offset.checked_add(index).ok_or(Error::InternalShape);
        let root = add(part.fragment.root)?;
        for mut node in part.fragment.nodes {
            if let HtmlNode::Element { children, .. } | HtmlNode::MathElement { children, .. } =
                &mut node
            {
                for child in children {
                    b.charge(Resource::Work, 1)?;
                    *child = add(*child)?;
                }
            }
            push(&mut nodes, node, b)?;
        }
        let Some(HtmlNode::Element { children, .. }) = nodes.first_mut() else {
            return Err(Error::InternalShape);
        };
        push(children, root, b)?;
        for class in part.policy.classes {
            let mut present = false;
            for prior in &classes {
                b.charge(Resource::Work, prior.len().min(class.len()) as u64 + 1)?;
                if prior == &class {
                    present = true;
                    break;
                }
            }
            if !present {
                push(&mut classes, class, b)?;
            }
        }
        for origin in &mut origins {
            b.charge(Resource::Work, 1)?;
            origin.element = add(origin.element)?;
        }
        for placement in &mut foreign {
            b.charge(Resource::Work, 1)?;
            placement.first_element = add(placement.first_element)?;
        }
        push(
            &mut placements,
            Placement {
                input,
                first_element: offset,
                elements,
                origins,
                foreign,
            },
            b,
        )?;
    }
    let markup = HtmlRequest {
        fragment: HtmlFragment { root: 0, nodes },
        slot: HtmlSlot::Block,
        policy: HtmlPolicy { classes },
    };
    validate(&markup.fragment, markup.slot, &markup.policy, b)?;
    Ok(Paragraph { markup, placements })
}
