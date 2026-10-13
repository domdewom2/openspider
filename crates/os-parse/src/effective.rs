//! Wirksame Indexierung/Folgen aus HTML- und Header-Direktiven (TECH-066).
//!
//! Nur Direktiven mit `applies = true` zählen (generisch oder zum Agent-Token passend). Die restriktivste Angabe
//! gewinnt, egal ob sie in einem Tag, in mehreren Tags oder in Meta und Header steht: `noindex` schlägt `index`
//! und `all`, `nofollow` schlägt `follow`, `none` bedeutet `noindex` und `nofollow`. Auch Angaben im Body
//! (`in_head = false`) wirken (TECH-065, vorsichtige Annahme).

use os_model::Directive;

/// Berechnet `(effective_index, effective_follow)`; die restriktivste Angabe gewinnt.
pub fn compute(html: &[Directive], header: &[Directive]) -> (bool, bool) {
    let mut index = true;
    let mut follow = true;
    for directive in html.iter().chain(header).filter(|d| d.applies) {
        for token in &directive.tokens {
            match token.as_str() {
                "noindex" => index = false,
                "nofollow" => follow = false,
                "none" => {
                    index = false;
                    follow = false;
                }
                _ => {}
            }
        }
    }
    (index, follow)
}
