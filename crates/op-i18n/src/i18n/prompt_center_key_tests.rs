//! Prompt-center key coverage across every locale table.

type Lookup = fn(&str) -> Option<&'static str>;

const KEYS: [&str; 53] = [
    "promptCenter.title",
    "promptCenter.searchPlaceholder",
    "promptCenter.category.all",
    "promptCenter.category.starter",
    "promptCenter.category.mobileApp",
    "promptCenter.category.webPage",
    "promptCenter.category.dashboard",
    "promptCenter.category.component",
    "promptCenter.category.modify",
    "promptCenter.category.custom",
    "promptCenter.empty",
    "promptCenter.saveCurrent",
    "promptCenter.saveTitlePlaceholder",
    "promptCenter.save",
    "promptCenter.cancel",
    "promptCenter.delete",
    "promptCenter.screens",
    "promptCenter.freeform",
    "promptCenter.item.wander.title",
    "promptCenter.item.forage.title",
    "promptCenter.item.still.title",
    "promptCenter.item.hearth.title",
    "promptCenter.item.meteo.title",
    "promptCenter.item.marginalia.title",
    "promptCenter.item.lingua.title",
    "promptCenter.item.daybreak.title",
    "promptCenter.item.verdant.title",
    "promptCenter.item.companion.title",
    "promptCenter.item.relic.title",
    "promptCenter.item.nocturne.title",
    "promptCenter.item.marquee.title",
    "promptCenter.item.ritual.title",
    "promptCenter.item.ember.title",
    "promptCenter.item.volt.title",
    "promptCenter.item.aloft.title",
    "promptCenter.item.gallery.title",
    "promptCenter.item.nightcap.title",
    "promptCenter.item.bloom.title",
    "promptCenter.item.extremeWeather.title",
    "promptCenter.item.extremeNowPlaying.title",
    "promptCenter.item.extremeDailyApp.title",
    "promptCenter.item.extremeCalendar.title",
    "promptCenter.item.extremeCalm.title",
    "promptCenter.item.webOrbit.title",
    "promptCenter.item.webAtelier.title",
    "promptCenter.item.webKilnform.title",
    "promptCenter.item.webReefwright.title",
    "promptCenter.item.dashboardPulse.title",
    "promptCenter.item.dashboardSentinel.title",
    "promptCenter.item.componentDataGrid.title",
    "promptCenter.item.componentFormLab.title",
    "promptCenter.item.modifyPolishCurrent.title",
    "promptCenter.item.modifyCompleteStates.title",
];

#[test]
fn prompt_center_keys_exist_directly_in_every_locale_table() {
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
            assert_ne!(
                value, key,
                "locale table `{name}` falls back to the raw key `{key}`"
            );
        }
    }
}
