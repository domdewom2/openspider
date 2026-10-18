use super::{invalid, Snapshot};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordUnit {
    Phrase,
    Word,
}

#[derive(Debug, Clone)]
pub struct WordOptions {
    pub unit: WordUnit,
    pub remove_stopwords: bool,
    pub language: String,
    pub max_terms: u32,
    pub include_nofollow: bool,
    pub exclude_self_links: bool,
}
impl Default for WordOptions {
    fn default() -> Self {
        Self {
            unit: WordUnit::Phrase,
            remove_stopwords: true,
            language: "en".into(),
            max_terms: 100,
            include_nofollow: true,
            exclude_self_links: true,
        }
    }
}
impl Snapshot {
    /// Phrase counts preserve the case-sensitive Anchors grouping. Stopwords apply
    /// only to word mode, so the default phrase view agrees with TECH-146.
    pub fn words(
        &self,
        url: i64,
        opts: &WordOptions,
    ) -> Result<Vec<(String, u64)>, os_store::StoreError> {
        if !self.contains_url(url) {
            return Err(invalid("unknown word-cloud URL"));
        }
        if !(1..=10_000).contains(&opts.max_terms) {
            return Err(invalid("maxTerms must be between 1 and 10000"));
        }
        let language = opts.language.split(['-', '_']).next().unwrap_or("en");
        if opts.remove_stopwords && opts.unit == WordUnit::Word && !matches!(language, "de" | "en")
        {
            return Err(invalid("stopword language must be de or en"));
        }
        let mut counts = BTreeMap::<String, u64>::new();
        for group in self.anchors.iter().filter(|g| {
            g.target_id == url
                && (opts.include_nofollow || g.follow)
                && (!opts.exclude_self_links || !g.self_link)
        }) {
            match opts.unit {
                WordUnit::Phrase => *counts.entry(group.text.clone()).or_default() += group.count,
                WordUnit::Word => {
                    if group.text == "(leer)" {
                        continue;
                    }
                    for word in group.text.split(|c: char| !c.is_alphanumeric()) {
                        let word = word.to_lowercase();
                        if word.is_empty()
                            || (opts.remove_stopwords && is_stopword(&word, language))
                        {
                            continue;
                        }
                        *counts.entry(word).or_default() += group.count;
                    }
                }
            }
        }
        let mut terms: Vec<_> = counts.into_iter().collect();
        terms.sort_by(|(a, ac), (b, bc)| bc.cmp(ac).then(a.cmp(b)));
        terms.truncate(opts.max_terms as usize);
        Ok(terms)
    }
}

// Small, explicit language lists; they affect word mode only, never phrase identity.
fn is_stopword(word: &str, language: &str) -> bool {
    let words = if language == "de" {
        "aber alle als am an auch auf aus bei bis das dass dem den der des die ein eine einem einen einer eines es für im in ist mit nach nicht oder sich sie sind und von vor war wie wir zu zum zur"
    } else {
        "a an and are as at be been but by for from has have he her his i if in into is it its me my not of on or our she so that the their them there these they this to us was we were what when which who will with you your"
    };
    words.split_whitespace().any(|w| w == word)
}
