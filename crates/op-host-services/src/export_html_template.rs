//! The self-contained slideshow page `export_html.rs` writes.
//!
//! Everything here is inline: no stylesheet, no script, no font and no
//! image lives outside the single `.html` file. That is the whole point
//! of the export — a presenter copies one file onto a conference laptop
//! that may have no network at all, double-clicks it, and presents.
//!
//! Kept in its own module so the exporter next door stays about
//! rendering and IO rather than markup.

use op_util::xml_escape::escape_html;
use std::fmt::Write as _;

use crate::export_html_structured::css_num;

/// One board's markup: the label the page announces it under, the
/// board's authored size, and the already-built element tree.
pub struct SlideAsset {
    /// Authored frame name. Used as the slide's accessible label — the
    /// visible chrome is the counter, so a deck never shows a stray
    /// layer name.
    pub name: String,
    /// Board size in doc px. The player scales the slide from this.
    pub width: f32,
    pub height: f32,
    /// The slide's inner markup, in board-local coordinates.
    pub body: String,
}

/// Build the whole page. `title` is the browser-tab / window title.
pub fn render_slideshow_page(title: &str, slides: &[SlideAsset]) -> String {
    // Every slide is emitted as its own element and navigation only
    // flips a class. Building the whole deck up front costs one longer
    // load but buys a transition that cannot stutter — swapping one
    // container's contents on each advance would re-style and re-paint
    // a full slide mid-presentation.
    let mut sections = String::new();
    for (i, slide) in slides.iter().enumerate() {
        let current = if i == 0 { " is-current" } else { "" };
        let _ = write!(
            sections,
            "\n<div class=\"slide{current}\" role=\"group\" aria-label=\"{label}\" \
             data-w=\"{w}\" data-h=\"{h}\" style=\"width:{w}px;height:{h}px\">{body}</div>",
            label = escape_html(&slide.name),
            w = css_num(slide.width),
            h = css_num(slide.height),
            body = slide.body,
        );
    }
    let counter = format!("1 / {}", slides.len());
    format!(
        "<!doctype html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         <style>{PLAYER_CSS}</style>\n\
         </head>\n\
         <body>\n\
         <div id=\"deck\">{sections}\n</div>\n\
         <div id=\"counter\">{counter}</div>\n\
         <script>{PLAYER_JS}</script>\n\
         </body>\n\
         </html>\n",
        title = escape_html(title),
    )
}

/// Black surround, slide letterboxed into whatever aspect the viewport
/// happens to be, counter pinned bottom-right.
///
/// The slide keeps its authored pixel size and is fitted with
/// `transform: scale()`, never by resizing the box. A transform scales
/// the RENDERED output, so no element inside is re-laid-out at the
/// presentation size — which is what guarantees a projected slide
/// cannot wrap a line differently from the editor canvas. Sizing the
/// box (or `zoom`) would re-run layout and give up exactly that.
///
/// `.n` is every emitted node: absolutely positioned, border-box so a
/// stroke overlay's border stays inside its stated size. `.k` is a
/// stroke overlay, which must not intercept a click meant for the text
/// it covers.
const PLAYER_CSS: &str = "\
html,body{margin:0;height:100%;background:#000;overflow:hidden}\
#deck{position:fixed;inset:0}\
.slide{position:absolute;left:50%;top:50%;transform-origin:50% 50%;\
transform:translate(-50%,-50%) scale(var(--scale,1));\
overflow:hidden;background:#fff;opacity:0;visibility:hidden}\
.slide.is-current{opacity:1;visibility:visible}\
.slide .n{position:absolute;box-sizing:border-box}\
.slide .k{pointer-events:none}\
#counter{position:fixed;right:16px;bottom:12px;padding:6px 12px;border-radius:999px;\
font:500 13px/1.2 ui-sans-serif,system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;\
color:rgba(255,255,255,.75);background:rgba(255,255,255,.12);\
user-select:none;pointer-events:none}";

/// Key + click handling, plus the letterbox fit. The key set mirrors
/// the native slideshow (arrows / Space / PageUp / PageDown / Home /
/// End) so muscle memory built in the editor carries over to the
/// exported file.
const PLAYER_JS: &str = r#"
(function () {
  var slides = Array.prototype.slice.call(document.querySelectorAll('.slide'));
  var counter = document.getElementById('counter');
  var index = 0;
  if (!slides.length) { return; }
  // Each slide is fitted from its OWN authored size: a deck may mix
  // board sizes, and scaling them all by the first one's ratio would
  // crop or shrink the odd one out.
  function fit() {
    var vw = window.innerWidth, vh = window.innerHeight;
    for (var i = 0; i < slides.length; i++) {
      var w = parseFloat(slides[i].getAttribute('data-w')) || 1;
      var h = parseFloat(slides[i].getAttribute('data-h')) || 1;
      slides[i].style.setProperty('--scale', Math.min(vw / w, vh / h));
    }
  }
  function show(next) {
    // Clamp instead of wrap: a deck that jumped back to the title slide
    // on the last click would read as having lost the presenter's place.
    var target = Math.max(0, Math.min(slides.length - 1, next));
    if (target === index) { return; }
    slides[index].classList.remove('is-current');
    slides[target].classList.add('is-current');
    index = target;
    counter.textContent = (index + 1) + ' / ' + slides.length;
  }
  document.addEventListener('keydown', function (event) {
    switch (event.key) {
      case 'ArrowRight': case 'ArrowDown': case 'PageDown':
      case ' ': case 'Spacebar': case 'Enter':
        show(index + 1); break;
      case 'ArrowLeft': case 'ArrowUp': case 'PageUp': case 'Backspace':
        show(index - 1); break;
      case 'Home': show(0); break;
      case 'End': show(slides.length - 1); break;
      default: return;
    }
    event.preventDefault();
  });
  // The left quarter goes back so a mouse-only presenter can recover
  // from an accidental advance without reaching for the keyboard. A
  // click that started as a text selection is left alone — the text is
  // real and selectable, and advancing on release would undo it.
  document.addEventListener('click', function (event) {
    var selection = window.getSelection();
    if (selection && !selection.isCollapsed) { return; }
    show(event.clientX < window.innerWidth / 4 ? index - 1 : index + 1);
  });
  window.addEventListener('resize', fit);
  fit();
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn slide(name: &str) -> SlideAsset {
        SlideAsset {
            name: name.to_string(),
            width: 1920.0,
            height: 1080.0,
            body: r#"<div class="n"></div>"#.to_string(),
        }
    }

    #[test]
    fn only_the_first_slide_starts_visible() {
        let html = render_slideshow_page("deck", &[slide("one"), slide("two"), slide("three")]);
        assert_eq!(html.matches("class=\"slide is-current\"").count(), 1);
        assert_eq!(html.matches("class=\"slide\"").count(), 2);
        assert!(html.contains("<div id=\"counter\">1 / 3</div>"));
    }

    #[test]
    fn each_slide_carries_its_own_board_size_for_the_letterbox_fit() {
        let mut wide = slide("wide");
        wide.width = 1600.0;
        wide.height = 900.0;
        let html = render_slideshow_page("deck", &[slide("full"), wide]);
        assert!(html.contains(r#"data-w="1920" data-h="1080""#), "{html}");
        assert!(html.contains(r#"data-w="1600" data-h="900""#), "{html}");
        assert!(
            html.contains("style=\"width:1600px;height:900px\""),
            "{html}"
        );
    }

    #[test]
    fn titles_and_slide_names_are_escaped_into_the_markup() {
        let html = render_slideshow_page("A & B <deck>", &[slide("\"quoted\" & <angled>")]);
        assert!(
            html.contains("<title>A &amp; B &lt;deck&gt;</title>"),
            "{html}"
        );
        assert!(
            html.contains("aria-label=\"&quot;quoted&quot; &amp; &lt;angled&gt;\""),
            "{html}"
        );
        assert!(!html.contains("<deck>"), "raw markup leaked into the page");
    }

    #[test]
    fn the_player_binds_the_native_slideshow_key_set() {
        let html = render_slideshow_page("deck", &[slide("one")]);
        for key in [
            "ArrowRight",
            "ArrowLeft",
            "ArrowUp",
            "ArrowDown",
            "PageDown",
            "PageUp",
            "Home",
            "End",
        ] {
            assert!(html.contains(key), "missing key binding {key}");
        }
        assert!(html.contains("addEventListener('click'"), "{html}");
    }

    #[test]
    fn the_player_fits_each_slide_to_the_viewport_on_load_and_resize() {
        let html = render_slideshow_page("deck", &[slide("one")]);
        assert!(html.contains("addEventListener('resize', fit)"), "{html}");
        assert!(html.contains("Math.min(vw / w, vh / h)"), "{html}");
        // Scaling must not resize the box, or the browser would re-lay
        // out the slide at presentation size.
        assert!(html.contains("scale(var(--scale,1))"), "{html}");
    }
}
