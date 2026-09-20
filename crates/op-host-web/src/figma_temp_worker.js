// Browser-side module-Worker and IndexedDB adapter for `.fig` imports.
//
// The editor's WASM instance never receives the raw `.fig` bytes. A separate
// module Worker loads a second copy of op-host-web, converts the file through
// `FigmaTempWriter`, writes page shards plus a complete canonical `.op` record
// to IndexedDB, and then returns only a small commit receipt. The main thread
// terminates the Worker's large WASM instance before reading that record back
// for the current eager EditorState install. Lazy page residency is deliberately
// a later host-level change because history and document-wide commands require
// a stronger backing-store contract.

const DB_NAME = 'openpencil-figma-temp';
const DB_VERSION = 1;
const RECORDS_STORE = 'records';
const MANIFESTS_STORE = 'manifests';
const TEMP_TTL_MS = 24 * 60 * 60 * 1000;
const IMPORT_TIMEOUT_MS = 10 * 60 * 1000;
const activeImports = new Map();

function mainRequestResult(request, label) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error(label));
  });
}

function mainTransactionDone(transaction, label) {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error || new Error(label));
    transaction.onabort = () => reject(transaction.error || new Error(`${label} (aborted)`));
  });
}

function openMainDatabase() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    let blocked = false;
    request.onupgradeneeded = () => {
      const opened = request.result;
      if (!opened.objectStoreNames.contains(RECORDS_STORE)) opened.createObjectStore(RECORDS_STORE);
      if (!opened.objectStoreNames.contains(MANIFESTS_STORE)) opened.createObjectStore(MANIFESTS_STORE);
    };
    request.onsuccess = () => {
      if (blocked) request.result.close();
      else resolve(request.result);
    };
    request.onerror = () => reject(request.error || new Error('IndexedDB open failed'));
    request.onblocked = () => {
      blocked = true;
      reject(new Error('IndexedDB cleanup was blocked by another tab'));
    };
  });
}

async function readCommittedDocument(sessionId, expectedPageCount) {
  const db = await openMainDatabase();
  let documentBlob = null;
  try {
    const transaction = db.transaction([RECORDS_STORE, MANIFESTS_STORE], 'readonly');
    const done = mainTransactionDone(transaction, 'IndexedDB document read failed');
    const manifestRequest = transaction.objectStore(MANIFESTS_STORE).get(sessionId);
    const documentKey = sessionId + '/document';
    const documentRequest = transaction.objectStore(RECORDS_STORE).get(documentKey);
    const [manifest, storedDocument] = await Promise.all([
      mainRequestResult(manifestRequest, 'IndexedDB manifest read failed'),
      mainRequestResult(documentRequest, 'IndexedDB document record read failed'),
    ]);
    await done;

    if (!manifest || manifest.status !== 'committed' || manifest.sessionId !== sessionId) {
      throw new Error('Figma temp session was not committed');
    }
    if (Number(manifest.pageCount || 0) !== expectedPageCount) {
      throw new Error('Figma temp session page count changed before install');
    }
    if (manifest.documentKey !== documentKey) {
      throw new Error('Figma temp session document key is invalid');
    }
    if (!storedDocument || typeof storedDocument.text !== 'function') {
      throw new Error('Figma temp session is missing canonical JSON');
    }
    documentBlob = storedDocument;
  } finally {
    db.close();
  }
  return documentBlob.text();
}

function generatedWasmModuleUrl() {
  const url = new URL(import.meta.url);
  const marker = '/snippets/';
  const at = url.pathname.lastIndexOf(marker);
  if (at >= 0) {
    // wasm-bindgen copies this file to
    //   <pkg>/snippets/<hash>/src/figma_temp_worker.js
    // while the generated module remains at <pkg>/op_host_web.js.
    url.pathname = `${url.pathname.slice(0, at)}/op_host_web.js`;
    url.search = '';
    url.hash = '';
    return url.href;
  }
  // Source-tree/dev fallback. Production bundles always take the snippets
  // branch above, but keeping this deterministic makes direct smoke harnesses
  // easier to diagnose.
  return new URL('../pkg-ck/op_host_web.js', import.meta.url).href;
}

function workerSource() {
  // Dynamic import keeps the generated module URL a runtime value. It also
  // means the same source works after the pkg directory is mounted below a
  // different base path.
  return `
const DB_NAME = ${JSON.stringify(DB_NAME)};
const DB_VERSION = ${DB_VERSION};
const RECORDS_STORE = ${JSON.stringify(RECORDS_STORE)};
const MANIFESTS_STORE = ${JSON.stringify(MANIFESTS_STORE)};
const TEMP_TTL_MS = ${TEMP_TTL_MS};

function requestResult(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error('IndexedDB request failed'));
  });
}

function transactionDone(transaction) {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error || new Error('IndexedDB transaction failed'));
    transaction.onabort = () => reject(transaction.error || new Error('IndexedDB transaction aborted'));
  });
}

async function openDatabase() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    let blocked = false;
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(RECORDS_STORE)) db.createObjectStore(RECORDS_STORE);
      if (!db.objectStoreNames.contains(MANIFESTS_STORE)) db.createObjectStore(MANIFESTS_STORE);
    };
    request.onsuccess = () => {
      if (blocked) request.result.close();
      else resolve(request.result);
    };
    request.onerror = () => reject(request.error || new Error('IndexedDB open failed'));
    request.onblocked = () => {
      blocked = true;
      reject(new Error('IndexedDB open was blocked by another tab'));
    };
  });
}

async function put(db, storeName, key, value) {
  const transaction = db.transaction(storeName, 'readwrite');
  transaction.objectStore(storeName).put(value, key);
  await transactionDone(transaction);
}

async function deleteSession(db, sessionId, pageCount) {
  const transaction = db.transaction([RECORDS_STORE, MANIFESTS_STORE], 'readwrite');
  const records = transaction.objectStore(RECORDS_STORE);
  for (let index = 0; index < pageCount; index += 1) {
    records.delete(sessionId + '/page/' + index);
  }
  records.delete(sessionId + '/document');
  records.delete(sessionId + '/skeleton');
  records.delete(sessionId + '/image-tables');
  transaction.objectStore(MANIFESTS_STORE).delete(sessionId);
  await transactionDone(transaction).catch(() => {});
}

function sessionCreatedAt(sessionId) {
  const match = /^fig-([0-9a-f]{13})-/i.exec(sessionId);
  if (!match) return null;
  const createdAt = Number.parseInt(match[1], 16);
  return Number.isFinite(createdAt) ? createdAt : null;
}

async function cleanupExpired(db) {
  const manifestsTransaction = db.transaction(MANIFESTS_STORE, 'readonly');
  const manifestsStore = manifestsTransaction.objectStore(MANIFESTS_STORE);
  const manifests = await requestResult(manifestsStore.getAll());
  await transactionDone(manifestsTransaction);
  const now = Date.now();
  const live = new Set();
  for (const manifest of manifests) {
    if (!manifest || typeof manifest.sessionId !== 'string') continue;
    if (now - Number(manifest.createdAt || 0) > TEMP_TTL_MS) {
      await deleteSession(db, manifest.sessionId, Number(manifest.pageCount || 0));
    } else {
      // Both pending leases and committed sessions are live. Future readers
      // must require status === 'committed', but cleanup must not race a Worker
      // that is still writing the page records behind a pending lease.
      live.add(manifest.sessionId);
    }
  }

  // A tab/Worker crash can leave page records without a manifest. Enumerating
  // keys (not values) is cheap; only TTL-expired session ids are removed so an
  // older concurrently-running Worker cannot be mistaken for an orphan.
  const keysTransaction = db.transaction(RECORDS_STORE, 'readonly');
  const keys = await requestResult(keysTransaction.objectStore(RECORDS_STORE).getAllKeys());
  await transactionDone(keysTransaction);
  const orphanKeys = keys.filter((key) => {
    const text = String(key);
    const slash = text.indexOf('/');
    if (slash <= 0) return false;
    const sessionId = text.slice(0, slash);
    if (live.has(sessionId)) return false;
    const createdAt = sessionCreatedAt(sessionId);
    // Never delete an uncommitted record from another active Worker. New
    // sessions publish a pending lease before their first record; this age
    // guard also keeps cleanup safe across one old/new-version overlap.
    return createdAt !== null && now - createdAt > TEMP_TTL_MS;
  });
  if (orphanKeys.length > 0) {
    const transaction = db.transaction(RECORDS_STORE, 'readwrite');
    const store = transaction.objectStore(RECORDS_STORE);
    for (const key of orphanKeys) store.delete(key);
    await transactionDone(transaction);
  }
}

self.onmessage = async (event) => {
  const { file, fileName, sessionId, wasmModuleUrl } = event.data;
  let db = null;
  let pageCount = 0;
  let writer = null;
  try {
    const wasm = await import(wasmModuleUrl);
    await wasm.default();
    let bytes = new Uint8Array(await file.arrayBuffer());
    writer = new wasm.FigmaTempWriter(bytes, fileName);
    bytes = null;
    pageCount = writer.page_count();
    if (pageCount < 1) throw new Error('Figma import produced no pages');

    db = await openDatabase();
    await cleanupExpired(db);

    const pages = [];
    const createdAt = Date.now();
    const pendingManifest = {
      formatVersion: 1,
      status: 'pending',
      sessionId,
      fileName,
      pageCount,
      pages: [],
      skeletonKey: sessionId + '/skeleton',
      imageTablesKey: sessionId + '/image-tables',
      documentKey: sessionId + '/document',
      createdAt,
    };
    // Publish the lease before the first page record. Cleanup in another tab
    // preserves non-expired pending sessions while future readers ignore them.
    await put(db, MANIFESTS_STORE, sessionId, pendingManifest);

    for (let index = 0; index < pageCount; index += 1) {
      const id = writer.page_id(index) || ('page-' + index);
      const name = writer.page_name(index) || ('Page ' + (index + 1));
      const pageJson = writer.page_json(index);
      await put(
        db,
        RECORDS_STORE,
        sessionId + '/page/' + index,
        new Blob([pageJson], { type: 'application/json' }),
      );
      pages.push({ id, name });
    }

    const skeletonJson = writer.skeleton_json();
    await put(
      db,
      RECORDS_STORE,
      sessionId + '/skeleton',
      new Blob([skeletonJson], { type: 'application/json' }),
    );
    const imageTablesJson = writer.image_tables_json();
    await put(
      db,
      RECORDS_STORE,
      sessionId + '/image-tables',
      new Blob([imageTablesJson], { type: 'application/json' }),
    );

    const warningsJson = writer.warnings_json();
    const fullDocumentJson = writer.full_document_json();
    // Drop the complete serde_json::Value before Blob encoding and IndexedDB
    // IO create more representations of the canonical JSON.
    writer.free();
    writer = null;
    await put(
      db,
      RECORDS_STORE,
      sessionId + '/document',
      new Blob([fullDocumentJson], { type: 'application/json' }),
    );

    // Updating the pending lease to committed is the visibility marker. Future
    // lazy readers must require this exact status, so quota/error paths never
    // expose a half-written document.
    const manifest = {
      ...pendingManifest,
      status: 'committed',
      pages,
      committedAt: Date.now(),
    };
    await put(db, MANIFESTS_STORE, sessionId, manifest);

    db.close();
    db = null;
    self.postMessage({
      ok: true,
      sessionId,
      pageCount,
      warningsJson,
    });
  } catch (error) {
    if (writer) writer.free();
    if (db) {
      await deleteSession(db, sessionId, pageCount);
      db.close();
    }
    self.postMessage({
      ok: false,
      sessionId,
      pageCount,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
`;
}

/**
 * Start one isolated `.fig` conversion. `done` is called exactly once with a
 * result object. Worker/IndexedDB/CSP failures are reported to Rust, whose
 * caller falls back to the existing main-thread parser.
 */
export function opStartFigmaTempImport(file, fileName, sessionId, done) {
  // The Rust generation guard owns which result may install. Canceling here
  // ensures direct JS callers also never retain two parser Workers at once.
  opCancelAllFigmaTempImports();
  let objectUrl = '';
  let worker = null;
  let timeout = 0;
  let settled = false;
  const releaseWorker = () => {
    if (worker) {
      worker.onmessage = null;
      worker.onerror = null;
      worker.onmessageerror = null;
      worker.terminate();
      worker = null;
    }
    if (objectUrl) {
      URL.revokeObjectURL(objectUrl);
      objectUrl = '';
    }
  };
  const finish = (result) => {
    if (settled) return;
    settled = true;
    activeImports.delete(sessionId);
    if (timeout) clearTimeout(timeout);
    releaseWorker();
    const deliver = () => {
      // Keep one task boundary between Worker teardown/cleanup and Rust's
      // synchronous parse of the canonical success payload or fallback.
      setTimeout(() => done(result), 0);
    };
    if (!result || result.ok !== true) {
      void opDeleteFigmaTempSession(sessionId, Number(result?.pageCount || 0)).then(deliver);
    } else {
      deliver();
    }
  };
  const cancel = () =>
    finish({ ok: false, canceled: true, sessionId, pageCount: 0, error: 'Figma import canceled' });
  try {
    const blob = new Blob([workerSource()], { type: 'text/javascript' });
    objectUrl = URL.createObjectURL(blob);
    worker = new Worker(objectUrl, { type: 'module', name: 'openpencil-figma-import' });
    worker.onmessage = async (event) => {
      const receipt = event.data;
      if (!receipt || receipt.ok !== true) {
        finish(receipt);
        return;
      }
      if (receipt.sessionId !== sessionId || Number(receipt.pageCount || 0) < 1) {
        finish({
          ok: false,
          sessionId,
          pageCount: Number(receipt.pageCount || 0),
          error: 'Figma Worker returned an invalid commit receipt',
        });
        return;
      }
      // The receipt is intentionally tiny. Tear down the second WASM instance
      // before materializing the canonical JSON in the main JavaScript realm.
      releaseWorker();
      try {
        const fullDocumentJson = await readCommittedDocument(
          receipt.sessionId,
          Number(receipt.pageCount || 0),
        );
        finish({ ...receipt, fullDocumentJson });
      } catch (error) {
        finish({
          ok: false,
          sessionId,
          pageCount: Number(receipt.pageCount || 0),
          error: error instanceof Error ? error.message : String(error),
        });
      }
    };
    worker.onerror = (event) => {
      event.preventDefault();
      finish({ ok: false, sessionId, pageCount: 0, error: event.message || 'Figma Worker failed' });
    };
    worker.onmessageerror = () =>
      finish({
        ok: false,
        sessionId,
        pageCount: 0,
        error: 'Figma Worker returned an unreadable message',
      });
    activeImports.set(sessionId, { cancel });
    timeout = setTimeout(
      () =>
        finish({
          ok: false,
          sessionId,
          pageCount: 0,
          error: 'Figma Worker timed out',
        }),
      IMPORT_TIMEOUT_MS,
    );
    worker.postMessage({
      file,
      fileName,
      sessionId,
      wasmModuleUrl: generatedWasmModuleUrl(),
    });
  } catch (error) {
    finish({
      ok: false,
      sessionId,
      pageCount: 0,
      error: error instanceof Error ? error.message : String(error),
    });
  }
}

/** Cancel every in-flight parser Worker before a new document import starts. */
export function opCancelAllFigmaTempImports() {
  for (const entry of Array.from(activeImports.values())) entry.cancel();
}

/** Delete a completed session that lost the Rust import-generation race. */
export async function opDeleteFigmaTempSession(sessionId, pageCount) {
  let db = null;
  try {
    db = await openMainDatabase();

    const keysTransaction = db.transaction(RECORDS_STORE, 'readonly');
    const keysRequest = keysTransaction.objectStore(RECORDS_STORE).getAllKeys();
    const keysDone = mainTransactionDone(keysTransaction, 'IndexedDB key transaction failed');
    const keys = await mainRequestResult(keysRequest, 'IndexedDB key read failed');
    await keysDone;

    const transaction = db.transaction([RECORDS_STORE, MANIFESTS_STORE], 'readwrite');
    const records = transaction.objectStore(RECORDS_STORE);
    const prefix = sessionId + '/';
    for (const key of keys) {
      if (String(key).startsWith(prefix)) records.delete(key);
    }
    // Retain the count-based deletes for compatibility with a browser whose
    // key enumeration omitted records written by an older bundle.
    for (let index = 0; index < pageCount; index += 1) {
      records.delete(sessionId + '/page/' + index);
    }
    transaction.objectStore(MANIFESTS_STORE).delete(sessionId);
    await mainTransactionDone(transaction, 'IndexedDB cleanup failed');
  } catch (error) {
    console.warn('[import-figma] temp cleanup failed:', error);
  } finally {
    if (db) db.close();
  }
}
