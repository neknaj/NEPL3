//! Native prepared Doc occurrence association, not global/artifact admission.
use super::{
    MathDisplayHost,
    display::{
        Preference,
        generation::{self, OwnedGeneration, Setup, composite},
        process::{Config, reply::Reply},
        request::PreparedRequest,
    },
};
use nepl3_core::{
    budget::{Budget, StopReason},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_html::guests::Context;

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Prepare(super::Error<E>),
    Compose(composite::Error),
}
impl<E> From<StopReason> for Error<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
/// Original native Doc context remains paired through every attempt outcome.
/// ```compile_fail
/// fn mutate<E>(v: &mut nepl3_tools::doc::math::occurrence::Generated<'_, '_, '_, E>) { let _ = &mut v.context; }
/// ```
/// ```compile_fail
/// use nepl3_tools::doc::math::{occurrence::Generated, display::generation::OwnedGeneration};
/// fn pair<E>(context: nepl3_doc_html::guests::Context<'_>, generation: OwnedGeneration<'_, '_, E>) { let _ = Generated { context, generation }; }
/// ```
/// ```compile_fail
/// fn duplicate<E>(v: nepl3_tools::doc::math::occurrence::Generated<'_, '_, '_, E>) { v.clone(); }
/// ```
/// ```compile_fail
/// fn extract<E>(v: nepl3_tools::doc::math::occurrence::Generated<'_, '_, '_, E>) { v.into_parts(); }
/// ```
/// ```compile_fail
/// fn change<E>(v: &nepl3_tools::doc::math::occurrence::Generated<'_, '_, '_, E>) { v.generation().prepared().mathml().syntax.value.nodes.clear(); }
/// ```
#[must_use]
pub struct Generated<'d, 'c, 'r, E> {
    context: Context<'d>,
    generation: OwnedGeneration<'c, 'r, E>,
}
impl<'d, 'c, 'r, E> Generated<'d, 'c, 'r, E> {
    pub fn context(&self) -> &Context<'d> {
        &self.context
    }
    pub fn generation(&self) -> &OwnedGeneration<'c, 'r, E> {
        &self.generation
    }
    pub fn into_composite(
        self,
        b: &mut Budget,
    ) -> Result<Composition<'d, 'c, 'r, E>, composite::Error> {
        self.into_composite_with_policy(composite::CompletionPolicy::Strict, b)
    }
    pub fn into_composite_with_policy(
        self,
        policy: composite::CompletionPolicy,
        b: &mut Budget,
    ) -> Result<Composition<'d, 'c, 'r, E>, composite::Error> {
        let Self {
            context,
            generation,
        } = self;
        match generation.into_composite_with_policy(policy, b)? {
            composite::Composition::Ready(math) => {
                Ok(Composition::Ready(Composed { context, math }))
            }
            composite::Composition::Deferred(generation) => Ok(Composition::Deferred(Generated {
                context,
                generation,
            })),
        }
    }
}
#[must_use]
pub enum Composition<'d, 'c, 'r, E> {
    Ready(Composed<'d, 'c, 'r>),
    Deferred(Generated<'d, 'c, 'r, E>),
}
/// Local composition tied to its original native Doc input and occurrence.
/// This does not assert successful import or completion of the surrounding Doc.
/// ```compile_fail
/// fn mutate(v: &mut nepl3_tools::doc::math::occurrence::Composed<'_, '_, '_>) { let _ = &mut v.math; }
/// ```
/// ```compile_fail
/// fn extract(v: nepl3_tools::doc::math::occurrence::Composed<'_, '_, '_>) { v.into_parts(); }
/// ```
/// ```compile_fail
/// use nepl3_tools::doc::math::{occurrence::Composed, display::generation::composite::ComposedMath};
/// fn pair(context: nepl3_doc_html::guests::Context<'_>, math: ComposedMath<'_, '_>) { let _ = Composed { context, math }; }
/// ```
/// ```compile_fail
/// fn duplicate(v: nepl3_tools::doc::math::occurrence::Composed<'_, '_, '_>) { v.clone(); }
/// ```
/// ```compile_fail
/// fn change(v: &nepl3_tools::doc::math::occurrence::Composed<'_, '_, '_>) { v.math().math().markup.fragment.nodes.clear(); }
/// ```
pub struct Composed<'d, 'c, 'r> {
    context: Context<'d>,
    math: composite::ComposedMath<'c, 'r>,
}
impl<'d, 'c, 'r> Composed<'d, 'c, 'r> {
    pub(in crate::doc::math) fn into_import_parts(
        self,
    ) -> (Context<'d>, composite::import::Parts<'c, 'r>) {
        (self.context, self.math.into_import_parts())
    }

    pub fn context(&self) -> &Context<'d> {
        &self.context
    }
    pub fn math(&self) -> &composite::ComposedMath<'c, 'r> {
        &self.math
    }
}
/// Prepare only the document/node selected by the opaque traversal context.
/// Code/unsupported selections and host preparation failures remain typed
/// failures. The driver contract and incomplete artifact status are unchanged.
pub fn generate<'d, 'c, 'r, C, E, F>(
    context: Context<'d>,
    host: &mut MathDisplayHost<'_, C>,
    preference: Preference,
    setup: Setup<'c, 'r, '_>,
    driver: F,
    b: &mut Budget,
) -> Result<Generated<'d, 'c, 'r, E>, Error<C::Error>>
where
    C: FoundationValueCodec,
    F: for<'a> FnOnce(&'a PreparedRequest<'a>, Config<'a>, &mut Budget) -> Result<Reply<'a>, E>,
{
    b.poll()?;
    let prepared = host.prepare_node(context.document(), context.node(), preference, b);
    b.poll()?;
    let prepared = prepared.map_err(|e| match e {
        super::Error::Stopped(s) => Error::Stopped(s),
        e => Error::Prepare(e),
    })?;
    let generation = prepared
        .generate_owned(setup, driver, b)
        .map_err(|e| match e {
            generation::Error::Stopped(s) => Error::Stopped(s),
        })?;
    Ok(Generated {
        context,
        generation,
    })
}

/// Complete the exact selected Doc occurrence without configuring a renderer.
/// Missing Node/modules/KaTeX assets cannot block this explicit MathMLOnly path.
/// The scope is host bookkeeping; no visual CSS or external assets are emitted.
pub fn mathml_only<'d, C: FoundationValueCodec>(
    context: Context<'d>,
    host: &mut MathDisplayHost<'_, C>,
    scope: &str,
    b: &mut Budget,
) -> Result<Composed<'d, 'static, 'static>, Error<C::Error>> {
    b.poll()?;
    let prepared = host.prepare_node(
        context.document(),
        context.node(),
        Preference::MathMLOnly,
        b,
    );
    b.poll()?;
    let prepared = prepared.map_err(|e| match e {
        super::Error::Stopped(s) => Error::Stopped(s),
        e => Error::Prepare(e),
    })?;
    let math = prepared
        .into_mathml_composite(scope, b)
        .map_err(|e| match e {
            composite::Error::Stopped(s) => Error::Stopped(s),
            e => Error::Compose(e),
        })?;
    b.poll()?;
    Ok(Composed { context, math })
}
