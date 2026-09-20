//! Source-level guardrails for the browser-only Figma Worker boundary.
//!
//! IndexedDB and module Worker behavior still needs browser smoke, but these
//! tests keep the memory/loss-safety contract from silently regressing in a
//! native CI job.

fn source(name: &str) -> String {
    std::fs::read_to_string(format!("{}/src/{name}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|error| panic!("read {name}: {error}"))
}

#[test]
fn raw_figma_file_goes_to_an_isolated_module_worker() {
    let js = source("figma_temp_worker.js");
    assert!(js.contains("new Worker(objectUrl, { type: 'module'"));
    assert!(js.contains("file.arrayBuffer()"));
    assert!(js.contains("new wasm.FigmaTempWriter(bytes, fileName)"));
    assert!(js.contains("worker.terminate()"));
    assert!(
        !js.contains("FileReader"),
        "the Worker path must not copy the raw file through main WASM"
    );
}

#[test]
fn indexeddb_commit_marker_is_written_after_pages_and_complete_document() {
    let js = source("figma_temp_worker.js");
    let pending_write = js
        .find("await put(db, MANIFESTS_STORE, sessionId, pendingManifest)")
        .expect("pending lease");
    let page_write = js
        .find("new Blob([pageJson], { type: 'application/json' })")
        .expect("per-page record");
    let skeleton_write = js
        .find("new Blob([skeletonJson], { type: 'application/json' })")
        .expect("document skeleton record");
    let image_tables_write = js
        .find("new Blob([imageTablesJson], { type: 'application/json' })")
        .expect("shared image tables record");
    let writer_free = js.find("writer.free();").expect("worker payload release");
    let document_write = js
        .find("new Blob([fullDocumentJson], { type: 'application/json' })")
        .expect("complete canonical record");
    let manifest_write = js
        .find("await put(db, MANIFESTS_STORE, sessionId, manifest)")
        .expect("atomic manifest marker");
    assert!(pending_write < page_write);
    assert!(page_write < skeleton_write);
    assert!(skeleton_write < image_tables_write);
    assert!(image_tables_write < writer_free);
    assert!(writer_free < document_write);
    assert!(document_write < manifest_write);
    assert!(js.contains("status: 'pending'"));
    assert!(js.contains("status: 'committed'"));
    assert!(js.contains("getAllKeys()"), "orphan records must be pruned");
    assert!(
        js.contains("TEMP_TTL_MS"),
        "temp sessions need bounded lifetime"
    );
}

#[test]
fn worker_returns_a_small_receipt_and_main_reads_the_committed_blob_after_teardown() {
    let js = source("figma_temp_worker.js");
    let post_message = js
        .find("self.postMessage({\n      ok: true")
        .expect("worker success receipt");
    let post_message_end = js[post_message..]
        .find("});")
        .map(|offset| post_message + offset)
        .expect("worker success receipt end");
    let receipt = &js[post_message..post_message_end];
    assert!(receipt.contains("sessionId"));
    assert!(receipt.contains("pageCount"));
    assert!(receipt.contains("warningsJson"));
    assert!(
        !receipt.contains("fullDocumentJson"),
        "canonical JSON must not be structured-cloned out of the Worker"
    );

    let main_handler = js
        .find("worker.onmessage = async (event) =>")
        .expect("main Worker message handler");
    let terminate = js[main_handler..]
        .find("releaseWorker();")
        .map(|offset| main_handler + offset)
        .expect("Worker teardown before handoff");
    let idb_read = js[main_handler..]
        .find("await readCommittedDocument(")
        .map(|offset| main_handler + offset)
        .expect("committed document read");
    assert!(terminate < idb_read);
    assert!(js.contains("manifest.status !== 'committed'"));
    assert!(js.contains("return documentBlob.text();"));
    assert!(js.contains("finish({ ...receipt, fullDocumentJson })"));
}

#[test]
fn main_host_installs_complete_canonical_json_and_keeps_the_old_fallback() {
    let dom = source("dom_io/figma_import.rs");
    let actions = source("file_actions.rs");
    assert!(dom.contains("figma_temp_bridge::start"));
    assert!(dom.contains("ingest_figma_temp_source"));
    assert!(dom.contains("ingest_figma_file_fallback"));
    assert!(dom.contains("ingest_figma_bytes"));
    assert!(actions.contains("op_pen_loader::load_canonical(source)"));
    assert!(actions.contains("preserve_authored_geometry = true"));
}

#[test]
fn committed_temp_session_is_deleted_only_after_rust_installs_the_document() {
    let figma = source("dom_io/figma_import.rs");
    let success = figma
        .split("Ok(ingested) =>")
        .nth(1)
        .and_then(|tail| tail.split("Err(error) =>").next())
        .expect("successful Worker install branch");
    let install = success
        .find("if finish_document_import(")
        .expect("confirmed Rust install");
    let cleanup = success
        .find("figma_temp_bridge::delete_session")
        .expect("post-install cleanup");

    assert!(install < cleanup);
    assert!(source("dom_io.rs").contains(") -> bool {"));
}

#[test]
fn worker_lifecycle_has_cancellation_timeout_and_deferred_completion() {
    let js = source("figma_temp_worker.js");
    let bridge = source("figma_temp_bridge.rs");
    let generations = source("dom_io/import_generation.rs");
    assert!(js.contains("const activeImports = new Map()"));
    assert!(js.contains("opCancelAllFigmaTempImports()"));
    assert!(js.contains("worker.onmessageerror"));
    assert!(js.contains("Figma Worker timed out"));
    assert!(js.contains("request.onblocked"));
    assert!(js.contains("setTimeout(() => done(result), 0)"));
    assert!(bridge.contains("op_cancel_all_figma_temp_imports"));
    assert!(generations.contains("crate::figma_temp_bridge::cancel_all()"));
}
