//! The text forms `ipv4()`, `ipv6()` and `ip()` accept, as Raoh for Java's `IpSyntax` defines
//! them: decided by grammar, not by a parser of the platform, so the answer is the same everywhere.

/// A dotted quad: four decimal octets from 0 to 255, without leading zeros.
pub(crate) fn is_ipv4(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut pos = 0;
    for octet in 0..4 {
        if octet > 0 {
            if bytes.get(pos) != Some(&b'.') {
                return false;
            }
            pos += 1;
        }
        let start = pos;
        let mut number = 0u32;
        while pos < bytes.len() && pos - start < 3 && bytes[pos].is_ascii_digit() {
            number = number * 10 + u32::from(bytes[pos] - b'0');
            pos += 1;
        }
        let digits = pos - start;
        if digits == 0 || number > 255 || (digits > 1 && bytes[start] == b'0') {
            return false;
        }
    }
    pos == bytes.len()
}

/// An IPv6 address, optionally followed by a zone ID.
///
/// The address is the RFC 4291 section 2.2 text form, RFC 3986's `IPv6address`: eight groups of
/// one to four hexadecimal digits, at most one `::`, and optionally a dotted quad in place of the
/// last two groups. Brackets belong to the URI host syntax and are not accepted.
///
/// A zone ID (`fe80::1%eth0`, RFC 4007) is accepted when the address's scope is below global:
/// link-local unicast (`fe80::/10`) or multicast of scope 1 to D (RFC 4291 section 2.7 as updated
/// by RFC 7346). The zone ID is any non-empty text without `%` or NUL; it is not looked up among
/// the host's interfaces.
pub(crate) fn is_ipv6(s: &str) -> bool {
    match s.split_once('%') {
        None => is_ipv6_address(s),
        Some((address, zone)) => {
            !zone.is_empty()
                && !zone.contains(['%', '\0'])
                && is_ipv6_address(address)
                && can_have_zone(first_group(address))
        }
    }
}

fn is_ipv6_address(s: &str) -> bool {
    let bytes = s.as_bytes();
    let to = bytes.len();
    let mut groups = 0;
    let mut compressed = false;
    let mut pos = 0;
    if bytes.starts_with(b"::") {
        compressed = true;
        pos = 2;
        if pos == to {
            return true;
        }
    }
    loop {
        let start = pos;
        while pos < to && pos - start < 4 && bytes[pos].is_ascii_hexdigit() {
            pos += 1;
        }
        if bytes.get(pos) == Some(&b'.') {
            let room = if compressed {
                groups + 2 <= 7
            } else {
                groups + 2 == 8
            };
            return room && is_ipv4(&s[start..]);
        }
        if pos == start {
            return false;
        }
        groups += 1;
        if pos == to {
            return if compressed { groups <= 7 } else { groups == 8 };
        }
        if bytes[pos] != b':' {
            return false;
        }
        pos += 1;
        if bytes.get(pos) == Some(&b':') {
            if compressed {
                return false;
            }
            compressed = true;
            pos += 1;
            if pos == to {
                return groups <= 7;
            }
        }
        if groups > 8 {
            return false;
        }
    }
}

/// The value of the first 16-bit group of an address [`is_ipv6_address`] accepted.
fn first_group(address: &str) -> u32 {
    address
        .bytes()
        .take_while(|&b| b != b':')
        .fold(0, |group, b| {
            group * 16 + char::from(b).to_digit(16).unwrap_or(0)
        })
}

/// Whether a zone ID may follow an address with this first group: link-local unicast
/// (`fe80::/10`, RFC 4291 section 2.5.6), or multicast whose scope, the low four bits of the
/// second byte, is below global. Scope 0 is reserved, E is global and F is reserved and treated
/// as global. The loopback address `::1` does not take a zone.
fn can_have_zone(first_group: u32) -> bool {
    let first = first_group >> 8;
    let second = first_group & 0xff;
    if first == 0xfe && (second & 0xc0) == 0x80 {
        return true;
    }
    first == 0xff && !matches!(second & 0x0f, 0x0 | 0xe | 0xf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_rejects_leading_zeros_and_large_octets() {
        assert!(is_ipv4("192.168.0.1"));
        assert!(is_ipv4("0.0.0.0"));
        for bad in [
            "192.168.00.1",
            "256.0.0.1",
            "1.2.3",
            "1.2.3.4.",
            "1.2.3.4 ",
            "١.2.3.4",
        ] {
            assert!(!is_ipv4(bad), "{bad}");
        }
    }

    #[test]
    fn ipv6_follows_the_rfc_4291_text_form() {
        for ok in [
            "::",
            "::1",
            "2001:db8::1",
            "1:2:3:4:5:6:7:8",
            "::ffff:1.2.3.4",
            "::1.2.3.4",
            "1:2:3:4:5:6:1.2.3.4",
            "ABCD::ef",
        ] {
            assert!(is_ipv6(ok), "{ok}");
        }
        for bad in [
            "[::1]",
            "1::2::3",
            "::00001",
            "::01.2.3.4",
            "1:2:3:4:5:6:7:8:9",
            "1:2:3:4:5:6:7",
            "1:2:3:4:5:6:7:1.2.3.4",
            "2001:db8::g",
            "1.2.3.4",
            ":1",
            "1:",
            "",
        ] {
            assert!(!is_ipv6(bad), "{bad}");
        }
    }

    #[test]
    fn a_zone_is_taken_only_below_global_scope() {
        for ok in [
            "fe80::1%eth0",
            "fe80::1%3",
            "febf::1%x",
            "ff02::1%en0",
            "ff15::1%a",
        ] {
            assert!(is_ipv6(ok), "{ok}");
        }
        for bad in [
            "::1%lo0",
            "2001:db8::1%eth0",
            "fec0::1%eth0",
            "ff0e::1%eth0",
            "ff00::1%eth0",
            "fe80::1%",
            "fe80::1%a%b",
            "fe80::1%a\0",
        ] {
            assert!(!is_ipv6(bad), "{bad}");
        }
    }
}
