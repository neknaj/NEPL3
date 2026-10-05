//! Static registered images for the explicit footnote page-set profile.
use super::*;
use nepl3_doc_core::{
    pages::PageSet,
    text::{AnnotationPolicy, PlainTextOutcome, PreparedText},
};

#[derive(Debug)]
pub struct ImageDependency {
    pub node: u64,
    pub asset_id: String,
    pub route: String,
    pub digest: Digest,
}

pub(super) struct Image {
    pub node: u64,
    pub alt: String,
    pub href: String,
}

pub(super) struct Files<'a> {
    set: &'a PageSet,
    digests: Vec<Digest>,
    used: Vec<bool>,
}

fn owned(s: &str, b: &mut Budget) -> Result<String, Error> {
    b.charge(Resource::Work, s.len() as u64)?;
    b.charge(Resource::AllocationUnits, s.len() as u64)?;
    Ok(s.into())
}

impl<'a> Files<'a> {
    pub(super) fn validate(set: &'a PageSet, b: &mut Budget) -> Result<Self, Error> {
        b.poll()?;
        if set.files.len() > 128 {
            return Err(Error::Invalid("SVG file count limit".into()));
        }
        let mut digests = Vec::new();
        let mut used = Vec::new();
        let mut total = 0usize;
        for file in &set.files {
            b.charge(Resource::Work, 1)?;
            let bytes = &file.content.0;
            total = total.checked_add(bytes.len()).ok_or(Error::OutputLimit)?;
            if bytes.len() > 262_144
                || total > 2_097_152
                || !file.registration.route.ends_with(".svg")
            {
                return Err(Error::Invalid("SVG file size or route".into()));
            }
            b.charge(Resource::Work, bytes.len() as u64)?;
            let svg = core::str::from_utf8(bytes)
                .map_err(|_| Error::Invalid("SVG requires UTF-8".into()))?;
            if !nepl3_markup::html::svg::validate(svg, b)? {
                return Err(Error::Invalid("unsupported static SVG".into()));
            }
            b.charge(Resource::Work, bytes.len() as u64)?;
            push(&mut digests, Digest::of(bytes), b)?;
            push(&mut used, false, b)?;
        }
        Ok(Self { set, digests, used })
    }

    pub(super) fn resolve(
        &mut self,
        document: &DocumentSyntax,
        prepared: &PreparedText<'_>,
        route: &str,
        node: u64,
        asset: &AssetRef,
        b: &mut Budget,
    ) -> Result<(Image, ImageDependency), Error> {
        let mut found = None;
        for (index, file) in self.set.files.iter().enumerate() {
            b.charge(
                Resource::Work,
                (file.registration.id.len() + asset.id.len()) as u64 + 1,
            )?;
            if file.registration.id == asset.id {
                found = Some(index);
                break;
            }
        }
        let index = found.ok_or(Error::NeedsResolution)?;
        let digest = self.digests[index];
        if asset.digest.is_some_and(|expected| expected != digest) {
            return Err(Error::Invalid("SVG digest mismatch".into()));
        }
        let alt = match usize::try_from(node)
            .ok()
            .and_then(|n| document.value.nodes.get(n))
            .map(|n| &n.kind)
        {
            Some(DocKind::Image { alt, .. } | DocKind::InlineImage { alt, .. }) => *alt,
            _ => return Err(Error::Unsupported { node }),
        };
        let reply = prepared
            .project(alt, AnnotationPolicy::BaseOnly, &[], b)
            .map_err(|_| Error::Invalid("prepared text limits mismatch".into()))?;
        let alt = match reply.outcome {
            PlainTextOutcome::Complete { text } => text,
            PlainTextOutcome::Stopped { reason } => return Err(Error::Stopped(reason)),
            PlainTextOutcome::Invalid { .. } => return Err(Error::NeedsResolution),
        };
        let target = &self.set.files[index].registration.route;
        let image = Image {
            node,
            alt,
            href: pages::relative(route, target, None, b)?,
        };
        let dependency = ImageDependency {
            node,
            asset_id: owned(&asset.id, b)?,
            route: owned(target, b)?,
            digest,
        };
        self.used[index] = true;
        Ok((image, dependency))
    }

    pub(super) fn finish(&self, b: &mut Budget) -> Result<(), Error> {
        for used in &self.used {
            b.charge(Resource::Work, 1)?;
            if !used {
                return Err(Error::Invalid("unused registered SVG".into()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_doc_core::pages::{FileBytes, PageFile, PageRegistration};

    fn files(count: usize, size: usize) -> PageSet {
        let head = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><!--"#;
        let tail = "--><path d=\"M0 0 L1 1\"/></svg>";
        let content = format!("{head}{}{tail}", " ".repeat(size - head.len() - tail.len()));
        PageSet {
            pages: vec![],
            files: (0..count)
                .map(|i| PageFile {
                    registration: PageRegistration {
                        id: format!("f{i}"),
                        source: format!("f{i}.svg"),
                        route: format!("assets/f{i}.svg"),
                    },
                    content: FileBytes(content.as_bytes().to_vec()),
                })
                .collect(),
        }
    }

    #[test]
    fn static_svg_admission_caps_are_inclusive() {
        // This isolates the size/count admission helper. Page references and
        // reachability are separately exercised by native page-set tests.
        for set in [files(128, 128), files(1, 262_144), files(8, 262_144)] {
            assert!(Files::validate(&set, &mut crate::doc::source::budget()).is_ok());
        }
        for set in [files(129, 128), files(1, 262_145), files(9, 262_144)] {
            assert!(matches!(
                Files::validate(&set, &mut crate::doc::source::budget()),
                Err(Error::Invalid(_))
            ));
        }
        let mut excess = files(8, 262_144);
        excess.files.push(PageFile {
            registration: PageRegistration {
                id: "extra".into(),
                source: "extra.svg".into(),
                route: "extra.svg".into(),
            },
            content: FileBytes(vec![b' ']),
        });
        assert!(matches!(
            Files::validate(&excess, &mut crate::doc::source::budget()),
            Err(Error::Invalid(message)) if message == "SVG file size or route"
        ));
    }
}
