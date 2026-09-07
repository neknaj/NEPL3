//! Dynamic head requests with a bounded projection of already read syntax.
mod check;
mod copy;
mod error;
mod model;
mod projection;
mod report;
mod window;
pub(crate) use check::compatible as compatible_windows;
pub use error::HeadError;
pub use model::*;
pub(crate) use projection::capture_completed;

pub(crate) fn signature(
    input: &nepl3_core::schema::TypeDescriptor,
    output: &nepl3_core::schema::TypeDescriptor,
    pure: bool,
) -> bool {
    use nepl3_core::schema::TypeDescriptor;
    match (input, output) {
        (TypeDescriptor::Named(input), TypeDescriptor::Named(output)) => {
            pure && input.package == output.package
                && input.revision == output.revision
                && input.package == "nepl3.engine"
                && input.revision == 1
                && input.name == "HeadCall"
                && output.name == "HeadReply"
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
