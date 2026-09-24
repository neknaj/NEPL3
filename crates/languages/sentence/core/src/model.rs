//! Sentence content has no document or host-annotation role. Source/Origin and
//! syntax views are a separate boundary; raw arena values are not proofs.
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    syntax::ForeignClosure,
    value::{SchemaRef, TypedValue},
};

/// Opaque language-owned input. A selected adapter validates guest meaning;
/// common schema validation alone grants no rendering or execution proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InlineContent {
    Syntax { closure: Box<ForeignClosure> },
    Value { value: TypedValue },
}
impl InlineContent {
    pub fn schema(&self) -> &SchemaRef {
        match self {
            Self::Syntax { closure } => &closure.syntax.schema,
            Self::Value {
                value: TypedValue::Record(value),
            } => &value.schema,
            Self::Value {
                value: TypedValue::Variant(value),
            } => &value.schema,
        }
    }
    pub fn syntax(&self) -> Option<&ForeignClosure> {
        match self {
            Self::Syntax { closure } => Some(closure),
            Self::Value { .. } => None,
        }
    }
}
impl From<ForeignClosure> for InlineContent {
    fn from(closure: ForeignClosure) -> Self {
        Self::Syntax {
            closure: Box::new(closure),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SentenceRef(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineRef(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmbedRef(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Root {
    Sentence(SentenceRef),
    Inline(InlineRef),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    Sentence {
        inlines: Vec<InlineRef>,
    },
    Text {
        text: String,
    },
    Concat {
        inlines: Vec<InlineRef>,
    },
    Ruby {
        base: InlineRef,
        reading: InlineRef,
    },
    /// Annotation within a sentence, not a syntax-level `annotate` wrapper.
    InlineAnno {
        base: InlineRef,
        notes: Vec<InlineRef>,
    },
    Code {
        text: String,
    },
    Emphasis {
        inline: InlineRef,
    },
    Strong {
        inline: InlineRef,
    },
    Break,
    ExternalLink {
        uri: String,
        label: InlineRef,
    },
    /// Role and preparation are selected by an explicit foreign-inline adapter.
    /// Document namespaces and guest language names are not owned here.
    ForeignInline {
        syntax: EmbedRef,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SentenceValue {
    pub root: Root,
    pub nodes: Vec<Kind>,
    pub embeds: Vec<InlineContent>,
}

impl Kind {
    /// Ordered edges without allocating or cloning a child list.
    pub(crate) fn child(&self, index: usize) -> Option<InlineRef> {
        match self {
            Self::Sentence { inlines } | Self::Concat { inlines } => inlines.get(index).copied(),
            Self::Ruby { base, reading } => match index {
                0 => Some(*base),
                1 => Some(*reading),
                _ => None,
            },
            Self::InlineAnno { base, notes } => {
                if index == 0 {
                    Some(*base)
                } else {
                    notes.get(index - 1).copied()
                }
            }
            Self::Emphasis { inline } | Self::Strong { inline } => (index == 0).then_some(*inline),
            Self::ExternalLink { label, .. } => (index == 0).then_some(*label),
            Self::Text { .. } | Self::Code { .. } | Self::Break | Self::ForeignInline { .. } => {
                None
            }
        }
    }
}
