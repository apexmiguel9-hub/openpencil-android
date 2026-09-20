//! Stage benchmark for the deferred binary Figma importer.
//!
//! Reports preparation, one-page conversion, and all-pages conversion
//! separately, together with prepared page metadata and recursive converted
//! node counts. Run this example with `--release` for meaningful timings.
//!
//! `PreparedFig` conversion consumes the prepared value, so the all-pages run
//! performs and reports a second preparation after the one-page result has
//! been summarized and dropped.
//!
//! Usage:
//! `cargo run --release -p op-figma --example bench_prepared -- <file.fig> [--page INDEX] [--layout preserve|openpencil]`

use jian_ops_schema::node::PenNode;
use op_figma::{prepare_fig_binary, FigLayoutMode};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const HELP: &str = "\
Benchmark PreparedFig stages on a real binary .fig file.

Usage:
  cargo run --release -p op-figma --example bench_prepared -- \
    <file.fig> [--page INDEX] [--layout preserve|openpencil]

Options:
  --page INDEX       Page converted by the single-page run (default: 0)
  --layout MODE      preserve (default) or openpencil
  -h, --help         Show this help

The input is read before timing starts. Conversion includes image resolution.
Because conversion consumes PreparedFig, the all-pages run prepares the same
bytes a second time and reports that re-prepare duration separately.";

struct Options {
    path: PathBuf,
    page_index: usize,
    layout_mode: FigLayoutMode,
}

struct PageSummary {
    name: String,
    root_nodes: usize,
    all_nodes: usize,
}

/// A malformed command line. `main` prints the `Display` text after
/// `error: `, so the rendering must stay byte-identical to the strings it
/// replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OptionsError {
    /// `--page` given without its index argument.
    MissingPageIndex,
    /// `--page` given a value that is not a `usize`.
    InvalidPageIndex(String),
    /// `--layout` given without its mode argument.
    MissingLayoutMode,
    /// `--layout` given a mode outside `preserve` / `openpencil`.
    InvalidLayoutMode(String),
    /// An unrecognised `-…` flag.
    UnknownOption(String),
    /// More than one positional `.fig` path.
    DuplicateInputPath,
    /// No positional `.fig` path at all.
    MissingInputPath,
}

impl std::fmt::Display for OptionsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPageIndex => formatter.write_str("--page requires an index"),
            Self::InvalidPageIndex(value) => write!(formatter, "invalid page index {value:?}"),
            Self::MissingLayoutMode => formatter.write_str("--layout requires a mode"),
            Self::InvalidLayoutMode(value) => write!(formatter, "invalid layout mode {value:?}"),
            Self::UnknownOption(value) => write!(formatter, "unknown option {value:?}"),
            Self::DuplicateInputPath => {
                formatter.write_str("only one input .fig path may be supplied")
            }
            Self::MissingInputPath => formatter.write_str("missing input .fig path"),
        }
    }
}

impl std::error::Error for OptionsError {}

/// A benchmark stage that could not run. Detail payloads carry the upstream
/// error's display text so this enum stays `Clone`/`Eq`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RunError {
    /// The input `.fig` file could not be read.
    ReadInput { path: String, detail: String },
    /// The first `prepare_fig_binary` call failed.
    Prepare(String),
    /// `--page INDEX` is past the prepared page count.
    PageOutOfRange { index: usize, count: usize },
    /// `PreparedFig::into_page` failed.
    SinglePageConversion(String),
    /// The second (all-pages) `prepare_fig_binary` call failed.
    RePrepare(String),
    /// `PreparedFig::into_all_pages` failed.
    AllPagesConversion(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadInput { path, detail } => write!(formatter, "cannot read {path}: {detail}"),
            Self::Prepare(detail) => write!(formatter, "prepare failed: {detail}"),
            Self::PageOutOfRange { index, count } => write!(
                formatter,
                "page index {index} is out of range (page count: {count})"
            ),
            Self::SinglePageConversion(detail) => {
                write!(formatter, "single-page conversion failed: {detail}")
            }
            Self::RePrepare(detail) => write!(formatter, "re-prepare failed: {detail}"),
            Self::AllPagesConversion(detail) => {
                write!(formatter, "all-pages conversion failed: {detail}")
            }
        }
    }
}

impl std::error::Error for RunError {}

fn main() {
    match parse_options(std::env::args().skip(1)) {
        Ok(Some(options)) => {
            if let Err(error) = run(options) {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
        }
        Ok(None) => println!("{HELP}"),
        Err(error) => {
            eprintln!("error: {error}\n\n{HELP}");
            std::process::exit(2);
        }
    }
}

fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, OptionsError> {
    let mut args = args.into_iter();
    let mut path = None;
    let mut page_index = 0;
    let mut layout_mode = FigLayoutMode::Preserve;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--page" => {
                let value = args.next().ok_or(OptionsError::MissingPageIndex)?;
                page_index = match value.parse() {
                    Ok(index) => index,
                    Err(_) => return Err(OptionsError::InvalidPageIndex(value)),
                };
            }
            "--layout" => {
                let value = args.next().ok_or(OptionsError::MissingLayoutMode)?;
                layout_mode = match value.as_str() {
                    "preserve" => FigLayoutMode::Preserve,
                    "openpencil" => FigLayoutMode::OpenPencil,
                    _ => return Err(OptionsError::InvalidLayoutMode(value)),
                };
            }
            value if value.starts_with('-') => {
                return Err(OptionsError::UnknownOption(value.to_string()));
            }
            value => {
                if path.replace(PathBuf::from(value)).is_some() {
                    return Err(OptionsError::DuplicateInputPath);
                }
            }
        }
    }

    let path = path.ok_or(OptionsError::MissingInputPath)?;
    Ok(Some(Options {
        path,
        page_index,
        layout_mode,
    }))
}

fn run(options: Options) -> Result<(), RunError> {
    let bytes = std::fs::read(&options.path).map_err(|error| RunError::ReadInput {
        path: options.path.display().to_string(),
        detail: error.to_string(),
    })?;
    let file_name = options
        .path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Figma Import");
    println!("input: {} ({} bytes)", options.path.display(), bytes.len());
    println!("layout: {}", layout_name(options.layout_mode));

    let started = Instant::now();
    let prepared = prepare_fig_binary(&bytes, file_name, options.layout_mode)
        .map_err(|error| RunError::Prepare(error.to_string()))?;
    let prepare_elapsed = started.elapsed();
    let pages = prepared.pages().to_vec();
    println!("prepare: {}", elapsed(prepare_elapsed));
    println!("prepared pages: {}", pages.len());
    for (index, page) in pages.iter().enumerate() {
        println!(
            "  [{index}] id={:?} name={:?} root_children={}",
            page.id, page.name, page.child_count
        );
    }

    let selected = pages
        .get(options.page_index)
        .ok_or(RunError::PageOutOfRange {
            index: options.page_index,
            count: pages.len(),
        })?;
    let started = Instant::now();
    let single = prepared
        .into_page(options.page_index)
        .map_err(|error| RunError::SinglePageConversion(error.to_string()))?;
    let single_elapsed = started.elapsed();
    let single_warnings = single.warnings.len();
    let single_pages = summarize_pages(single.document.pages.as_deref().unwrap_or(&[]));
    println!(
        "single-page convert: {} index={} name={:?} warnings={}",
        elapsed(single_elapsed),
        options.page_index,
        selected.name,
        single_warnings
    );
    print_converted_pages(&single_pages);
    drop(single);

    let started = Instant::now();
    let prepared = prepare_fig_binary(&bytes, file_name, options.layout_mode)
        .map_err(|error| RunError::RePrepare(error.to_string()))?;
    let reprepare_elapsed = started.elapsed();
    println!("re-prepare for all pages: {}", elapsed(reprepare_elapsed));

    let started = Instant::now();
    let all = prepared
        .into_all_pages()
        .map_err(|error| RunError::AllPagesConversion(error.to_string()))?;
    let all_elapsed = started.elapsed();
    let all_warnings = all.warnings.len();
    let all_pages = summarize_pages(all.document.pages.as_deref().unwrap_or(&[]));
    let total_nodes: usize = all_pages.iter().map(|page| page.all_nodes).sum();
    println!(
        "all-pages convert: {} pages={} nodes={} warnings={}",
        elapsed(all_elapsed),
        all_pages.len(),
        total_nodes,
        all_warnings
    );
    print_converted_pages(&all_pages);
    Ok(())
}

fn summarize_pages(pages: &[jian_ops_schema::page::PenPage]) -> Vec<PageSummary> {
    pages
        .iter()
        .map(|page| PageSummary {
            name: page.name.clone(),
            root_nodes: page.children.len(),
            all_nodes: count_nodes(&page.children),
        })
        .collect()
}

fn print_converted_pages(pages: &[PageSummary]) {
    for (index, page) in pages.iter().enumerate() {
        println!(
            "  [{index}] name={:?} root_nodes={} nodes={}",
            page.name, page.root_nodes, page.all_nodes
        );
    }
}

fn count_nodes(nodes: &[PenNode]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + count_nodes(children(node)))
        .sum()
}

fn children(node: &PenNode) -> &[PenNode] {
    match node {
        PenNode::Frame(node) => node.children.as_deref().unwrap_or(&[]),
        PenNode::Group(node) => node.children.as_deref().unwrap_or(&[]),
        PenNode::Rectangle(node) => node.children.as_deref().unwrap_or(&[]),
        PenNode::Tabs(node) => node.children.as_deref().unwrap_or(&[]),
        PenNode::Ref(node) => node.children.as_deref().unwrap_or(&[]),
        _ => &[],
    }
}

fn elapsed(duration: Duration) -> String {
    format!("{:.3} ms", duration.as_secs_f64() * 1_000.0)
}

fn layout_name(mode: FigLayoutMode) -> &'static str {
    match mode {
        FigLayoutMode::Preserve => "preserve",
        FigLayoutMode::OpenPencil => "openpencil",
    }
}
