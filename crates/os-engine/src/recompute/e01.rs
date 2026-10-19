//! Hash-Neuberechnung aus den unveränderten gespeicherten Response-Bytes (ONPAGE-143).
use crate::EngineError;
use os_model::{CancelToken, CrawlConfig, UrlId};
use os_store::{
    writer::{apply_fact_updates, FactUpdates},
    CrawlFile,
};

pub fn run(
    file: &mut CrawlFile,
    old: &CrawlConfig,
    new: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<Vec<UrlId>, EngineError> {
    cancel.checkpoint().map_err(|_| EngineError::Cancelled)?;
    if old.extraction.hash || !new.extraction.hash {
        return Ok(Vec::new());
    }
    let mut changed = Vec::new();
    let mut after = 0;
    loop {
        cancel.checkpoint().map_err(|_| EngineError::Cancelled)?;
        let ids = {
            let mut stmt = file.conn().prepare("SELECT id FROM urls WHERE id > ?1 AND content_class = 'HTML' AND status_code BETWEEN 200 AND 299 AND hash_md5 IS NULL ORDER BY id LIMIT 128").map_err(os_store::StoreError::from)?;
            let ids = stmt
                .query_map([after], |row| row.get::<_, i64>(0))
                .map_err(os_store::StoreError::from)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(os_store::StoreError::from)?;
            ids
        };
        let Some(last) = ids.last() else {
            break;
        };
        after = *last;
        let mut updates = FactUpdates::default();
        for raw in ids {
            cancel.checkpoint().map_err(|_| EngineError::Cancelled)?;
            let id = UrlId(raw);
            if let Some(html) = os_store::queries::stored_html(file, id)? {
                updates.hash.push((id, Some(md5(&html.bytes))));
            }
        }
        cancel.checkpoint().map_err(|_| EngineError::Cancelled)?;
        changed.extend(apply_fact_updates(file, &updates)?);
    }
    Ok(changed)
}

// RFC-1321 MD5, ausschließlich als Inhaltsfingerabdruck. Kein Manifest-Eingriff erforderlich.
fn md5(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let mut state = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476];
    let blocks = bytes.len() / 64;
    let mut tail = [0u8; 128];
    let remainder = &bytes[blocks * 64..];
    tail[..remainder.len()].copy_from_slice(remainder);
    tail[remainder.len()] = 0x80;
    let tail_len = if remainder.len() < 56 { 64 } else { 128 };
    tail[tail_len - 8..tail_len]
        .copy_from_slice(&(bytes.len() as u64).wrapping_mul(8).to_le_bytes());
    for chunk in bytes[..blocks * 64]
        .as_chunks::<64>()
        .0
        .iter()
        .chain(tail[..tail_len].as_chunks::<64>().0.iter())
    {
        let mut words = [0u32; 16];
        for (word, part) in words.iter_mut().zip(chunk.as_chunks::<4>().0.iter()) {
            *word = u32::from_le_bytes(*part);
        }
        let [mut a, mut b, mut c, mut d] = state;
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let next = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(K[i])
                    .wrapping_add(words[g])
                    .rotate_left(S[i]),
            );
            a = d;
            d = c;
            c = b;
            b = next;
        }
        for (s, v) in state.iter_mut().zip([a, b, c, d]) {
            *s = s.wrapping_add(v);
        }
    }
    state
        .iter()
        .flat_map(|n| n.to_le_bytes())
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::md5;
    // spec: ONPAGE-143 [M2]
    #[test]
    fn byte_fingerprints_match_standard_vectors_and_padding_boundaries() {
        assert_eq!(md5(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            md5(b"abcdefghijklmnopqrstuvwxyz"),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
        assert_eq!(
            md5(&vec![b'a'; 1_000_000]),
            "7707d6ae4e027c70eea2a935c2296f21"
        );
    }
}
