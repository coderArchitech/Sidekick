//! Ingest pipeline tests with generated fixture files (Task T-017).

#![allow(clippy::unwrap_used, clippy::cast_possible_truncation)]

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sidekick_core::embed::EmbeddingProvider;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_gateway::providers::mock_embed::MockEmbedder;
use sidekick_ingest::parse::parse;
use sidekick_ingest::{DocStatus, Owner, Progress, Source, ingest, repo};
use sidekick_secrets::SecretString;
use sidekick_store::{Database, vectors};

// ---- Fixture builders --------------------------------------------------------

fn pdf(pages: &[String]) -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
    let mut kids = Vec::new();
    for text in pages {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![50.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(text.as_str())]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(
            dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => content_id },
        );
        kids.push(page_id.into());
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count, "Resources" => resources_id, "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()] }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

fn zip_of(files: &[(&str, String)]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, body) in files {
            w.start_file(*name, opts).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }
    buf.into_inner()
}

fn docx(paragraphs: &[&str]) -> Vec<u8> {
    let body: String = paragraphs
        .iter()
        .map(|p| format!("<w:p><w:r><w:t>{p}</w:t></w:r></w:p>"))
        .collect();
    zip_of(&[(
        "word/document.xml",
        format!(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
        ),
    )])
}

fn pptx(slides: &[&str]) -> Vec<u8> {
    let files: Vec<(String, String)> = slides
        .iter()
        .enumerate()
        .map(|(i, t)| {
            (
                format!("ppt/slides/slide{}.xml", i + 1),
                format!(r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>{t}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#),
            )
        })
        .collect();
    zip_of(
        &files
            .iter()
            .map(|(n, b)| (n.as_str(), b.clone()))
            .collect::<Vec<_>>(),
    )
}

fn xlsx(rows: &[[&str; 2]]) -> Vec<u8> {
    let cells: String = rows
        .iter()
        .enumerate()
        .map(|(r, [a, b])| {
            let n = r + 1;
            format!(r#"<row r="{n}"><c r="A{n}" t="inlineStr"><is><t>{a}</t></is></c><c r="B{n}" t="inlineStr"><is><t>{b}</t></is></c></row>"#)
        })
        .collect();
    zip_of(&[
        ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.into()),
        ("_rels/.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.into()),
        ("xl/workbook.xml", r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Pricing" sheetId="1" r:id="rId1"/></sheets></workbook>"#.into()),
        ("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.into()),
        ("xl/worksheets/sheet1.xml", format!(r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{cells}</sheetData></worksheet>"#)),
    ])
}

// ---- Parsers -----------------------------------------------------------------

#[test]
fn pdf_pages_keep_their_numbers() {
    let (_, pages) = parse(
        "spec.pdf",
        &pdf(&[
            "Rate limits".into(),
            "Pricing tiers".into(),
            "Support".into(),
        ]),
    )
    .unwrap();
    let got: Vec<(Option<u32>, String)> = pages
        .iter()
        .map(|p| (p.number, p.text.trim().to_owned()))
        .collect();
    assert_eq!(
        got,
        [
            (Some(1), "Rate limits".into()),
            (Some(2), "Pricing tiers".into()),
            (Some(3), "Support".into())
        ]
    );
}

#[test]
fn docx_paragraphs_become_lines() {
    let (_, pages) = parse("brief.docx", &docx(&["Agenda", "Pricing &amp; terms"])).unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].number, None);
    assert_eq!(pages[0].text, "Agenda\nPricing & terms\n");
}

#[test]
fn pptx_slides_are_pages() {
    let (_, pages) = parse("deck.pptx", &pptx(&["Q3 results", "Roadmap"])).unwrap();
    let got: Vec<(Option<u32>, &str)> = pages.iter().map(|p| (p.number, p.text.trim())).collect();
    assert_eq!(got, [(Some(1), "Q3 results"), (Some(2), "Roadmap")]);
}

#[test]
fn xlsx_rows_are_tab_separated_per_sheet() {
    let (_, pages) = parse("budget.xlsx", &xlsx(&[["Tier", "Price"], ["Pro", "49"]])).unwrap();
    assert_eq!(pages[0].number, Some(1));
    assert_eq!(pages[0].text, "Sheet: Pricing\nTier\tPrice\nPro\t49\n");
}

// ---- Pipeline ----------------------------------------------------------------

fn db() -> Arc<Mutex<Database>> {
    let db = Database::open_in_memory(&SecretString::from("c".repeat(64))).unwrap();
    db.conn()
        .execute_batch(
            "INSERT INTO sessions (id, title, status, stt_provider, llm_provider, llm_model, created_at)
             VALUES ('s1', 'Review', 'draft', 'deepgram', 'anthropic', 'm', 1), ('s2', 'Other', 'draft', 'deepgram', 'anthropic', 'm', 1);",
        )
        .unwrap();
    Arc::new(Mutex::new(db))
}

type Log = Arc<Mutex<Vec<Progress>>>;

fn recorder() -> (Log, Arc<dyn Fn(Progress) + Send + Sync>) {
    let log: Log = Arc::default();
    let l = Arc::clone(&log);
    (log, Arc::new(move |p| l.lock().unwrap().push(p)))
}

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn count(db: &Arc<Mutex<Database>>, sql: &str) -> i64 {
    db.lock()
        .unwrap()
        .conn()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}

#[tokio::test]
async fn fifty_page_pdf_ingests_under_30_seconds_with_progress() {
    let dir = tempfile::tempdir().unwrap();
    let pages: Vec<String> = (1..=50)
        .map(|i| {
            format!(
                "Page {i}: the Pro tier rate limit is {} requests per minute for region {i}.",
                500 + i
            )
        })
        .collect();
    let path = write(dir.path(), "spec.pdf", &pdf(&pages));
    let db = db();
    let (log, progress) = recorder();
    let started = Instant::now();
    let id = ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::File(path),
        Arc::new(MockEmbedder),
        None,
        progress,
    )
    .await
    .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "CTX-3: {:?}",
        started.elapsed()
    );

    let statuses: Vec<DocStatus> = log.lock().unwrap().iter().map(|p| p.status).collect();
    assert_eq!(statuses.first(), Some(&DocStatus::Queued));
    assert!(statuses.contains(&DocStatus::Parsing) && statuses.contains(&DocStatus::Embedding));
    assert_eq!(statuses.last(), Some(&DocStatus::Ready));
    let percents: Vec<u8> = log.lock().unwrap().iter().map(|p| p.percent).collect();
    assert!(
        percents.windows(2).all(|w| w[0] <= w[1]),
        "progress only goes up: {percents:?}"
    );

    let info = repo::get(&db.lock().unwrap(), &id).unwrap();
    assert_eq!(
        (info.status, info.page_count, info.chunk_count),
        (DocStatus::Ready, Some(50), 50)
    );
    assert_eq!(count(&db, "SELECT count(*) FROM chunk_vectors"), 50);
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH 'region'"
        ),
        50,
        "keyword index filled"
    );
    let page_of_region_7: i64 = db
        .lock()
        .unwrap()
        .conn()
        .query_row(
            "SELECT page FROM chunks WHERE text LIKE '%region 7.%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(page_of_region_7, 7, "page attribution");
}

#[tokio::test]
async fn the_same_file_twice_is_not_embedded_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "notes.md", b"# Agenda\nPricing and rate limits");
    let db = db();
    let (_, p) = recorder();
    let a = ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::File(path.clone()),
        Arc::new(MockEmbedder),
        None,
        p.clone(),
    )
    .await
    .unwrap();
    let b = ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::File(path.clone()),
        Arc::new(MockEmbedder),
        None,
        p.clone(),
    )
    .await
    .unwrap();
    assert_eq!(a, b);
    assert_eq!(count(&db, "SELECT count(*) FROM documents"), 1);
    // A different session gets its own copy.
    let c = ingest(
        Arc::clone(&db),
        Owner::Session("s2".into()),
        Source::File(path),
        Arc::new(MockEmbedder),
        None,
        p,
    )
    .await
    .unwrap();
    assert_ne!(a, c);
}

#[tokio::test]
async fn deleting_a_document_removes_chunks_vectors_and_keywords() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(
        dir.path(),
        "deck.pptx",
        &pptx(&["Quarterly revenue", "Hiring plan"]),
    );
    let db = db();
    let (_, p) = recorder();
    let id = ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::File(path),
        Arc::new(MockEmbedder),
        None,
        p,
    )
    .await
    .unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM chunk_vectors"), 2);
    repo::delete(&db.lock().unwrap(), &id).unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM chunks"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM chunk_vectors"), 0);
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH 'revenue'"
        ),
        0
    );
}

struct FailingEmbedder;

#[async_trait::async_trait]
impl EmbeddingProvider for FailingEmbedder {
    fn id(&self) -> &'static str {
        "failing"
    }
    fn model(&self) -> &str {
        "failing"
    }
    fn dims(&self) -> usize {
        8
    }
    async fn embed(&self, _: &[String], _: Option<&SecretString>) -> SidekickResult<Vec<Vec<f32>>> {
        Err(SidekickError::new(
            ErrorCode::KeyInvalid,
            "OpenAI rejected the API key.",
        ))
    }
}

#[tokio::test]
async fn failures_are_stored_on_the_document_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let db = db();
    let (log, p) = recorder();
    let path = write(dir.path(), "brief.docx", &docx(&["Agenda"]));
    let err = ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::File(path),
        Arc::new(FailingEmbedder),
        None,
        p.clone(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::KeyInvalid);
    let last = log.lock().unwrap().last().cloned().unwrap();
    assert_eq!(
        (last.status, last.error.as_deref()),
        (DocStatus::Failed, Some("OpenAI rejected the API key."))
    );
    let docs = repo::list_for_session(&db.lock().unwrap(), "s1").unwrap();
    assert_eq!(
        (docs[0].status, docs[0].error.as_deref()),
        (DocStatus::Failed, Some("OpenAI rejected the API key."))
    );

    // Corrupted file: PARSE_FAILED, also recorded.
    let bad = write(dir.path(), "broken.pdf", b"%PDF-1.4 not really");
    let err = ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::File(bad),
        Arc::new(MockEmbedder),
        None,
        p.clone(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::ParseFailed);
    // Unsupported type: rejected before a document row is made.
    let zip = write(dir.path(), "archive.zip", b"PK");
    let err = ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::File(zip),
        Arc::new(MockEmbedder),
        None,
        p,
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::UnsupportedFile);
}

#[tokio::test]
async fn web_pages_are_ingested_as_text() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(path("/docs/limits"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("<html><head><title>Limits</title></head><body><p>Pro tier: 600 requests per minute.</p></body></html>", "text/html"))
        .mount(&server)
        .await;
    let db = db();
    let (_, p) = recorder();
    let url = format!("{}/docs/limits", server.uri());
    let id = ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::Url(url),
        Arc::new(MockEmbedder),
        None,
        p.clone(),
    )
    .await
    .unwrap();
    let info = repo::get(&db.lock().unwrap(), &id).unwrap();
    assert_eq!((info.name.as_str(), info.chunk_count), ("Limits", 1));
    let text: String = db
        .lock()
        .unwrap()
        .conn()
        .query_row("SELECT text FROM chunks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(text, "Pro tier: 600 requests per minute.");
    let bad = ingest(
        db,
        Owner::Session("s1".into()),
        Source::Url("file:///etc/passwd".into()),
        Arc::new(MockEmbedder),
        None,
        p,
    )
    .await
    .unwrap_err();
    assert_eq!(bad.code, ErrorCode::UnsupportedFile);
}

#[tokio::test]
async fn vector_search_finds_the_right_page() {
    let dir = tempfile::tempdir().unwrap();
    let pages = vec![
        "Lunch menu and cafeteria hours".to_owned(),
        "The Pro tier rate limit is 600 requests per minute".to_owned(),
    ];
    let db = db();
    let (_, p) = recorder();
    ingest(
        Arc::clone(&db),
        Owner::Session("s1".into()),
        Source::File(write(dir.path(), "spec.pdf", &pdf(&pages))),
        Arc::new(MockEmbedder),
        None,
        p,
    )
    .await
    .unwrap();
    let q = sidekick_gateway::providers::mock_embed::embed_one("What is the Pro tier rate limit?");
    let hits = vectors::nearest(&db.lock().unwrap(), &vectors::session_owner("s1"), &q, 1).unwrap();
    let page: i64 = db
        .lock()
        .unwrap()
        .conn()
        .query_row("SELECT page FROM chunks WHERE id = ?1", [hits[0].0], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(page, 2);
}
