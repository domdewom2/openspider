//! NTLMv2 for HTTP, MS-NLMP 2.2.1 and 3.3.2. No NTLMv1 downgrade.
use hmac::{Hmac, KeyInit, Mac};
use md4::Md4;
use md5::{Digest, Md5};

/// Consume a challenge without buffering an untrusted response body.
pub(crate) async fn drain(mut response: reqwest::Response) -> Result<(), reqwest::Error> {
    while response.chunk().await?.is_some() {}
    Ok(())
}

pub(crate) fn negotiate() -> Vec<u8> {
    let mut out = b"NTLMSSP\0\x01\0\0\0".to_vec();
    out.extend_from_slice(&0x0088_0205u32.to_le_bytes()); // Unicode, request target, NTLM, ESS, target info
    out.resize(32, 0);
    out
}
fn utf16(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn u16_at(input: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(input.get(at..at + 2)?.try_into().ok()?))
}
fn u32_at(input: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(input.get(at..at + 4)?.try_into().ok()?))
}
fn buffer(input: &[u8], at: usize) -> Option<&[u8]> {
    let len = usize::from(u16_at(input, at)?);
    let offset = usize::try_from(u32_at(input, at + 4)?).ok()?;
    input.get(offset..offset.checked_add(len)?)
}
fn field(out: &mut Vec<u8>, at: usize, value: &[u8]) -> Option<()> {
    let len = u16::try_from(value.len()).ok()?.to_le_bytes();
    let offset = u32::try_from(out.len()).ok()?.to_le_bytes();
    out[at..at + 2].copy_from_slice(&len);
    out[at + 2..at + 4].copy_from_slice(&len);
    out[at + 4..at + 8].copy_from_slice(&offset);
    out.extend_from_slice(value);
    Some(())
}
fn hmac(key: &[u8], data: &[u8]) -> [u8; 16] {
    let mut mac = Hmac::<Md5>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}
/// Inputs for time and nonce are supplied by the caller, so vectors are reproducible.
pub(crate) fn authenticate(
    challenge: &[u8],
    user: &str,
    password: &str,
    nonce: [u8; 8],
    time: u64,
) -> Option<Vec<u8>> {
    if challenge.get(..12)? != b"NTLMSSP\0\x02\0\0\0" {
        return None;
    }
    let flags = u32_at(challenge, 20)?;
    if flags & 0x0088_0201 != 0x0088_0201 {
        return None;
    }
    let server_nonce = challenge.get(24..32)?;
    let info = buffer(challenge, 40)?;
    let mut pairs = Vec::new();
    let mut timestamp = None;
    let mut av_flags = 0;
    let mut at = 0;
    loop {
        let id = u16_at(info, at)?;
        let len = usize::from(u16_at(info, at + 2)?);
        let value = info.get(at + 4..at + 4 + len)?;
        at += 4 + len;
        if id == 0 {
            if len != 0 || at != info.len() {
                return None;
            }
            break;
        }
        if id == 7 {
            timestamp = Some(u64::from_le_bytes(value.try_into().ok()?));
        } else if id == 6 {
            av_flags = u32::from_le_bytes(value.try_into().ok()?);
        } else {
            pairs.extend_from_slice(&info[at - len - 4..at]);
        }
    }
    let mic = timestamp.is_some() || av_flags & 2 != 0;
    if mic {
        av_flags |= 2;
    }
    if av_flags != 0 {
        pairs.extend_from_slice(&[6, 0, 4, 0]);
        pairs.extend_from_slice(&av_flags.to_le_bytes());
    }
    if let Some(timestamp) = timestamp {
        pairs.extend_from_slice(&[7, 0, 8, 0]);
        pairs.extend_from_slice(&timestamp.to_le_bytes());
    }
    pairs.extend_from_slice(&[0; 4]);
    let (domain, user) = user.split_once('\\').unwrap_or(("", user));
    let key = hmac(
        &md4(&utf16(password)),
        &utf16(&format!("{}{domain}", user.to_uppercase())),
    );
    let mut blob = vec![1, 1, 0, 0, 0, 0, 0, 0];
    blob.extend_from_slice(&timestamp.unwrap_or(time).to_le_bytes());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&[0; 4]);
    blob.extend_from_slice(&pairs);
    blob.extend_from_slice(&[0; 4]);
    let proof = hmac(&key, &[server_nonce, &blob].concat());
    let nt = [&proof[..], &blob].concat();
    let lm = if timestamp.is_some() {
        vec![0; 24]
    } else {
        [&hmac(&key, &[server_nonce, &nonce].concat())[..], &nonce].concat()
    };
    // Version is absent from negotiated flags; the reserved version slot precedes MIC.
    let mut out = vec![0; if mic { 88 } else { 64 }];
    out[..12].copy_from_slice(b"NTLMSSP\0\x03\0\0\0");
    out[60..64].copy_from_slice(&(flags & 0x0088_0205).to_le_bytes());
    field(&mut out, 12, &lm)?;
    field(&mut out, 20, &nt)?;
    field(&mut out, 28, &utf16(domain))?;
    field(&mut out, 36, &utf16(user))?;
    field(&mut out, 44, &[])?;
    field(&mut out, 52, &[])?;
    if mic {
        let session_key = hmac(&key, &proof);
        let signature = hmac(&session_key, &[&negotiate()[..], challenge, &out].concat());
        out[72..88].copy_from_slice(&signature);
    }
    Some(out)
}

// NTOWFv1 password transform from MS-NLMP 3.3.1, implemented by RustCrypto.
fn md4(input: &[u8]) -> [u8; 16] {
    Md4::digest(input).into()
}

pub(crate) fn decode(value: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut bits = 0u32;
    let mut count = 0;
    for c in value.trim_end_matches('=').bytes() {
        let n = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(n);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    // MS-NLMP 4.2.2.1.2 (NTOWFv1), 4.2.4.1.1 (NTOWFv2), plus RFC 1320/2202.
    // https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-nlmp/7795bd0e-fd5e-43ec-bd9c-994704d8ee26
    // spec: CRAWL-121 [M3]
    // spec: OUT-223
    #[test]
    fn ntlm_password_and_response_key_match_published_vectors() {
        assert_eq!(hex::encode(md4(b"")), "31d6cfe0d16ae931b73c59d7e0c089c0");
        assert_eq!(
            hex::encode(md4(&utf16("Password"))),
            "a4f49c406510bdcab6824ee7c30fd852"
        );
        assert_eq!(
            hex::encode(hmac(&md4(&utf16("Password")), &utf16("USERDomain"))),
            "0c868a403bfd7a93a3001ef22ef02e3f"
        );
        assert_eq!(
            hex::encode(md4(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            )),
            "e33b4ddc9c38f2199c3e7b164fcc0536"
        );
        assert_eq!(
            hex::encode(hmac(&[0x0b; 16], b"Hi There")),
            "9294727a3638bb1c13f48ef8158bfc9d"
        );
        assert!(authenticate(b"invalid", "user", "password", [0; 8], 0).is_none());
    }

    // MS-NLMP 4.2.4.1.2 and 4.2.4.2.1–2, with the example's fixed time and nonces.
    // https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-nlmp/54973495-20d2-49e8-9925-c399a403ed4a
    // spec: CRAWL-121 [M3]
    #[test]
    fn ntlm_v2_response_and_session_key_match_specification() {
        let info =
            hex::decode("02000c0044006f006d00610069006e0001000c0053006500720076006500720000000000")
                .unwrap();
        let mut challenge = vec![0; 48];
        challenge[..12].copy_from_slice(b"NTLMSSP\0\x02\0\0\0");
        challenge[20..24].copy_from_slice(&0x0088_0201u32.to_le_bytes());
        challenge[24..32].copy_from_slice(&hex::decode("0123456789abcdef").unwrap());
        let len = u16::try_from(info.len()).unwrap().to_le_bytes();
        challenge[40..42].copy_from_slice(&len);
        challenge[42..44].copy_from_slice(&len);
        challenge[44..48].copy_from_slice(&48u32.to_le_bytes());
        challenge.extend(info);
        let message = authenticate(&challenge, "Domain\\User", "Password", [0xaa; 8], 0).unwrap();
        assert_eq!(
            hex::encode(buffer(&message, 12).unwrap()),
            "86c35097ac9cec102554764a57cccc19aaaaaaaaaaaaaaaa"
        );
        let response = buffer(&message, 20).unwrap();
        assert_eq!(
            hex::encode(&response[..16]),
            "68cd0ab851e51c96aabc927bebef6a1c"
        );
        let key = hmac(&md4(&utf16("Password")), &utf16("USERDomain"));
        assert_eq!(
            hex::encode(hmac(&key, &response[..16])),
            "8de40ccadbc14a82f15cb0ad0de95ca3"
        );
    }

    // RFC 2202 test case 6 exercises HMAC's long-key hashing path.
    // spec: CRAWL-121 [M3]
    #[test]
    fn hmac_md5_accepts_keys_longer_than_one_block() {
        assert_eq!(
            hex::encode(hmac(
                &[0xaa; 80],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "6b1ab7fe4bd7bf8f0b62e6ce61b9d0cd"
        );
    }
}
