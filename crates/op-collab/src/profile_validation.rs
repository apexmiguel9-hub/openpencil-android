//! Deceptive-profile rejection for owner-authenticated roster metadata.
//!
//! Display names and avatar URLs are rendered and fetched by every
//! participant's client, so an admitted peer can aim them at other
//! participants. This module rejects the two shapes that abuse allows: a name
//! that renders identically to (or visually reverses) another roster entry,
//! and an avatar URL that points every fetching client at a non-public network
//! address.

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::{
    MAX_COLLAB_PROFILE_AVATAR_URL_BYTES, MAX_COLLAB_PROFILE_DISPLAY_NAME_BYTES,
    MAX_COLLAB_PROFILE_DISPLAY_NAME_CHARS,
};

pub(crate) fn valid_profile_display_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_COLLAB_PROFILE_DISPLAY_NAME_BYTES
        && value.chars().count() <= MAX_COLLAB_PROFILE_DISPLAY_NAME_CHARS
        && value.trim() == value
        && !value.chars().any(is_non_graphic)
}

/// Rejects code points that draw nothing of their own, or that reorder the
/// glyphs around them, so a roster name always renders as the characters it
/// contains.
///
/// [`char::is_control`] covers Unicode category Cc. The explicit set below is
/// category Cf (format) plus the line/paragraph separators: `std` exposes no
/// stable general-category query, and this crate stays dependency-light and
/// wasm32-clean, so the ranges are listed literally instead of pulling in a
/// `unicode-*` table.
///
/// ZERO WIDTH JOINER (U+200D) and the tag characters (U+E0020..=U+E007F) are
/// inside the rejected set even though emoji ZWJ and subdivision-flag
/// sequences use them. An invisible code point admitted anywhere in a name is
/// exactly the roster-spoofing primitive this check exists to remove, and the
/// duplicate check keys on participant id rather than name. Ordinary emoji,
/// CJK, Cyrillic, Arabic, and accented Latin names are unaffected.
fn is_non_graphic(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            // SOFT HYPHEN
            '\u{00ad}'
            // Arabic number/format signs and ARABIC LETTER MARK
            | '\u{0600}'..='\u{0605}'
            | '\u{061c}'
            | '\u{06dd}'
            // SYRIAC ABBREVIATION MARK and Arabic script format signs
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            // MONGOLIAN VOWEL SEPARATOR
            | '\u{180e}'
            // Zero-width space/joiners plus the LEFT/RIGHT-TO-LEFT MARKs
            | '\u{200b}'..='\u{200f}'
            // Line/paragraph separators plus the bidi embeddings and overrides
            | '\u{2028}'..='\u{202e}'
            // WORD JOINER, invisible operators, isolates, deprecated formats
            | '\u{2060}'..='\u{206f}'
            // ZERO WIDTH NO-BREAK SPACE (byte-order mark)
            | '\u{feff}'
            // Interlinear annotation anchors
            | '\u{fff9}'..='\u{fffb}'
            // Supplementary-plane format controls
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            // LANGUAGE TAG and the tag characters
            | '\u{e0000}'..='\u{e007f}'
        )
}

pub(crate) fn valid_profile_avatar_url(value: &str) -> bool {
    if value.is_empty()
        || value.len() > MAX_COLLAB_PROFILE_AVATAR_URL_BYTES
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        || value.contains(['#', '\\'])
    {
        return false;
    }
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = rest
        .split_once(['/', '?'])
        .map_or(rest, |(authority, _)| authority);
    valid_https_authority(authority)
}

fn valid_https_authority(authority: &str) -> bool {
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    if let Some(bracketed) = authority.strip_prefix('[') {
        let Some(closing_bracket) = bracketed.find(']') else {
            return false;
        };
        let host = &bracketed[..closing_bracket];
        let suffix = &bracketed[closing_bracket + 1..];
        return host.parse::<Ipv6Addr>().is_ok_and(globally_routable_ipv6)
            && (suffix.is_empty() || suffix.strip_prefix(':').is_some_and(valid_https_port));
    }
    if authority.contains('[') || authority.contains(']') {
        return false;
    }
    let host = match authority.rsplit_once(':') {
        Some((host, port)) => {
            if host.contains(':') || !valid_https_port(port) {
                return false;
            }
            host
        }
        None => authority,
    };
    valid_dns_or_ipv4_host(host)
}

fn valid_https_port(port: &str) -> bool {
    !port.is_empty()
        && port.bytes().all(|byte| byte.is_ascii_digit())
        && port.parse::<u16>().is_ok_and(|port| port != 0)
}

/// Accepts a DNS host name, or an IPv4 literal that is globally routable.
///
/// DNS names keep their existing behaviour on purpose: a name can still
/// resolve to a private address, and refusing that belongs to the fetch layer,
/// which is the only layer that sees the resolved address. This crate is
/// transport-free and resolves nothing. Rejecting literals still removes the
/// direct case, where an admitted peer names an internal endpoint outright.
fn valid_dns_or_ipv4_host(host: &str) -> bool {
    if let Ok(address) = host.parse::<Ipv4Addr>() {
        return globally_routable_ipv4(address);
    }
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
        })
}

/// Whether an IPv4 literal addresses the public internet.
///
/// Only stable inherent methods are used (`is_global` is still unstable), with
/// the remaining non-routable blocks spelled out.
fn globally_routable_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    !(address.is_unspecified()
        // 127.0.0.0/8
        || address.is_loopback()
        // 10/8, 172.16/12, 192.168/16
        || address.is_private()
        // 169.254/16, which carries the cloud instance-metadata endpoint
        || address.is_link_local()
        // 224.0.0.0/4
        || address.is_multicast()
        // 255.255.255.255
        || address.is_broadcast()
        // 192.0.2/24, 198.51.100/24, 203.0.113/24
        || address.is_documentation()
        // 0.0.0.0/8 "this network"
        || first == 0
        // 100.64.0.0/10 carrier-grade NAT
        || (first == 100 && (64..128).contains(&second))
        // 240.0.0.0/4 reserved
        || first >= 240)
}

/// Whether an IPv6 literal addresses the public internet.
///
/// The IPv4 aliases are resolved first so `::ffff:10.0.0.1` cannot smuggle a
/// private target past the IPv4 rules.
fn globally_routable_ipv6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    // ::a.b.c.d (IPv4-compatible, includes `::` and `::1`) and ::ffff:a.b.c.d
    // (IPv4-mapped) both address IPv4 space.
    if segments[..5] == [0, 0, 0, 0, 0] && (segments[5] == 0 || segments[5] == 0xffff) {
        return globally_routable_ipv4(embedded_ipv4(segments[6], segments[7]));
    }
    // 64:ff9b::/96 reaches IPv4 space through the well-known NAT64 prefix.
    if segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2..6] == [0, 0, 0, 0] {
        return globally_routable_ipv4(embedded_ipv4(segments[6], segments[7]));
    }
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        // fe80::/10 link-local
        || segments[0] & 0xffc0 == 0xfe80
        // fc00::/7 unique-local
        || segments[0] & 0xfe00 == 0xfc00
        // 2001:db8::/32 documentation
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn embedded_ipv4(high: u16, low: u16) -> Ipv4Addr {
    Ipv4Addr::from((u32::from(high) << 16) | u32::from(low))
}
