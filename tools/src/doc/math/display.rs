//! Generation inputs for one Doc Math occurrence, before any KaTeX execution.
//! Both representations use the same retained lowered source. No arbitrary
//! provider response is admitted here and no HTML/resource identity is certified.
use super::{Error, MathDisplayHost, RenderedMath};
use nepl3_core::{budget::Budget, value_codec::FoundationValueCodec};
use nepl3_doc_core::model::DocumentSyntax;
use nepl3_markup::mathml::Display;
use nepl3_math_core::check;
use nepl3_math_tex::{Occurrence, Unsupported};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Preference {
    KaTeXPreferred,
    MathMLOnly,
}

/// Why TeX was omitted, or the input still requiring a generation-side renderer.
/// Ready does not mean KaTeX ran or that its output passed markup validation.
#[derive(Debug, Eq, PartialEq)]
pub enum TexPreparation {
    MathMLOnly,
    Unsupported {
        node: u64,
        reason: Unsupported,
    },
    Ready {
        tex: String,
        occurrences: Vec<Occurrence>,
    },
}

pub struct PreparedDisplay {
    doc_node: u64,
    display: Display,
    mathml: RenderedMath,
    tex: TexPreparation,
}
impl PreparedDisplay {
    /// Node in the immutable Doc input used during preparation. This index is
    /// local to that document, not a document identity or execution receipt.
    pub fn doc_node(&self) -> u64 {
        self.doc_node
    }
    /// Mode selected by the validated Doc node, never by a renderer response.
    pub fn display(&self) -> Display {
        self.display
    }
    pub fn mathml(&self) -> &RenderedMath {
        &self.mathml
    }
    pub fn tex(&self) -> &TexPreparation {
        &self.tex
    }
    /// Move both results without copying the retained source or markup.
    /// Drops the Doc occurrence and display-mode metadata. Returned data is not
    /// an admission proof for a remote renderer.
    pub fn into_parts(self) -> (RenderedMath, TexPreparation) {
        (self.mathml, self.tex)
    }
}

impl<C: FoundationValueCodec> MathDisplayHost<'_, C> {
    /// Prepare independent MathML first, then optional structural TeX. The same
    /// caller budget spans both operations: capability failure permits MathML,
    /// whereas any stopped budget or other error fails the entire preparation.
    /// Cost is the sum of MathML generation, checked-expression validation and
    /// TeX generation; no second source parse or lower is performed.
    pub fn prepare_node(
        &mut self,
        document: &DocumentSyntax,
        node: u64,
        preference: Preference,
        budget: &mut Budget,
    ) -> Result<PreparedDisplay, Error<C::Error>> {
        let (mathml, display) = self.render_node_with_display(document, node, budget)?;
        prepare_rendered(node, mathml, display, preference, budget)
    }
}

// Shared only after the caller has selected and validated the exact Doc node.
pub(super) fn prepare_rendered<E>(
    node: u64,
    mathml: RenderedMath,
    display: Display,
    preference: Preference,
    budget: &mut Budget,
) -> Result<PreparedDisplay, Error<E>> {
    let tex = match preference {
        Preference::MathMLOnly => TexPreparation::MathMLOnly,
        Preference::KaTeXPreferred => {
            let checked =
                check::expression(&mathml.syntax.value, budget).map_err(|error| match error {
                    check::ShapeError::Stopped(reason) => Error::Stopped(reason),
                    error => Error::Check(error),
                })?;
            match nepl3_math_tex::render(&checked, budget) {
                Ok(rendered) => {
                    let (tex, occurrences) = rendered.into_parts();
                    TexPreparation::Ready { tex, occurrences }
                }
                Err(nepl3_math_tex::Error::Unsupported { node, reason }) => {
                    TexPreparation::Unsupported { node, reason }
                }
                Err(error) => {
                    budget.poll()?;
                    return Err(Error::Tex(error));
                }
            }
        }
    };
    budget.poll()?;
    Ok(PreparedDisplay {
        doc_node: node,
        display,
        mathml,
        tex,
    })
}

pub mod request;

pub mod process;

pub mod generation;
