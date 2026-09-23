use nepl3_doc_core::{check::Category, print::PrintEntry};
pub(super) fn entry(value: PrintEntry) -> (&'static str, Category) {
    match value {
        PrintEntry::Article => ("Article", Category::Article),
        PrintEntry::Body => ("Body", Category::Body),
        PrintEntry::Block => ("Block", Category::Block),
        PrintEntry::Flow => ("Flow", Category::Flow),
        PrintEntry::Sentence => ("Sentence", Category::Sentence),
        PrintEntry::Inline => ("Inline", Category::Inline),
        PrintEntry::Variant => ("Variant", Category::Variant),
        PrintEntry::Row => ("Row", Category::Row),
        PrintEntry::ListItem => ("ListItem", Category::ListItem),
        PrintEntry::Alignment => ("Alignment", Category::Alignment),
        PrintEntry::ListStyle => ("ListStyle", Category::ListStyle),
        PrintEntry::Check => ("Check", Category::Check),
        PrintEntry::LinkTarget => ("LinkTarget", Category::Target),
        PrintEntry::Asset => ("Asset", Category::Asset),
        PrintEntry::OptionalRow => ("OptionalRow", Category::OptionalRow),
        PrintEntry::OptionalSentence => ("OptionalSentence", Category::OptionalSentence),
        PrintEntry::OptionalText => ("OptionalText", Category::OptionalText),
        PrintEntry::MathGuest => ("MathGuest", Category::MathGuest),
        PrintEntry::CircuitGuest => ("CircuitGuest", Category::CircuitGuest),
        PrintEntry::Guest => ("Guest", Category::Guest),
    }
}
