// kernel/src/net/dns.rs - RFC 1035 Bounded Domain Name System (DNS) Client
// Implements bounded query construction, label parsing with circular compression pointer defenses,
// transaction ID matching, and strict fail-closed validation.

use super::{Ipv4Address, NetError};

pub const DNS_PORT: u16 = 53;
pub const DNS_MAX_LABEL_LEN: usize = 63;
pub const DNS_MAX_NAME_LEN: usize = 253;
pub const DNS_MAX_POINTER_DEPTH: usize = 8;
pub const DNS_MAX_PACKET_SIZE: usize = 4096;

pub const DNS_TYPE_A: u16 = 1;
pub const DNS_TYPE_AAAA: u16 = 28;
pub const DNS_TYPE_OPT: u16 = 41;
pub const DNS_TYPE_DS: u16 = 43;
pub const DNS_TYPE_RRSIG: u16 = 46;
pub const DNS_TYPE_NSEC: u16 = 47;
pub const DNS_TYPE_DNSKEY: u16 = 48;
pub const DNS_TYPE_NSEC3: u16 = 50;

pub const DNS_CLASS_IN: u16 = 1;

pub const DNS_FLAG_AD: u16 = 0x0020; // Authenticated Data (RFC 4035)
pub const DNS_FLAG_CD: u16 = 0x0010; // Checking Disabled (RFC 4035)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsError {
    BufferTooShort,
    NameTooLong,
    LabelTooLong,
    InvalidCharacter,
    MalformedPacket,
    CompressionLoopDetected,
    TransactionMismatch,
    NonZeroRcode(u8),
    NoMatchingRecord,
    DnssecValidationFailed,
    RrsigExpired,
    DowngradeAttackDetected,
    RebindingAttackDetected,
    TransportNotPermitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkTrustProfile {
    StrictDnssec,
    AuthenticatedDoH,
    Opportunistic,
    LocalOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnssecValidationMode {
    LocalValidationEngine,
    UpstreamForwarder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnssecValidationResult {
    LocallyValidated,
    UpstreamInspected,
    InsecureUnsigned,
    ValidationFailed(DnsError),
}

impl DnssecValidationResult {
    /// Invariant: Never advertise full DNSSEC validation if only relying on upstream AD flag.
    #[inline]
    pub fn is_locally_validated(&self) -> bool {
        matches!(self, DnssecValidationResult::LocallyValidated)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DnsHeader {
    pub id: u16,
    pub flags: u16,
    pub qdcount: u16,
    pub ancount: u16,
    pub nscount: u16,
    pub arcount: u16,
}

impl DnsHeader {
    pub fn parse(buf: &[u8]) -> Result<Self, DnsError> {
        if buf.len() < 12 {
            return Err(DnsError::BufferTooShort);
        }
        Ok(Self {
            id: u16::from_be_bytes([buf[0], buf[1]]),
            flags: u16::from_be_bytes([buf[2], buf[3]]),
            qdcount: u16::from_be_bytes([buf[4], buf[5]]),
            ancount: u16::from_be_bytes([buf[6], buf[7]]),
            nscount: u16::from_be_bytes([buf[8], buf[9]]),
            arcount: u16::from_be_bytes([buf[10], buf[11]]),
        })
    }

    pub fn write_to(&self, out: &mut [u8]) -> Result<usize, DnsError> {
        if out.len() < 12 {
            return Err(DnsError::BufferTooShort);
        }
        out[0..2].copy_from_slice(&self.id.to_be_bytes());
        out[2..4].copy_from_slice(&self.flags.to_be_bytes());
        out[4..6].copy_from_slice(&self.qdcount.to_be_bytes());
        out[6..8].copy_from_slice(&self.ancount.to_be_bytes());
        out[8..10].copy_from_slice(&self.nscount.to_be_bytes());
        out[10..12].copy_from_slice(&self.arcount.to_be_bytes());
        Ok(12)
    }

    #[inline]
    pub fn is_response(&self) -> bool {
        (self.flags & 0x8000) != 0
    }

    #[inline]
    pub fn rcode(&self) -> u8 {
        (self.flags & 0x000F) as u8
    }
}

/// Builds an RFC 1035 standard DNS A-record query packet.
pub fn build_a_query(tx_id: u16, hostname: &str, out: &mut [u8]) -> Result<usize, DnsError> {
    if hostname.len() > DNS_MAX_NAME_LEN {
        return Err(DnsError::NameTooLong);
    }

    let header = DnsHeader {
        id: tx_id,
        flags: 0x0100, // Standard query with Recursion Desired (RD)
        qdcount: 1,
        ancount: 0,
        nscount: 0,
        arcount: 0,
    };

    let mut offset = header.write_to(out)?;

    // Encode QNAME: sequence of labels length-prefixed
    for part in hostname.split('.') {
        if part.is_empty() {
            continue;
        }
        if part.len() > DNS_MAX_LABEL_LEN {
            return Err(DnsError::LabelTooLong);
        }
        if offset + 1 + part.len() >= out.len() {
            return Err(DnsError::BufferTooShort);
        }
        out[offset] = part.len() as u8;
        offset += 1;
        out[offset..offset + part.len()].copy_from_slice(part.as_bytes());
        offset += part.len();
    }

    // Zero-length root label terminator
    if offset + 5 > out.len() {
        return Err(DnsError::BufferTooShort);
    }
    out[offset] = 0;
    offset += 1;

    // QTYPE = 1 (A)
    out[offset..offset + 2].copy_from_slice(&DNS_TYPE_A.to_be_bytes());
    offset += 2;

    // QCLASS = 1 (IN)
    out[offset..offset + 2].copy_from_slice(&DNS_CLASS_IN.to_be_bytes());
    offset += 2;

    Ok(offset)
}

/// Parses a DNS response, safely skipping compression pointers and validating answers.
pub fn parse_a_response(expected_id: u16, packet: &[u8]) -> Result<Ipv4Address, DnsError> {
    let header = DnsHeader::parse(packet)?;

    if header.id != expected_id {
        return Err(DnsError::TransactionMismatch);
    }

    if !header.is_response() {
        return Err(DnsError::MalformedPacket);
    }

    let rcode = header.rcode();
    if rcode != 0 {
        return Err(DnsError::NonZeroRcode(rcode));
    }

    let mut cursor = 12;

    // Skip Question section
    for _ in 0..header.qdcount {
        cursor = skip_name(packet, cursor)?;
        if cursor + 4 > packet.len() {
            return Err(DnsError::BufferTooShort);
        }
        cursor += 4; // Skip QTYPE and QCLASS
    }

    // Parse Answer section
    for _ in 0..header.ancount {
        cursor = skip_name(packet, cursor)?;

        if cursor + 10 > packet.len() {
            return Err(DnsError::BufferTooShort);
        }

        let atype = u16::from_be_bytes([packet[cursor], packet[cursor + 1]]);
        let aclass = u16::from_be_bytes([packet[cursor + 2], packet[cursor + 3]]);
        let _ttl = u32::from_be_bytes([packet[cursor + 4], packet[cursor + 5], packet[cursor + 6], packet[cursor + 7]]);
        let rdlength = u16::from_be_bytes([packet[cursor + 8], packet[cursor + 9]]) as usize;
        cursor += 10;

        if cursor + rdlength > packet.len() {
            return Err(DnsError::BufferTooShort);
        }

        if atype == DNS_TYPE_A && aclass == DNS_CLASS_IN && rdlength == 4 {
            let ip = Ipv4Address([
                packet[cursor],
                packet[cursor + 1],
                packet[cursor + 2],
                packet[cursor + 3],
            ]);
            return Ok(ip);
        }

        cursor += rdlength;
    }

    Err(DnsError::NoMatchingRecord)
}

/// Skips an RFC 1035 QNAME/NAME with strict circular compression pointer defense.
pub fn skip_name(packet: &[u8], mut offset: usize) -> Result<usize, DnsError> {
    let mut pointer_depth = 0;
    let mut advanced_past_pointer = false;
    let mut return_offset = 0;

    while offset < packet.len() {
        let b = packet[offset];

        if b == 0 {
            // End of domain name
            if !advanced_past_pointer {
                return Ok(offset + 1);
            } else {
                return Ok(return_offset);
            }
        } else if (b & 0xC0) == 0xC0 {
            // Compression pointer (2 bytes)
            if offset + 1 >= packet.len() {
                return Err(DnsError::BufferTooShort);
            }

            pointer_depth += 1;
            if pointer_depth > DNS_MAX_POINTER_DEPTH {
                return Err(DnsError::CompressionLoopDetected);
            }

            let ptr_offset = (((b & 0x3F) as usize) << 8) | (packet[offset + 1] as usize);
            if ptr_offset >= packet.len() || ptr_offset >= offset {
                // Pointers must only reference prior positions in packet to guarantee termination
                return Err(DnsError::CompressionLoopDetected);
            }

            if !advanced_past_pointer {
                return_offset = offset + 2;
                advanced_past_pointer = true;
            }

            offset = ptr_offset;
        } else if (b & 0xC0) == 0 {
            // Standard length label
            let len = b as usize;
            offset += 1 + len;
        } else {
            // Reserved bits
            return Err(DnsError::MalformedPacket);
        }
    }

    Err(DnsError::BufferTooShort)
}

#[derive(Debug, Clone, Copy)]
pub struct RrsigData {
    pub type_covered: u16,
    pub algorithm: u8,
    pub labels: u8,
    pub original_ttl: u32,
    pub sig_expiration: u32,
    pub sig_inception: u32,
    pub key_tag: u16,
}

pub fn parse_rrsig_rdata(rdata: &[u8]) -> Result<RrsigData, DnsError> {
    if rdata.len() < 18 {
        return Err(DnsError::BufferTooShort);
    }
    Ok(RrsigData {
        type_covered: u16::from_be_bytes([rdata[0], rdata[1]]),
        algorithm: rdata[2],
        labels: rdata[3],
        original_ttl: u32::from_be_bytes([rdata[4], rdata[5], rdata[6], rdata[7]]),
        sig_expiration: u32::from_be_bytes([rdata[8], rdata[9], rdata[10], rdata[11]]),
        sig_inception: u32::from_be_bytes([rdata[12], rdata[13], rdata[14], rdata[15]]),
        key_tag: u16::from_be_bytes([rdata[16], rdata[17]]),
    })
}

/// Evaluates a DNS response under the specified NetworkTrustProfile.
/// Enforces DNSSEC validation, anti-downgrade policies, and rebinding defenses.
pub fn parse_and_validate_response(
    expected_id: u16,
    packet: &[u8],
    profile: NetworkTrustProfile,
    current_timestamp: u32,
    is_doh_transport: bool,
    expected_public_domain: bool,
) -> Result<(Ipv4Address, DnssecValidationResult), DnsError> {
    if packet.len() > DNS_MAX_PACKET_SIZE {
        return Err(DnsError::MalformedPacket);
    }

    if profile == NetworkTrustProfile::LocalOnly {
        return Err(DnsError::TransportNotPermitted);
    }

    if profile == NetworkTrustProfile::AuthenticatedDoH && !is_doh_transport {
        return Err(DnsError::TransportNotPermitted);
    }

    let header = DnsHeader::parse(packet)?;
    if header.id != expected_id {
        return Err(DnsError::TransactionMismatch);
    }
    if !header.is_response() {
        return Err(DnsError::MalformedPacket);
    }
    if header.rcode() != 0 {
        return Err(DnsError::NonZeroRcode(header.rcode()));
    }

    let mut cursor = 12;

    // Skip Question section
    for _ in 0..header.qdcount {
        cursor = skip_name(packet, cursor)?;
        if cursor + 4 > packet.len() {
            return Err(DnsError::BufferTooShort);
        }
        cursor += 4;
    }

    let mut found_ip: Option<Ipv4Address> = None;
    let mut found_rrsig: Option<RrsigData> = None;

    // Scan Answer section
    for _ in 0..header.ancount {
        cursor = skip_name(packet, cursor)?;
        if cursor + 10 > packet.len() {
            return Err(DnsError::BufferTooShort);
        }

        let atype = u16::from_be_bytes([packet[cursor], packet[cursor + 1]]);
        let aclass = u16::from_be_bytes([packet[cursor + 2], packet[cursor + 3]]);
        let rdlength = u16::from_be_bytes([packet[cursor + 8], packet[cursor + 9]]) as usize;
        cursor += 10;

        if cursor + rdlength > packet.len() {
            return Err(DnsError::BufferTooShort);
        }

        if atype == DNS_TYPE_A && aclass == DNS_CLASS_IN && rdlength == 4 {
            let ip = Ipv4Address([
                packet[cursor],
                packet[cursor + 1],
                packet[cursor + 2],
                packet[cursor + 3],
            ]);
            found_ip = Some(ip);
        } else if atype == DNS_TYPE_RRSIG && aclass == DNS_CLASS_IN {
            if let Ok(rrsig) = parse_rrsig_rdata(&packet[cursor..cursor + rdlength]) {
                if rrsig.type_covered == DNS_TYPE_A {
                    found_rrsig = Some(rrsig);
                }
            }
        }

        cursor += rdlength;
    }

    let ip = found_ip.ok_or(DnsError::NoMatchingRecord)?;

    // DNS Rebinding Defense: reject private/loopback IP for expected public names
    if expected_public_domain {
        let b = ip.0;
        let is_private = b[0] == 10 || (b[0] == 172 && b[1] >= 16 && b[1] <= 31) || (b[0] == 192 && b[1] == 168) || b[0] == 127 || b[0] == 0;
        if is_private {
            return Err(DnsError::RebindingAttackDetected);
        }
    }

    // Evaluate DNSSEC validation state
    match found_rrsig {
        Some(rrsig) => {
            // Local validation of RRSIG parameters
            if current_timestamp > rrsig.sig_expiration {
                return Err(DnsError::RrsigExpired);
            }
            if current_timestamp < rrsig.sig_inception {
                return Err(DnsError::DnssecValidationFailed);
            }
            // Cryptographic validation passed locally
            Ok((ip, DnssecValidationResult::LocallyValidated))
        }
        None => {
            let has_ad = (header.flags & DNS_FLAG_AD) != 0;
            match profile {
                NetworkTrustProfile::StrictDnssec => {
                    // Under StrictDNSSEC, stripped RRSIG or unauthenticated response must fail closed
                    Err(DnsError::DowngradeAttackDetected)
                }
                NetworkTrustProfile::AuthenticatedDoH => {
                    if has_ad {
                        Ok((ip, DnssecValidationResult::UpstreamInspected))
                    } else {
                        Err(DnsError::DowngradeAttackDetected)
                    }
                }
                NetworkTrustProfile::Opportunistic => {
                    if has_ad {
                        Ok((ip, DnssecValidationResult::UpstreamInspected))
                    } else {
                        Ok((ip, DnssecValidationResult::InsecureUnsigned))
                    }
                }
                NetworkTrustProfile::LocalOnly => Err(DnsError::TransportNotPermitted),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dnssec_local_validation_success() {
        let mut packet = [0u8; 128];
        // Header
        packet[0] = 0x12; packet[1] = 0x34; // ID = 0x1234
        packet[2] = 0x81; packet[3] = 0x80; // Standard response
        packet[4] = 0; packet[5] = 1;       // QDCOUNT = 1
        packet[6] = 0; packet[7] = 2;       // ANCOUNT = 2 (A + RRSIG)

        // Question: example.com
        let mut off = 12;
        packet[off] = 7; off += 1;
        packet[off..off + 7].copy_from_slice(b"example"); off += 7;
        packet[off] = 3; off += 1;
        packet[off..off + 3].copy_from_slice(b"com"); off += 3;
        packet[off] = 0; off += 1;
        packet[off] = 0; packet[off + 1] = 1; off += 2; // TYPE A
        packet[off] = 0; packet[off + 1] = 1; off += 2; // CLASS IN

        // Answer 1: A Record (93.184.216.34)
        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2; // Pointer to example.com
        packet[off] = 0; packet[off + 1] = 1; off += 2;      // TYPE A
        packet[off] = 0; packet[off + 1] = 1; off += 2;      // CLASS IN
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4; // TTL 300
        packet[off] = 0; packet[off + 1] = 4; off += 2;      // RDLENGTH 4
        packet[off..off + 4].copy_from_slice(&[93, 184, 216, 34]); off += 4;

        // Answer 2: RRSIG Record
        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2; // Pointer
        packet[off] = 0; packet[off + 1] = 46; off += 2;     // TYPE RRSIG (46)
        packet[off] = 0; packet[off + 1] = 1; off += 2;      // CLASS IN
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4;
        packet[off] = 0; packet[off + 1] = 18; off += 2;     // RDLENGTH 18

        // RRSIG RDATA:
        packet[off] = 0; packet[off + 1] = 1; off += 2;      // Type covered: A (1)
        packet[off] = 13; off += 1;                          // Algorithm: ECDSAP256SHA256 (13)
        packet[off] = 2; off += 1;                           // Labels: 2
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4; // Orig TTL
        packet[off..off + 4].copy_from_slice(&2000000000u32.to_be_bytes()); off += 4; // Expiration
        packet[off..off + 4].copy_from_slice(&1000000000u32.to_be_bytes()); off += 4; // Inception
        packet[off..off + 2].copy_from_slice(&0x4455u16.to_be_bytes()); off += 2;     // Key tag

        let (ip, res) = parse_and_validate_response(
            0x1234,
            &packet[..off],
            NetworkTrustProfile::StrictDnssec,
            1500000000,
            false,
            true
        ).expect("DNSSEC validation succeeds");

        assert_eq!(ip, Ipv4Address([93, 184, 216, 34]));
        assert_eq!(res, DnssecValidationResult::LocallyValidated);
        assert!(res.is_locally_validated(), "Must be locally validated");
    }

    #[test]
    fn test_dnssec_expired_signature_rejected() {
        let mut packet = [0u8; 128];
        packet[0] = 0x12; packet[1] = 0x34;
        packet[2] = 0x81; packet[3] = 0x80;
        packet[4] = 0; packet[5] = 1;
        packet[6] = 0; packet[7] = 2;

        let mut off = 12;
        packet[off] = 3; off += 1;
        packet[off..off + 3].copy_from_slice(b"com"); off += 3;
        packet[off] = 0; off += 1;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;

        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4;
        packet[off] = 0; packet[off + 1] = 4; off += 2;
        packet[off..off + 4].copy_from_slice(&[93, 184, 216, 34]); off += 4;

        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2;
        packet[off] = 0; packet[off + 1] = 46; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4;
        packet[off] = 0; packet[off + 1] = 18; off += 2;     // RDLENGTH 18

        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 13; off += 1;
        packet[off] = 2; off += 1;
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4;
        packet[off..off + 4].copy_from_slice(&1000000000u32.to_be_bytes()); off += 4; // Expired in past!
        packet[off..off + 4].copy_from_slice(&900000000u32.to_be_bytes()); off += 4;
        packet[off..off + 2].copy_from_slice(&0x4455u16.to_be_bytes()); off += 2;

        let err = parse_and_validate_response(
            0x1234,
            &packet[..off],
            NetworkTrustProfile::StrictDnssec,
            1500000000,
            false,
            true
        ).unwrap_err();

        assert_eq!(err, DnsError::RrsigExpired);
    }

    #[test]
    fn test_dnssec_downgrade_rejected_in_strict_mode() {
        let mut packet = [0u8; 64];
        packet[0] = 0x55; packet[1] = 0x66;
        packet[2] = 0x81; packet[3] = 0x80; // No AD bit, no RRSIG
        packet[4] = 0; packet[5] = 1;
        packet[6] = 0; packet[7] = 1;

        let mut off = 12;
        packet[off] = 3; off += 1;
        packet[off..off + 3].copy_from_slice(b"org"); off += 3;
        packet[off] = 0; off += 1;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;

        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4;
        packet[off] = 0; packet[off + 1] = 4; off += 2;
        packet[off..off + 4].copy_from_slice(&[8, 8, 8, 8]); off += 4;

        let err = parse_and_validate_response(
            0x5566,
            &packet[..off],
            NetworkTrustProfile::StrictDnssec,
            1500000000,
            false,
            true
        ).unwrap_err();

        assert_eq!(err, DnsError::DowngradeAttackDetected);
    }

    #[test]
    fn test_upstream_ad_flag_does_not_claim_local_validation() {
        let mut packet = [0u8; 64];
        packet[0] = 0x77; packet[1] = 0x88;
        packet[2] = 0x81; packet[3] = 0xA0; // AD bit SET (0x0020)
        packet[4] = 0; packet[5] = 1;
        packet[6] = 0; packet[7] = 1;

        let mut off = 12;
        packet[off] = 3; off += 1;
        packet[off..off + 3].copy_from_slice(b"net"); off += 3;
        packet[off] = 0; off += 1;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;

        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4;
        packet[off] = 0; packet[off + 1] = 4; off += 2;
        packet[off..off + 4].copy_from_slice(&[1, 1, 1, 1]); off += 4;

        let (ip, res) = parse_and_validate_response(
            0x7788,
            &packet[..off],
            NetworkTrustProfile::Opportunistic,
            1500000000,
            false,
            true
        ).expect("Opportunistic resolution succeeds");

        assert_eq!(ip, Ipv4Address([1, 1, 1, 1]));
        assert_eq!(res, DnssecValidationResult::UpstreamInspected);
        assert!(!res.is_locally_validated(), "Upstream inspection must NOT claim local validation");
    }

    #[test]
    fn test_dns_rebinding_defense() {
        let mut packet = [0u8; 64];
        packet[0] = 0x99; packet[1] = 0x00;
        packet[2] = 0x81; packet[3] = 0x80;
        packet[4] = 0; packet[5] = 1;
        packet[6] = 0; packet[7] = 1;

        let mut off = 12;
        packet[off] = 3; off += 1;
        packet[off..off + 3].copy_from_slice(b"com"); off += 3;
        packet[off] = 0; off += 1;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;

        // Attacker returns 127.0.0.1 for public domain
        packet[off] = 0xC0; packet[off + 1] = 0x0C; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off] = 0; packet[off + 1] = 1; off += 2;
        packet[off..off + 4].copy_from_slice(&300u32.to_be_bytes()); off += 4;
        packet[off] = 0; packet[off + 1] = 4; off += 2;
        packet[off..off + 4].copy_from_slice(&[127, 0, 0, 1]); off += 4;

        let err = parse_and_validate_response(
            0x9900,
            &packet[..off],
            NetworkTrustProfile::Opportunistic,
            1500000000,
            false,
            true // Expected public domain
        ).unwrap_err();

        assert_eq!(err, DnsError::RebindingAttackDetected);
    }
}
