//! Strict development-host manifest selection, not a portable Doc codec.
use nepl3_doc_html::{ParallelMode, RenderOptions};
use serde::Deserialize;
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
pub enum Selection {
    Rows {},
    Columns {},
    Single {
        language: String,
        #[serde(default)]
        fallbacks: Vec<String>,
    },
}
impl Selection {
    /// Bounded strict host configuration, not a portable display proof.
    pub fn read(path: &std::path::Path) -> crate::Result<Self> {
        super::super::resources::read_configuration(path)
    }
}
impl Default for Selection {
    fn default() -> Self {
        Self::Rows {}
    }
}
impl From<Selection> for RenderOptions {
    fn from(value: Selection) -> Self {
        Self {
            parallel: match value {
                Selection::Rows {} => ParallelMode::Rows,
                Selection::Columns {} => ParallelMode::Columns,
                Selection::Single {
                    language,
                    fallbacks,
                } => ParallelMode::Single {
                    language,
                    fallbacks,
                },
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_input_is_strict_and_preserves_requested_options() -> Result<(), serde_json::Error> {
        for text in [
            r#"{"mode":"rows"}"#,
            r#"{"mode":"columns"}"#,
            r#"{"mode":"single","language":"en"}"#,
            r#"{"mode":"single","language":"fr","fallbacks":["ja","en"]}"#,
        ] {
            let _: Selection = serde_json::from_str(text)?;
        }
        for text in [
            r#"{}"#,
            r#"null"#,
            r#"{"mode":"Rows"}"#,
            r#"{"mode":"rows","language":"en"}"#,
            r#"{"mode":"columns","fallbacks":[]}"#,
            r#"{"mode":"single"}"#,
            r#"{"mode":"single","language":null}"#,
            r#"{"mode":"single","language":"en","fallbacks":null}"#,
            r#"{"mode":"single","language":"en","unknown":0}"#,
        ] {
            assert!(serde_json::from_str::<Selection>(text).is_err(), "{text}");
        }
        let selected: Selection =
            serde_json::from_str(r#"{"mode":"single","language":"FR","fallbacks":["ja","en"]}"#)?;
        let options: RenderOptions = selected.into();
        assert!(
            matches!(options.parallel,ParallelMode::Single{language,fallbacks} if language=="FR"&&fallbacks==["ja","en"])
        );
        Ok(())
    }
}
