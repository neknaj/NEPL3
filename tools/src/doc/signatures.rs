//! Form catalog to typed Doc values. Surface syntax belongs to the Doc printer.
use crate::Result;
use nepl3_core::{
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore},
};
use nepl3_doc_core::{model::*, print};
use nepl3_wire::foundation::FoundationCodec;
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};
use std::{fmt, fs, marker::PhantomData, path::Path};

const LANGUAGES: [&str; 4] = ["Grammar", "Doc", "Math", "Circuit"];

// Catalog insertion order is presentation data. Do not sort categories or forms.
#[derive(Debug)]
struct Ordered<T>(Vec<(String, T)>);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Ordered<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Entries<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for Entries<T> {
            type Value = Ordered<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("ordered catalog entries")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    values.push(entry);
                }
                Ok(Ordered(values))
            }
        }
        deserializer.deserialize_map(Entries(PhantomData))
    }
}

#[derive(Deserialize)]
struct Catalog {
    categories: Ordered<Category>,
}
#[derive(Deserialize)]
struct Category {
    forms: Ordered<Form>,
    leaf: Option<String>,
}
#[derive(Deserialize)]
struct Form {
    kind: String,
    fields: Vec<Field>,
}
#[derive(Deserialize)]
struct Field {
    name: String,
    read: ReadType,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum ReadType {
    Name(String),
    List(ListType),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListType {
    list: Box<ReadType>,
}
impl ReadType {
    fn label(&self) -> Result<String> {
        match self {
            Self::Name(name) if !name.is_empty() => Ok(name.clone()),
            Self::List(list) => Ok(format!("List<{}>", list.list.label()?)),
            _ => Err("empty reader category".into()),
        }
    }
}

#[derive(Default)]
struct Builder {
    nodes: Vec<DocNode>,
}
impl Builder {
    fn node(&mut self, kind: DocKind) -> u64 {
        let id = self.nodes.len() as u64;
        self.nodes.push(DocNode {
            kind,
            locations: vec![],
            origin: None,
            span: None,
        });
        id
    }
    fn text(&mut self, text: &str) -> InlineRef {
        InlineRef(self.node(DocKind::Text { text: text.into() }))
    }
    fn code(&mut self, text: &str) -> InlineRef {
        InlineRef(self.node(DocKind::InlineCode { text: text.into() }))
    }
    fn ruby(&mut self, base: &str, reading: &str) -> InlineRef {
        let base = self.text(base);
        let reading = self.text(reading);
        InlineRef(self.node(DocKind::Ruby { base, reading }))
    }
    fn sentence(&mut self, inlines: Vec<InlineRef>) -> SentenceRef {
        SentenceRef(self.node(DocKind::Sentence { inlines }))
    }
    fn code_sentence(&mut self, text: &str) -> SentenceRef {
        let inline = self.code(text);
        self.sentence(vec![inline])
    }
    fn text_sentence(&mut self, text: &str) -> SentenceRef {
        let inline = self.text(text);
        self.sentence(vec![inline])
    }
    fn paragraph(&mut self, sentences: Vec<SentenceRef>) -> BlockRef {
        BlockRef(self.node(DocKind::Paragraph {
            items: sentences.into_iter().map(|s| FlowRef(s.0)).collect(),
        }))
    }
    fn body(&mut self, blocks: Vec<BlockRef>) -> BodyRef {
        BodyRef(self.node(DocKind::Body { blocks }))
    }
}

fn document(catalog: &Catalog, language: &str) -> Result<DocumentSyntax> {
    if !LANGUAGES.contains(&language) {
        return Err("unknown signature language".into());
    }
    let mut b = Builder::default();
    let title = vec![
        b.text(&format!("{language}：")),
        b.ruby("構文", "こうぶん"),
        b.text("signatureの"),
        b.ruby("全表", "ぜんぴょう"),
    ];
    let title = b.sentence(title);
    let intro = vec![
        b.text("この"),
        b.ruby("表", "ひょう"),
        b.text("は"),
        b.code("design/forms.json"),
        b.text("から"),
        b.ruby("生成", "せいせい"),
        b.text("した。"),
    ];
    let intro = b.sentence(intro);
    let notation = vec![
        b.code("List<T>"),
        b.text("は"),
        b.code("cons T List<T>"),
        b.text(" / "),
        b.code("nil"),
        b.text("、"),
        b.code("@"),
        b.text("は"),
        b.ruby("基礎", "きそ"),
        b.text("readerの"),
        b.ruby("識別", "しきべつ"),
        b.text("である。"),
    ];
    let notation = b.sentence(notation);
    let mut blocks = vec![b.paragraph(vec![intro, notation])];
    for (category, definition) in &catalog.categories.0 {
        if !category.starts_with(&format!("{language}/")) {
            continue;
        }
        let title = b.code_sentence(category);
        let spelling = vec![b.ruby("綴", "つづ"), b.text("り")];
        let spelling = b.sentence(spelling);
        let kind = b.text_sentence("kind");
        let children = vec![
            b.ruby("子", "こ"),
            b.text("（"),
            b.ruby("順序固定", "じゅんじょこてい"),
            b.text("）"),
        ];
        let children = b.sentence(children);
        let arity = b.text_sentence("arity");
        let header = RowRef(b.node(DocKind::Row {
            cells: vec![spelling, kind, children, arity],
        }));
        let mut rows = Vec::new();
        for (spelling, form) in &definition.forms.0 {
            let fields = form
                .fields
                .iter()
                .map(|field| Ok(format!("{}: {}", field.name, field.read.label()?)))
                .collect::<Result<Vec<_>>>()?
                .join(", ");
            let cells = vec![
                b.code_sentence(spelling),
                b.code_sentence(&format!("{language}.{}", form.kind)),
                if fields.is_empty() {
                    b.text_sentence("なし")
                } else {
                    b.code_sentence(&fields)
                },
                b.text_sentence(&form.fields.len().to_string()),
            ];
            rows.push(RowRef(b.node(DocKind::Row { cells })));
        }
        let table = BlockRef(b.node(DocKind::Table {
            columns: vec![
                Alignment::Left,
                Alignment::Left,
                Alignment::Left,
                Alignment::Right,
            ],
            header: Some(header),
            rows,
        }));
        let mut content = vec![table];
        if let Some(leaf) = &definition.leaf {
            let inlines = vec![
                b.ruby("葉", "は"),
                b.text("の"),
                b.ruby("認識規則", "にんしききそく"),
                b.text("："),
                b.code(leaf),
                b.text("。"),
            ];
            let sentence = b.sentence(inlines);
            content.push(b.paragraph(vec![sentence]));
        }
        let body = b.body(content);
        let id = format!(
            "category_{}",
            category
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        blocks.push(BlockRef(b.node(DocKind::Section { id, title, body })));
    }
    if blocks.len() == 1 {
        return Err("no categories for signature language".into());
    }
    let body = b.body(blocks);
    let root = DocRoot::Article(ArticleRef(b.node(DocKind::Article {
        language: "ja".into(),
        title,
        body,
    })));
    Ok(DocumentSyntax {
        value: DocValue {
            root,
            nodes: b.nodes,
            embeds: vec![],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    })
}

fn source(document: DocumentSyntax) -> Result<String> {
    let mut budget = super::source::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget),
        nepl3_doc_core::schema::descriptor(&mut budget),
    ] {
        let descriptor = descriptor.map_err(|e| format!("{e:?}"))?;
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("{e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("{e:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&registry, &sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    let request = print::PrintRequest {
        document,
        mode: print::PrintMode::Prefix,
        bindings: vec![],
        guests: vec![],
    };
    let reply =
        print::print(&request, &registry, &mut codec, &mut budget).map_err(|e| format!("{e:?}"))?;
    match reply.outcome {
        print::PrintOutcome::Complete { artifact } => Ok(artifact.text),
        failure => Err(format!("signature printing failed: {failure:?}").into()),
    }
}

pub fn run(root: &Path, write: bool) -> Result<()> {
    let catalog: Catalog = crate::json(root, "design/forms.json")?;
    for language in LANGUAGES {
        let output = root.join(format!(
            "doc/migration/generated/{}-signatures.nepld",
            language.to_lowercase()
        ));
        let data = source(document(&catalog, language)?)?;
        if write {
            fs::write(output, data)?;
        } else if fs::read_to_string(&output)? != data {
            return Err(format!("{} is stale; run signatures --write", output.display()).into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
