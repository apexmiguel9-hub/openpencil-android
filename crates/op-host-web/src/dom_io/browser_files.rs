//! Hidden browser file inputs and one-shot `FileReader` operations.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

/// One-shot closure slot — the closure drops itself out of the slot after
/// firing, freeing its wasm-bindgen slot.
type OnceSlot = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

/// Shared one-shot callback ownership for async browser operations. Taking the
/// callback before invoking it makes synchronous setup failure and a later DOM
/// event race harmless: only the first completion can observe the callback.
type CallbackSlot<T> = Rc<RefCell<Option<Box<dyn FnOnce(T)>>>>;

fn complete_once<T>(slot: &CallbackSlot<T>, value: T) {
    let callback = slot.borrow_mut().take();
    if let Some(callback) = callback {
        callback(value);
    }
}

/// Open a normal multi-file picker. Selecting one `.html` file preserves the
/// basic import path; selecting its saved-page siblings supplies a resource
/// bundle without forcing browsers into directory-only mode.
pub(super) fn open_html_project_picker(on_files: Box<dyn FnOnce(Vec<web_sys::File>)>) {
    let result = (|| -> Result<(), JsValue> {
        let window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("HTML picker: window unavailable"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("HTML picker: document unavailable"))?;
        let input = document
            .create_element("input")?
            .dyn_into::<web_sys::HtmlInputElement>()
            .map_err(|_| JsValue::from_str("HTML picker: <input> cast failed"))?;
        input.set_type("file");
        input.set_multiple(true);
        input.set_accept(
            ".html,.htm,.zip,.css,.js,.mjs,.cjs,.json,.webmanifest,.map,.xml,.txt,.wasm,.png,.jpg,.jpeg,.gif,.webp,.avif,.ico,.bmp,.svg,.woff,.woff2,.ttf,.otf,.eot",
        );
        input.set_attribute("style", "display:none")?;
        input.set_attribute("aria-hidden", "true")?;
        let body = document
            .body()
            .ok_or_else(|| JsValue::from_str("HTML picker: document.body unavailable"))?;
        body.append_child(&input)?;

        let slot: OnceSlot = Rc::new(RefCell::new(None));
        let slot2 = slot.clone();
        let input_cb = input.clone();
        let mut once = Some(on_files);
        *slot.borrow_mut() = Some(Closure::new(move || {
            let mut files = Vec::new();
            if let Some(list) = input_cb.files() {
                for index in 0..list.length() {
                    if let Some(file) = list.get(index) {
                        files.push(file);
                    }
                }
            }
            input_cb.remove();
            if files.is_empty() {
                once.take();
            } else if let Some(cb) = once.take() {
                cb(files);
            }
            let _ = slot2.borrow_mut().take();
        }));
        let listener_result = {
            let slot_ref = slot.borrow();
            let closure = slot_ref.as_ref().expect("closure just installed");
            input
                .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())
                .and_then(|()| {
                    input.add_event_listener_with_callback(
                        "cancel",
                        closure.as_ref().unchecked_ref(),
                    )
                })
        };
        if let Err(error) = listener_result {
            input.remove();
            let _ = slot.borrow_mut().take();
            return Err(error);
        }
        input.click();
        Ok(())
    })();
    if let Err(error) = result {
        web_sys::console::error_1(&error);
    }
}

pub(super) fn html_project_file_path(file: &web_sys::File) -> String {
    let relative = js_sys::Reflect::get(file.as_ref(), &JsValue::from_str("webkitRelativePath"))
        .ok()
        .and_then(|value| value.as_string())
        .filter(|path| !path.is_empty());
    relative.unwrap_or_else(|| file.name())
}

/// Pop a hidden `<input type=file accept=…>` and invoke `on_file` with the
/// chosen file. Modern browsers dispatch `cancel`, which runs the same cleanup
/// path even when no file was selected.
pub(crate) fn open_file_picker(accept: &str, on_file: Box<dyn FnOnce(web_sys::File)>) {
    let result = (|| -> Result<(), JsValue> {
        let window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("file picker: window unavailable"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("file picker: document unavailable"))?;
        let input = document
            .create_element("input")?
            .dyn_into::<web_sys::HtmlInputElement>()
            .map_err(|_| JsValue::from_str("file picker: <input> cast failed"))?;
        input.set_type("file");
        input.set_accept(accept);
        input.set_attribute("style", "display:none")?;
        input.set_attribute("aria-hidden", "true")?;
        let body = document
            .body()
            .ok_or_else(|| JsValue::from_str("file picker: document.body unavailable"))?;
        body.append_child(&input)?;

        let slot: OnceSlot = Rc::new(RefCell::new(None));
        let slot2 = slot.clone();
        let input_cb = input.clone();
        let mut once = Some(on_file);
        *slot.borrow_mut() = Some(Closure::new(move || {
            let file = input_cb.files().and_then(|files| files.get(0));
            input_cb.remove();
            if let (Some(cb), Some(file)) = (once.take(), file) {
                cb(file);
            }
            let _ = slot2.borrow_mut().take();
        }));
        let listener_result = {
            let slot_ref = slot.borrow();
            let closure = slot_ref.as_ref().expect("closure just installed");
            input
                .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())
                .and_then(|()| {
                    input.add_event_listener_with_callback(
                        "cancel",
                        closure.as_ref().unchecked_ref(),
                    )
                })
        };
        if let Err(error) = listener_result {
            input.remove();
            let _ = slot.borrow_mut().take();
            return Err(error);
        }
        input.click();
        Ok(())
    })();
    if let Err(error) = result {
        web_sys::console::error_1(&error);
    }
}

/// How to read a `File` — maps onto the three `FileReader` modes the
/// ingestion paths need.
pub(crate) enum ReadMode {
    /// `readAsText` → `.op` / `.pen` / `.svg` sources.
    Text,
    /// `readAsArrayBuffer` → binary `.fig` and HTML-project resources.
    Bytes,
    /// `readAsDataURL` → raster images (the browser builds the `data:` URL).
    DataUrl,
}

/// Read `file` asynchronously and invoke `on_done` with the raw
/// `FileReader.result` (`JsValue::NULL` on any failed read).
pub(crate) fn read_file(file: web_sys::File, mode: ReadMode, on_done: Box<dyn FnOnce(JsValue)>) {
    let callback: CallbackSlot<JsValue> = Rc::new(RefCell::new(Some(on_done)));
    let reader = match web_sys::FileReader::new() {
        Ok(reader) => reader,
        Err(error) => {
            web_sys::console::error_1(&error);
            complete_once(&callback, JsValue::NULL);
            return;
        }
    };

    let closure_slot: OnceSlot = Rc::new(RefCell::new(None));
    let closure_slot_cb = closure_slot.clone();
    let callback_cb = callback.clone();
    let reader_cb = reader.clone();
    *closure_slot.borrow_mut() = Some(Closure::new(move || {
        let value = reader_cb.result().unwrap_or(JsValue::NULL);
        reader_cb.set_onloadend(None);
        complete_once(&callback_cb, value);
        let _ = closure_slot_cb.borrow_mut().take();
    }));
    {
        let slot_ref = closure_slot.borrow();
        let closure = slot_ref.as_ref().expect("closure just installed");
        reader.set_onloadend(Some(closure.as_ref().unchecked_ref()));
    }

    let read_result = match mode {
        ReadMode::Text => reader.read_as_text(&file),
        ReadMode::Bytes => reader.read_as_array_buffer(&file),
        ReadMode::DataUrl => reader.read_as_data_url(&file),
    };
    if let Err(error) = read_result {
        reader.set_onloadend(None);
        web_sys::console::error_1(&error);
        complete_once(&callback, JsValue::NULL);
        let _ = closure_slot.borrow_mut().take();
    }
}

/// A `Blob.slice()` range read that could not be completed.
///
/// `Display` reproduces the ad-hoc `String` messages this enum replaced byte
/// for byte, so the `[import-html] …` console lines stay identical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BlobReadError {
    /// `offset + length` does not fit in a `u64`.
    RangeOverflow,
    /// The range end is outside JavaScript's exactly representable integers.
    RangeNotExact,
    /// The range end is past the end of the source Blob.
    RangeExceedsSource { offset: u64, end: u64, size: u64 },
    /// `Blob.slice` threw.
    SliceFailed(String),
    /// `new FileReader()` threw.
    ReaderCreateFailed(String),
    /// The reader completed with an error result.
    ReadFailed(String),
    /// The reader completed with something other than an `ArrayBuffer`.
    NotArrayBuffer,
    /// `FileReader.readAsArrayBuffer` threw before any read began.
    ReadCouldNotStart(String),
}

impl std::fmt::Display for BlobReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobReadError::RangeOverflow => write!(f, "Blob range end overflows u64"),
            BlobReadError::RangeNotExact => {
                write!(f, "Blob range exceeds JavaScript's exact integer range")
            }
            BlobReadError::RangeExceedsSource { offset, end, size } => {
                write!(f, "Blob range {offset}..{end} exceeds source size {size}")
            }
            BlobReadError::SliceFailed(error) => write!(f, "Blob.slice failed: {error}"),
            BlobReadError::ReaderCreateFailed(error) => {
                write!(f, "FileReader creation failed: {error}")
            }
            BlobReadError::ReadFailed(error) => write!(f, "Blob range read failed: {error}"),
            BlobReadError::NotArrayBuffer => {
                write!(f, "Blob range read did not produce an ArrayBuffer")
            }
            BlobReadError::ReadCouldNotStart(error) => {
                write!(f, "Blob range read could not start: {error}")
            }
        }
    }
}

impl std::error::Error for BlobReadError {}

impl From<BlobReadError> for String {
    fn from(error: BlobReadError) -> String {
        error.to_string()
    }
}

/// Read one byte range from a browser `Blob` without copying the whole source.
/// The callback is completed exactly once for setup errors, read failures,
/// aborts, and successful reads.
pub(crate) fn read_blob_range(
    blob: &web_sys::Blob,
    offset: u64,
    length: u64,
    on_done: Box<dyn FnOnce(Result<Vec<u8>, BlobReadError>)>,
) {
    let callback: CallbackSlot<Result<Vec<u8>, BlobReadError>> =
        Rc::new(RefCell::new(Some(on_done)));
    let Some(end) = offset.checked_add(length) else {
        complete_once(&callback, Err(BlobReadError::RangeOverflow));
        return;
    };
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    if end > MAX_SAFE_INTEGER {
        complete_once(&callback, Err(BlobReadError::RangeNotExact));
        return;
    }
    let blob_size = blob.size() as u64;
    if end > blob_size {
        complete_once(
            &callback,
            Err(BlobReadError::RangeExceedsSource {
                offset,
                end,
                size: blob_size,
            }),
        );
        return;
    }
    let slice = match blob.slice_with_f64_and_f64(offset as f64, end as f64) {
        Ok(slice) => slice,
        Err(error) => {
            complete_once(
                &callback,
                Err(BlobReadError::SliceFailed(js_error_message(&error))),
            );
            return;
        }
    };
    let reader = match web_sys::FileReader::new() {
        Ok(reader) => reader,
        Err(error) => {
            complete_once(
                &callback,
                Err(BlobReadError::ReaderCreateFailed(js_error_message(&error))),
            );
            return;
        }
    };

    let closure_slot: OnceSlot = Rc::new(RefCell::new(None));
    let closure_slot_cb = closure_slot.clone();
    let callback_cb = callback.clone();
    let reader_cb = reader.clone();
    *closure_slot.borrow_mut() = Some(Closure::new(move || {
        let result = reader_cb
            .result()
            .map_err(|error| BlobReadError::ReadFailed(js_error_message(&error)))
            .and_then(|value| js_bytes(&value).ok_or(BlobReadError::NotArrayBuffer));
        reader_cb.set_onloadend(None);
        complete_once(&callback_cb, result);
        let _ = closure_slot_cb.borrow_mut().take();
    }));
    {
        let slot_ref = closure_slot.borrow();
        let closure = slot_ref.as_ref().expect("closure just installed");
        reader.set_onloadend(Some(closure.as_ref().unchecked_ref()));
    }

    if let Err(error) = reader.read_as_array_buffer(&slice) {
        reader.set_onloadend(None);
        complete_once(
            &callback,
            Err(BlobReadError::ReadCouldNotStart(js_error_message(&error))),
        );
        let _ = closure_slot.borrow_mut().take();
    }
}

fn js_error_message(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| js_sys::JSON::stringify(error).ok()?.as_string())
        .unwrap_or_else(|| "unknown JavaScript error".to_string())
}

/// Extract a byte vec from a `FileReader.result` ArrayBuffer.
pub(crate) fn js_bytes(value: &JsValue) -> Option<Vec<u8>> {
    let buf = value.clone().dyn_into::<js_sys::ArrayBuffer>().ok()?;
    Some(js_sys::Uint8Array::new(&buf).to_vec())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn completion_callback_is_exactly_once() {
        let calls = Rc::new(Cell::new(0));
        let observed = Rc::new(Cell::new(0));
        let calls_cb = calls.clone();
        let observed_cb = observed.clone();
        let slot: CallbackSlot<u32> = Rc::new(RefCell::new(Some(Box::new(move |value| {
            calls_cb.set(calls_cb.get() + 1);
            observed_cb.set(value);
        }))));

        complete_once(&slot, 17);
        complete_once(&slot, 29);

        assert_eq!(calls.get(), 1);
        assert_eq!(observed.get(), 17);
        assert!(slot.borrow().is_none());
    }

    #[test]
    fn range_completion_callback_is_exactly_once() {
        let calls = Rc::new(Cell::new(0));
        let calls_cb = calls.clone();
        let slot: CallbackSlot<Result<Vec<u8>, BlobReadError>> =
            Rc::new(RefCell::new(Some(Box::new(move |_| {
                calls_cb.set(calls_cb.get() + 1)
            }))));

        complete_once(&slot, Ok(vec![1, 2, 3]));
        complete_once(&slot, Err(BlobReadError::NotArrayBuffer));

        assert_eq!(calls.get(), 1);
        assert!(slot.borrow().is_none());
    }
}
