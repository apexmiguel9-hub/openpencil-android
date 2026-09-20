//! HTML/ZIP import copy coverage across every locale table.

type Lookup = fn(&str) -> Option<&'static str>;

fn tables() -> [(&'static str, Lookup); 15] {
    [
        ("en", super::en::lookup),
        ("zh_cn", super::zh_cn::lookup),
        ("zh_tw", super::zh_tw::lookup),
        ("ja", super::ja::lookup),
        ("ko", super::ko::lookup),
        ("fr", super::fr::lookup),
        ("es", super::es::lookup),
        ("de", super::de::lookup),
        ("pt", super::pt::lookup),
        ("ru", super::ru::lookup),
        ("hi", super::hi::lookup),
        ("tr", super::tr::lookup),
        ("th", super::th::lookup),
        ("vi", super::vi::lookup),
        ("id", super::id::lookup),
    ]
}

#[test]
fn every_locale_advertises_html_and_zip_imports_directly() {
    for (name, lookup) in tables() {
        let drop = lookup("html.dropFile")
            .unwrap_or_else(|| panic!("locale table `{name}` is missing `html.dropFile`"));
        for extension in [".html", ".htm", ".zip"] {
            assert!(
                drop.contains(extension),
                "locale table `{name}` omits `{extension}` from `html.dropFile`"
            );
        }

        let tip = lookup("html.saveTip")
            .unwrap_or_else(|| panic!("locale table `{name}` is missing `html.saveTip`"));
        assert!(
            tip.contains(".zip"),
            "locale table `{name}` omits `.zip` from `html.saveTip`"
        );

        let overlay = lookup("dialog.dropToOpen")
            .unwrap_or_else(|| panic!("locale table `{name}` is missing `dialog.dropToOpen`"));
        for extension in [".html", ".zip"] {
            assert!(
                overlay.contains(extension),
                "locale table `{name}` omits `{extension}` from `dialog.dropToOpen`"
            );
        }
    }
}

#[test]
fn primary_locale_copy_describes_page_and_project_imports_naturally() {
    assert_eq!(
        super::en::lookup("html.title"),
        Some("Import HTML or web project")
    );
    assert_eq!(
        super::zh_cn::lookup("html.title"),
        Some("导入 HTML 或网页项目")
    );
    assert_eq!(
        super::zh_tw::lookup("html.title"),
        Some("匯入 HTML 或網頁專案")
    );
}
