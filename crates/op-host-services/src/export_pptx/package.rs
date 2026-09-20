//! The OPC container: which parts a `.pptx` must contain, how they
//! point at each other, and the zip they are written into.
//!
//! A PowerPoint file is an Open Packaging Convention zip — a set of XML
//! parts plus a relationship graph that says which part is reached from
//! which. Nothing here is about how a slide LOOKS; the drawing lives in
//! the slide parts the caller hands in. This module owns the scaffolding
//! those slides need to be openable at all:
//!
//! ```text
//! [Content_Types].xml            what every part in the zip is
//! _rels/.rels                    package root → the presentation
//! ppt/presentation.xml           slide size + slide order
//! ppt/slideMasters/…             one empty master
//! ppt/slideLayouts/…             one blank layout
//! ppt/theme/theme1.xml           the theme the master is required to have
//! ppt/slides/slideN.xml          the caller's drawing
//! ppt/media/imageN.…             embedded bitmaps
//! ```
//!
//! **The master and the layout are deliberately empty.** A PowerPoint
//! deck normally inherits placeholders, backgrounds and text styles from
//! its layout, and anything inherited is something that could disagree
//! with the canvas. Every element this exporter emits is absolutely
//! positioned on the slide itself with its own explicit formatting, so
//! the layout's only job is to exist — the schema requires a slide to
//! have one, and requires that one to have a master, and requires that
//! master to name a theme.

use std::io::{Cursor, Write as _};

use crate::export::ExportError;

use super::units::emu;

/// One embedded bitmap, already decoded to bytes.
pub struct MediaFile {
    /// Lower-case file extension without the dot (`png` / `jpeg` / …).
    /// It drives both the part name and the `[Content_Types]` default,
    /// so an extension with no declared type would make the package
    /// unreadable — [`content_type_for`] is the single gate.
    pub ext: &'static str,
    pub bytes: Vec<u8>,
}

/// One finished slide: its `<p:spTree>` body plus the media it refers
/// to, in relationship-id order.
pub struct SlidePart {
    /// The board's authored name. PowerPoint shows it in the outline and
    /// the selection pane, so a slide stays identifiable as the board it
    /// came from after the deck leaves OpenPencil.
    pub name: String,
    /// The `<p:sp>` / `<p:pic>` elements, in paint order.
    pub shapes: String,
    /// Indices into the package media table. Position `i` here is the
    /// slide's `rId{i + 2}` — `rId1` is always the layout.
    pub media: Vec<usize>,
}

/// The `r:embed` id a slide uses for the `n`-th media file it carries.
///
/// Slide relationship ids are per-part, so two slides embedding the same
/// picture each get their own id pointing at the one shared media part.
pub fn slide_media_rel_id(index: usize) -> String {
    format!("rId{}", index + 2)
}

/// MIME type for an embedded media extension, or `None` when the
/// exporter has no declaration for it — the caller must then rasterise
/// instead of embedding an undeclarable part.
pub fn content_type_for(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "png" => "image/png",
        "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    })
}

/// Assemble the whole package. `slide_px` is the board size the deck
/// presents at; see the note in `export_pptx.rs` about why it comes from
/// the first slide rather than being normalised to 4:3 or 16:9.
pub fn build(
    slide_px: (f32, f32),
    slides: &[SlidePart],
    media: &[MediaFile],
) -> Result<Vec<u8>, ExportError> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let put = |zip: &mut zip::ZipWriter<Cursor<Vec<u8>>>, name: &str, bytes: &[u8]| {
        zip.start_file(name, options)
            .and_then(|()| zip.write_all(bytes).map_err(Into::into))
            .map_err(|e| ExportError::Write(e.to_string()))
    };

    put(
        &mut zip,
        "[Content_Types].xml",
        content_types(slides.len(), media).as_bytes(),
    )?;
    put(&mut zip, "_rels/.rels", ROOT_RELS.as_bytes())?;
    put(
        &mut zip,
        "ppt/presentation.xml",
        presentation(slide_px, slides.len()).as_bytes(),
    )?;
    put(
        &mut zip,
        "ppt/_rels/presentation.xml.rels",
        presentation_rels(slides.len()).as_bytes(),
    )?;
    put(
        &mut zip,
        "ppt/slideMasters/slideMaster1.xml",
        SLIDE_MASTER.as_bytes(),
    )?;
    put(
        &mut zip,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        MASTER_RELS.as_bytes(),
    )?;
    put(
        &mut zip,
        "ppt/slideLayouts/slideLayout1.xml",
        SLIDE_LAYOUT.as_bytes(),
    )?;
    put(
        &mut zip,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        LAYOUT_RELS.as_bytes(),
    )?;
    put(&mut zip, "ppt/theme/theme1.xml", THEME.as_bytes())?;

    for (i, slide) in slides.iter().enumerate() {
        let n = i + 1;
        put(
            &mut zip,
            &format!("ppt/slides/slide{n}.xml"),
            slide_xml(slide).as_bytes(),
        )?;
        put(
            &mut zip,
            &format!("ppt/slides/_rels/slide{n}.xml.rels"),
            slide_rels(slide, media).as_bytes(),
        )?;
    }
    for (i, file) in media.iter().enumerate() {
        put(
            &mut zip,
            &format!("ppt/media/image{}.{}", i + 1, file.ext),
            &file.bytes,
        )?;
    }

    let cursor = zip
        .finish()
        .map_err(|e| ExportError::Write(e.to_string()))?;
    Ok(cursor.into_inner())
}

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// The three namespaces every presentation part declares.
const NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"";

const REL_NS: &str = "xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"";

const REL_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn content_types(slide_count: usize, media: &[MediaFile]) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str(
        "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>",
    );
    // One `Default` per DISTINCT media extension present. A media part
    // whose extension has no declaration here is a part PowerPoint
    // cannot type, and it refuses the whole file rather than skipping
    // the picture — so the emitter only ever embeds extensions
    // `content_type_for` knows.
    let mut seen: Vec<&str> = Vec::new();
    for file in media {
        if seen.contains(&file.ext) {
            continue;
        }
        seen.push(file.ext);
        if let Some(mime) = content_type_for(file.ext) {
            out.push_str(&format!(
                "<Default Extension=\"{}\" ContentType=\"{mime}\"/>",
                file.ext
            ));
        }
    }
    let pml = "application/vnd.openxmlformats-officedocument.presentationml";
    out.push_str(&format!(
        "<Override PartName=\"/ppt/presentation.xml\" ContentType=\"{pml}.presentation.main+xml\"/>\
<Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"{pml}.slideMaster+xml\"/>\
<Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"{pml}.slideLayout+xml\"/>\
<Override PartName=\"/ppt/theme/theme1.xml\" \
ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>"
    ));
    for n in 1..=slide_count {
        out.push_str(&format!(
            "<Override PartName=\"/ppt/slides/slide{n}.xml\" ContentType=\"{pml}.slide+xml\"/>"
        ));
    }
    out.push_str("</Types>");
    out
}

const ROOT_RELS: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    "<Relationship Id=\"rId1\" ",
    "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" ",
    "Target=\"ppt/presentation.xml\"/>",
    "</Relationships>"
);

/// `ppt/presentation.xml` — the slide size and the slide ORDER.
///
/// The order lives in `sldIdLst`, not in the part names: a deck whose
/// slides were listed out of order would present out of order even
/// though `slide3.xml` is the third file in the zip. Both are emitted
/// in the same loop index so they cannot disagree.
fn presentation(slide_px: (f32, f32), slide_count: usize) -> String {
    let (w, h) = (emu(slide_px.0).max(1), emu(slide_px.1).max(1));
    let mut out = String::from(XML_DECL);
    out.push_str(&format!("<p:presentation {NS} saveSubsetFonts=\"1\">"));
    out.push_str(
        "<p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>",
    );
    out.push_str("<p:sldIdLst>");
    for i in 0..slide_count {
        // Slide ids are an arbitrary but stable key space starting at
        // 256 (PowerPoint's own first value); the r:id is what actually
        // resolves the part.
        out.push_str(&format!(
            "<p:sldId id=\"{}\" r:id=\"rId{}\"/>",
            256 + i,
            i + 2
        ));
    }
    out.push_str("</p:sldIdLst>");
    out.push_str(&format!("<p:sldSz cx=\"{w}\" cy=\"{h}\"/>"));
    // Notes pages keep the stock US-Letter portrait size; nothing this
    // exporter writes lands on one, but the element is required.
    out.push_str("<p:notesSz cx=\"6858000\" cy=\"9144000\"/>");
    out.push_str("</p:presentation>");
    out
}

fn presentation_rels(slide_count: usize) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str(&format!("<Relationships {REL_NS}>"));
    out.push_str(&format!(
        "<Relationship Id=\"rId1\" Type=\"{REL_BASE}/slideMaster\" \
Target=\"slideMasters/slideMaster1.xml\"/>"
    ));
    for i in 0..slide_count {
        out.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"{REL_BASE}/slide\" Target=\"slides/slide{}.xml\"/>",
            i + 2,
            i + 1
        ));
    }
    out.push_str(&format!(
        "<Relationship Id=\"rId{}\" Type=\"{REL_BASE}/theme\" Target=\"theme/theme1.xml\"/>",
        slide_count + 2
    ));
    out.push_str("</Relationships>");
    out
}

fn slide_xml(slide: &SlidePart) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str(&format!(
        "<p:sld {NS}><p:cSld name=\"{}\"><p:spTree>",
        op_util::xml_escape::escape_xml(&slide.name)
    ));
    out.push_str(EMPTY_TREE_HEAD);
    out.push_str(&slide.shapes);
    out.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>");
    out
}

fn slide_rels(slide: &SlidePart, media: &[MediaFile]) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str(&format!("<Relationships {REL_NS}>"));
    out.push_str(&format!(
        "<Relationship Id=\"rId1\" Type=\"{REL_BASE}/slideLayout\" \
Target=\"../slideLayouts/slideLayout1.xml\"/>"
    ));
    for (i, media_index) in slide.media.iter().enumerate() {
        let ext = media.get(*media_index).map(|f| f.ext).unwrap_or("png");
        out.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{REL_BASE}/image\" Target=\"../media/image{}.{ext}\"/>",
            slide_media_rel_id(i),
            media_index + 1
        ));
    }
    out.push_str("</Relationships>");
    out
}

/// The group-shape header every `<p:spTree>` opens with. `id="1"` is
/// reserved for it, which is why emitted shapes start numbering at 2.
const EMPTY_TREE_HEAD: &str = "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/>\
</p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>\
<a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>";

const SLIDE_MASTER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<p:sldMaster xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
    "xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">",
    "<p:cSld><p:bg><p:bgPr><a:solidFill><a:schemeClr val=\"bg1\"/></a:solidFill>",
    "<a:effectLst/></p:bgPr></p:bg><p:spTree>",
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>",
    "<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>",
    "<a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>",
    "</p:spTree></p:cSld>",
    "<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" ",
    "accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" ",
    "accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>",
    "<p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>",
    "</p:sldMaster>"
);

const MASTER_RELS: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    "<Relationship Id=\"rId1\" ",
    "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" ",
    "Target=\"../slideLayouts/slideLayout1.xml\"/>",
    "<Relationship Id=\"rId2\" ",
    "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" ",
    "Target=\"../theme/theme1.xml\"/>",
    "</Relationships>"
);

const SLIDE_LAYOUT: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<p:sldLayout xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
    "xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" ",
    "type=\"blank\" preserve=\"1\">",
    "<p:cSld name=\"Blank\"><p:spTree>",
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>",
    "<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>",
    "<a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>",
    "</p:spTree></p:cSld>",
    "<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>",
    "</p:sldLayout>"
);

const LAYOUT_RELS: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    "<Relationship Id=\"rId1\" ",
    "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" ",
    "Target=\"../slideMasters/slideMaster1.xml\"/>",
    "</Relationships>"
);

/// A complete, schema-valid theme.
///
/// Nothing the exporter emits references a theme colour, font or effect
/// style — every shape states its own — but `ECMA-376` requires the
/// master to have a theme part, and the theme's `fmtScheme` must carry
/// exactly three entries in each of its four style lists. This is the
/// smallest thing that satisfies both.
const THEME: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "name=\"OpenPencil\"><a:themeElements>",
    "<a:clrScheme name=\"OpenPencil\">",
    "<a:dk1><a:sysClr val=\"windowText\" lastClr=\"000000\"/></a:dk1>",
    "<a:lt1><a:sysClr val=\"window\" lastClr=\"FFFFFF\"/></a:lt1>",
    "<a:dk2><a:srgbClr val=\"44546A\"/></a:dk2>",
    "<a:lt2><a:srgbClr val=\"E7E6E6\"/></a:lt2>",
    "<a:accent1><a:srgbClr val=\"4472C4\"/></a:accent1>",
    "<a:accent2><a:srgbClr val=\"ED7D31\"/></a:accent2>",
    "<a:accent3><a:srgbClr val=\"A5A5A5\"/></a:accent3>",
    "<a:accent4><a:srgbClr val=\"FFC000\"/></a:accent4>",
    "<a:accent5><a:srgbClr val=\"5B9BD5\"/></a:accent5>",
    "<a:accent6><a:srgbClr val=\"70AD47\"/></a:accent6>",
    "<a:hlink><a:srgbClr val=\"0563C1\"/></a:hlink>",
    "<a:folHlink><a:srgbClr val=\"954F72\"/></a:folHlink>",
    "</a:clrScheme>",
    "<a:fontScheme name=\"OpenPencil\">",
    "<a:majorFont><a:latin typeface=\"Calibri Light\"/><a:ea typeface=\"\"/>",
    "<a:cs typeface=\"\"/></a:majorFont>",
    "<a:minorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/>",
    "<a:cs typeface=\"\"/></a:minorFont>",
    "</a:fontScheme>",
    "<a:fmtScheme name=\"OpenPencil\">",
    "<a:fillStyleLst>",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
    "</a:fillStyleLst>",
    "<a:lnStyleLst>",
    "<a:ln w=\"6350\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\">",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln>",
    "<a:ln w=\"12700\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\">",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln>",
    "<a:ln w=\"19050\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\">",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln>",
    "</a:lnStyleLst>",
    "<a:effectStyleLst>",
    "<a:effectStyle><a:effectLst/></a:effectStyle>",
    "<a:effectStyle><a:effectLst/></a:effectStyle>",
    "<a:effectStyle><a:effectLst/></a:effectStyle>",
    "</a:effectStyleLst>",
    "<a:bgFillStyleLst>",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
    "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
    "</a:bgFillStyleLst>",
    "</a:fmtScheme></a:themeElements></a:theme>"
);
