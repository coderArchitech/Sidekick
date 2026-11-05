//! Extracts text from supported files (PRD CTX-2). Each parser returns pages so
//! chunks keep a page (PDF page, slide number, or sheet number) for citations.

use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::Event;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// 1-based page, slide or sheet number; `None` where the format has no pages (DOCX, TXT).
    pub number: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pdf,
    Docx,
    Pptx,
    Spreadsheet,
    Text,
    Image,
}

impl Kind {
    /// From the file extension. `None` means unsupported.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let ext = name.rsplit('.').next()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "pdf" => Self::Pdf,
            "docx" => Self::Docx,
            "pptx" => Self::Pptx,
            "xlsx" | "xlsm" | "xls" | "ods" => Self::Spreadsheet,
            "txt" | "md" | "markdown" | "csv" | "json" => Self::Text,
            "png" | "jpg" | "jpeg" | "webp" | "gif" => Self::Image,
            _ => return None,
        })
    }

    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            Self::Spreadsheet => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            }
            Self::Text => "text/plain",
            Self::Image => "image/*",
        }
    }
}

fn unsupported(name: &str) -> SidekickError {
    SidekickError::new(
        ErrorCode::UnsupportedFile,
        format!(
            "Sidekick can't read {name}. Supported: PDF, Word (.docx), PowerPoint (.pptx), Excel, text and Markdown."
        ),
    )
}

fn failed(name: &str, why: &str) -> SidekickError {
    tracing::warn!(target: "ingest", why, "parse failed");
    SidekickError::new(
        ErrorCode::ParseFailed,
        format!("Couldn't read {name}: {why}."),
    )
}

/// Parses a file's bytes. CPU-heavy: call from a blocking thread.
pub fn parse(name: &str, bytes: &[u8]) -> SidekickResult<(Kind, Vec<Page>)> {
    let kind = Kind::from_name(name).ok_or_else(|| unsupported(name))?;
    let pages = match kind {
        Kind::Pdf => pdf(name, bytes)?,
        Kind::Docx => docx(name, bytes)?,
        Kind::Pptx => pptx(name, bytes)?,
        Kind::Spreadsheet => spreadsheet(name, bytes)?,
        Kind::Text => vec![Page {
            number: None,
            text: String::from_utf8_lossy(bytes).into_owned(),
        }],
        Kind::Image => {
            return Err(SidekickError::new(
                ErrorCode::UnsupportedFile,
                "Images are read with a vision model, which arrives with screenshots. Add them as screenshots for now.",
            ));
        }
    };
    if pages.iter().all(|p| p.text.trim().is_empty()) {
        return Err(failed(
            name,
            "no text found (a scanned PDF needs OCR, which isn't supported yet)",
        ));
    }
    Ok((kind, pages))
}

fn pdf(name: &str, bytes: &[u8]) -> SidekickResult<Vec<Page>> {
    // pdf-extract can panic on malformed files; contain it.
    let pages = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(bytes))
        .map_err(|_| failed(name, "the PDF is damaged"))?
        .map_err(|_| failed(name, "the PDF is damaged or encrypted"))?;
    Ok(pages
        .into_iter()
        .enumerate()
        .map(|(i, text)| Page {
            number: u32::try_from(i + 1).ok(),
            text,
        })
        .collect())
}

fn zip_entry(name: &str, bytes: &[u8], entry: &str) -> SidekickResult<Option<String>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| failed(name, "it isn't a valid Office file"))?;
    let Ok(mut file) = archive.by_name(entry) else {
        return Ok(None);
    };
    let mut xml = String::new();
    file.read_to_string(&mut xml)
        .map_err(|_| failed(name, "the file is damaged"))?;
    Ok(Some(xml))
}

/// Text runs from Office XML: `text_tag` holds text, `para_tag` ends a line.
fn office_text(name: &str, xml: &str, text_tag: &[u8], para_tag: &[u8]) -> SidekickResult<String> {
    let mut reader = Reader::from_str(xml);
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader
            .read_event()
            .map_err(|_| failed(name, "the document XML is damaged"))?
        {
            Event::Start(e) if e.name().as_ref() == text_tag => in_text = true,
            Event::End(e) if e.name().as_ref() == text_tag => in_text = false,
            Event::End(e) if e.name().as_ref() == para_tag => out.push('\n'),
            Event::Empty(e) if e.name().as_ref() == b"w:tab" => out.push('\t'),
            Event::Empty(e) if e.name().as_ref() == b"w:br" || e.name().as_ref() == b"a:br" => {
                out.push('\n')
            }
            Event::Text(t) if in_text => {
                out.push_str(
                    &t.decode()
                        .map_err(|_| failed(name, "the document has invalid text"))?,
                );
            }
            Event::GeneralRef(r) if in_text => {
                let ent = String::from_utf8_lossy(&r);
                out.push_str(match ent.as_ref() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    _ => "",
                });
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

fn docx(name: &str, bytes: &[u8]) -> SidekickResult<Vec<Page>> {
    let xml = zip_entry(name, bytes, "word/document.xml")?
        .ok_or_else(|| failed(name, "it isn't a Word document"))?;
    Ok(vec![Page {
        number: None,
        text: office_text(name, &xml, b"w:t", b"w:p")?,
    }])
}

fn pptx(name: &str, bytes: &[u8]) -> SidekickResult<Vec<Page>> {
    let archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| failed(name, "it isn't a valid Office file"))?;
    let mut slides: Vec<u32> = archive
        .file_names()
        .filter_map(|f| {
            f.strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse()
                .ok()
        })
        .collect();
    if slides.is_empty() {
        return Err(failed(name, "it has no slides"));
    }
    slides.sort_unstable();
    slides
        .into_iter()
        .map(|n| {
            let xml =
                zip_entry(name, bytes, &format!("ppt/slides/slide{n}.xml"))?.unwrap_or_default();
            Ok(Page {
                number: Some(n),
                text: office_text(name, &xml, b"a:t", b"a:p")?,
            })
        })
        .collect()
}

fn spreadsheet(name: &str, bytes: &[u8]) -> SidekickResult<Vec<Page>> {
    use calamine::Reader as _;
    let mut book = calamine::open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|_| failed(name, "it isn't a readable spreadsheet"))?;
    let names = book.sheet_names();
    let mut pages = Vec::new();
    for (i, sheet) in names.iter().enumerate() {
        let Ok(range) = book.worksheet_range(sheet) else {
            continue;
        };
        let mut text = format!("Sheet: {sheet}\n");
        for row in range.rows() {
            let cells: Vec<String> = row.iter().map(ToString::to_string).collect();
            if cells.iter().any(|c| !c.is_empty()) {
                text.push_str(&cells.join("\t"));
                text.push('\n');
            }
        }
        pages.push(Page {
            number: u32::try_from(i + 1).ok(),
            text,
        });
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kinds_from_names() {
        assert_eq!(Kind::from_name("Spec v2.PDF"), Some(Kind::Pdf));
        assert_eq!(Kind::from_name("notes.md"), Some(Kind::Text));
        assert_eq!(Kind::from_name("deck.pptx"), Some(Kind::Pptx));
        assert_eq!(Kind::from_name("budget.xlsx"), Some(Kind::Spreadsheet));
        assert_eq!(Kind::from_name("photo.JPG"), Some(Kind::Image));
        assert_eq!(Kind::from_name("archive.zip"), None);
        assert_eq!(Kind::from_name("noextension"), None);
    }

    #[test]
    fn unsupported_and_empty_files_have_clear_errors() {
        assert_eq!(
            parse("a.zip", b"PK").unwrap_err().code,
            ErrorCode::UnsupportedFile
        );
        assert_eq!(
            parse("a.png", b"\x89PNG").unwrap_err().code,
            ErrorCode::UnsupportedFile
        );
        assert_eq!(
            parse("a.txt", b"   \n ").unwrap_err().code,
            ErrorCode::ParseFailed
        );
        assert_eq!(
            parse("a.docx", b"not a zip").unwrap_err().code,
            ErrorCode::ParseFailed
        );
        assert_eq!(
            parse("a.pdf", b"%PDF-1.4 garbage").unwrap_err().code,
            ErrorCode::ParseFailed
        );
    }

    #[test]
    fn reads_text_files() {
        let (kind, pages) = parse("notes.md", "# Agenda\n- Pricing".as_bytes()).unwrap();
        assert_eq!(kind, Kind::Text);
        assert_eq!(
            pages,
            vec![Page {
                number: None,
                text: "# Agenda\n- Pricing".into()
            }]
        );
    }
}
