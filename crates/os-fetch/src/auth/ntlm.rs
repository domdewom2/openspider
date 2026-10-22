//! NTLMv2 for HTTP, MS-NLMP 2.2.1 and 3.3.2. No NTLMv1 downgrade.
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
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..16].copy_from_slice(&Md5::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Md5::new();
    inner.update(block.map(|b| b ^ 0x36));
    inner.update(data);
    let mut outer = Md5::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
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

// RFC 1320 compression, used only by the NTLM password transform.
fn md4(input: &[u8]) -> [u8; 16] {
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&(input.len() as u64 * 8).to_le_bytes());
    let mut state = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476];
    for block in padded.as_chunks::<64>().0 {
        let words: Vec<u32> = block
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w))
            .collect();
        let mut v = state;
        for round in 0..3 {
            for step in 0..16 {
                let a = (16 - step) % 4;
                let [b, c, d] = [v[(a + 1) % 4], v[(a + 2) % 4], v[(a + 3) % 4]];
                let (f, k, add, rotation) = match round {
                    0 => ((b & c) | (!b & d), step, 0, [3, 7, 11, 19][step % 4]),
                    1 => (
                        (b & c) | (b & d) | (c & d),
                        (step % 4) * 4 + step / 4,
                        0x5a827999,
                        [3, 5, 9, 13][step % 4],
                    ),
                    _ => (
                        b ^ c ^ d,
                        [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15][step],
                        0x6ed9eba1,
                        [3, 9, 11, 15][step % 4],
                    ),
                };
                v[a] = v[a]
                    .wrapping_add(f)
                    .wrapping_add(words[k])
                    .wrapping_add(add)
                    .rotate_left(rotation);
            }
        }
        for i in 0..4 {
            state[i] = state[i].wrapping_add(v[i]);
        }
    }
    let mut out = [0; 16];
    for i in 0..4 {
        out[i * 4..i * 4 + 4].copy_from_slice(&state[i].to_le_bytes());
    }
    out
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
}
