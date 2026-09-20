//! Figma-parity property-panel key coverage across every locale table.

type Lookup = fn(&str) -> Option<&'static str>;

const KEYS: [&str; 29] = [
    "page.background",
    "page.background.clear",
    "property.swapComponent",
    "appearance.blendMode",
    "appearance.maskType",
    "layer.blendMode",
    "layer.maskType",
    "fill.blendMode",
    "image.tileScale",
    "blendMode.normal",
    "blendMode.darken",
    "blendMode.multiply",
    "blendMode.screen",
    "blendMode.overlay",
    "blendMode.lighten",
    "blendMode.difference",
    "blendMode.hue",
    "blendMode.saturation",
    "blendMode.color",
    "blendMode.luminosity",
    "blendMode.softLight",
    "blendMode.colorDodge",
    "blendMode.colorBurn",
    "blendMode.hardLight",
    "blendMode.exclusion",
    "maskType.none",
    "maskType.alpha",
    "maskType.vector",
    "maskType.luminance",
];

#[test]
fn figma_property_keys_exist_directly_in_every_locale_table() {
    let tables: [(&str, Lookup); 15] = [
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
    ];

    for (name, lookup) in tables {
        for key in KEYS {
            let value =
                lookup(key).unwrap_or_else(|| panic!("locale table `{name}` is missing `{key}`"));
            assert!(
                !value.is_empty(),
                "locale table `{name}` has an empty value for `{key}`"
            );
        }
    }
}
