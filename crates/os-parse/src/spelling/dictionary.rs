//! Wörterbücher der Rechtschreibprüfung (ONPAGE-153, ONPAGE-157; Klärung K2-03).
//!
//! **Engine:** `spellbook` (MPL-2.0), Hunspell-kompatibel. **Gebündelt** sind nur Wörterbücher mit freizügiger Lizenz:
//! die SCOWL-Wörterbücher `en_US`, `en_GB` (Schreibung „-ise“), `en_CA`, `en_AU` (Lizenz
//! `resources/dictionaries/LICENSE-SCOWL.txt`); `en-NZ` und `en-ZA` nutzen das britische Wörterbuch. Die `.dic`-Dateien
//! liegen gzip-komprimiert im Repository und werden beim ersten Gebrauch entpackt. Alle übrigen Sprachen lädt der
//! Benutzer selbst (Hunspell-Dateien, nicht gebündelt) in ein Wörterbuchverzeichnis.
//!
//! **Suchreihenfolge** für einen Code wie `de-AT`: in jedem Verzeichnis aus [`Resources::dictionary_dirs`]
//! `de_AT.dic`/`de-AT.dic`, danach die gebündelten Wörterbücher, danach in den Verzeichnissen die Basissprache
//! `de.dic`. Fehlt die `.aff`-Datei neben einer `.dic`-Datei, gilt eine leere Affixdatei (reine Wortliste, UTF-8).
//! Geladene Wörterbücher bleiben für den Prozess im Speicher.
//!
//! **Persönliches Wörterbuch** (ONPAGE-157): eine Textdatei je Sprache (`<Code>.txt`, ein Wort je Zeile) in
//! [`Resources::personal_dir`]; die App setzt den Ordner beim Start auf `<Einstellungen>/dictionaries/personal` (über
//! `dirs` bzw. `OPENSPIDER_HOME`, [`under`]). Eine Prüfung in `de-AT` liest `de-AT.txt` und `de.txt`. Vergleich ohne
//! Beachtung der Groß-/Kleinschreibung.
//!
//! **Orte ohne App** (CLI, Tests): ohne [`configure`] gelten die Verzeichnisse aus `OPENSPIDER_DICTIONARIES` und, falls
//! `OPENSPIDER_HOME` gesetzt ist, `<OPENSPIDER_HOME>/dictionaries` samt persönlichem Wörterbuch ([`resources`]).

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::SystemTime;

/// Ein geladenes Wörterbuch.
pub type Dictionary = spellbook::Dictionary;

/// Orte der Wörterbücher (prozessweit, [`configure`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resources {
    /// Verzeichnisse mit Hunspell-Wörterbüchern (`<Code>.aff`/`<Code>.dic`), vor den gebündelten durchsucht.
    pub dictionary_dirs: Vec<PathBuf>,
    /// Ordner des persönlichen Wörterbuchs (`None` = keins).
    pub personal_dir: Option<PathBuf>,
}

/// Umgebungsvariable mit zusätzlichen Wörterbuchverzeichnissen (Pfadliste wie `PATH`).
pub const DICTIONARIES_ENV: &str = "OPENSPIDER_DICTIONARIES";

/// Umgebungsvariable des Heimatordners (`os_store::paths::HOME_ENV`).
const HOME_ENV: &str = "OPENSPIDER_HOME";

fn global() -> &'static RwLock<Option<Resources>> {
    static GLOBAL: OnceLock<RwLock<Option<Resources>>> = OnceLock::new();
    GLOBAL.get_or_init(|| RwLock::new(None))
}

/// Setzt die Orte der Wörterbücher für alle folgenden Prüfungen des Prozesses (die App beim Start).
pub fn configure(resources: Resources) {
    *global().write().unwrap_or_else(|e| e.into_inner()) = Some(resources);
}

/// Verzeichnisse aus [`DICTIONARIES_ENV`].
pub fn env_dirs() -> Vec<PathBuf> {
    std::env::var_os(DICTIONARIES_ENV)
        .map(|v| {
            std::env::split_paths(&v)
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Orte unterhalb eines Einstellungsordners: `<Ordner>/dictionaries` und `<Ordner>/dictionaries/personal`, davor die
/// Verzeichnisse aus [`DICTIONARIES_ENV`].
pub fn under(settings: &Path) -> Resources {
    let root = settings.join("dictionaries");
    let mut dirs = env_dirs();
    dirs.push(root.clone());
    Resources {
        dictionary_dirs: dirs,
        personal_dir: Some(root.join("personal")),
    }
}

/// Die aktuell gesetzten Orte; ohne [`configure`] aus der Umgebung: [`DICTIONARIES_ENV`] und, falls gesetzt,
/// `OPENSPIDER_HOME` (wie [`under`]).
pub fn resources() -> Resources {
    if let Some(set) = global().read().unwrap_or_else(|e| e.into_inner()).clone() {
        return set;
    }
    match std::env::var_os(HOME_ENV).filter(|h| !h.is_empty()) {
        Some(home) => under(Path::new(&home)),
        None => Resources {
            dictionary_dirs: env_dirs(),
            personal_dir: None,
        },
    }
}

/// Gebündelte Wörterbücher: Code, Affixdatei, gzip-komprimierte Wortliste.
const BUNDLED: &[(&str, &str, &[u8])] = &[
    (
        "en-US",
        include_str!("../../resources/dictionaries/en_US.aff"),
        include_bytes!("../../resources/dictionaries/en_US.dic.gz"),
    ),
    (
        "en-GB",
        include_str!("../../resources/dictionaries/en_GB.aff"),
        include_bytes!("../../resources/dictionaries/en_GB.dic.gz"),
    ),
    (
        "en-CA",
        include_str!("../../resources/dictionaries/en_CA.aff"),
        include_bytes!("../../resources/dictionaries/en_CA.dic.gz"),
    ),
    (
        "en-AU",
        include_str!("../../resources/dictionaries/en_AU.aff"),
        include_bytes!("../../resources/dictionaries/en_AU.dic.gz"),
    ),
];

/// Varianten ohne eigenes gebündeltes Wörterbuch und ihr Ersatz.
const BUNDLED_ALIASES: &[(&str, &str)] = &[("en-NZ", "en-GB"), ("en-ZA", "en-GB")];

/// Codes mit gebündeltem Wörterbuch (einschließlich der Aliase).
pub fn bundled_codes() -> Vec<&'static str> {
    BUNDLED
        .iter()
        .map(|(c, _, _)| *c)
        .chain(BUNDLED_ALIASES.iter().map(|(c, _)| *c))
        .collect()
}

/// Herkunft eines Wörterbuchs (Schlüssel des Zwischenspeichers).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Source {
    Bundled(&'static str),
    File(PathBuf),
}

fn cache() -> &'static Mutex<HashMap<Source, Option<Arc<Dictionary>>>> {
    static CACHE: OnceLock<Mutex<HashMap<Source, Option<Arc<Dictionary>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Minimale Affixdatei für reine Wortlisten.
const EMPTY_AFF: &str = "SET UTF-8\n";

fn load(source: &Source) -> Option<Arc<Dictionary>> {
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = cache.get(source) {
        return found.clone();
    }
    let loaded = match source {
        Source::Bundled(code) => {
            BUNDLED
                .iter()
                .find(|(c, _, _)| c == code)
                .and_then(|(_, aff, dic)| {
                    let mut text = String::new();
                    flate2::read::GzDecoder::new(*dic)
                        .read_to_string(&mut text)
                        .ok()?;
                    Dictionary::new(aff, &text).ok()
                })
        }
        Source::File(dic_path) => {
            let dic = std::fs::read_to_string(dic_path).ok()?;
            let aff = std::fs::read_to_string(dic_path.with_extension("aff"))
                .unwrap_or_else(|_| EMPTY_AFF.to_string());
            Dictionary::new(&aff, &dic).ok()
        }
    }
    .map(Arc::new);
    cache.insert(source.clone(), loaded.clone());
    loaded
}

fn file_in(dirs: &[PathBuf], names: &[String]) -> Option<PathBuf> {
    dirs.iter()
        .flat_map(|dir| names.iter().map(move |n| dir.join(format!("{n}.dic"))))
        .find(|p| p.is_file())
}

/// Wörterbuch für einen kanonischen Code (`de-AT`, `fr`), `None` ohne passendes Wörterbuch.
pub fn find(code: &str, resources: &Resources) -> Option<Arc<Dictionary>> {
    let dirs = &resources.dictionary_dirs;
    let exact = [code.replace('-', "_"), code.to_string()];
    if let Some(path) = file_in(dirs, &exact) {
        return load(&Source::File(path));
    }
    let bundled = BUNDLED_ALIASES
        .iter()
        .find(|(alias, _)| *alias == code)
        .map_or(code, |(_, target)| *target);
    if let Some((c, _, _)) = BUNDLED.iter().find(|(c, _, _)| *c == bundled) {
        return load(&Source::Bundled(c));
    }
    let base = super::lang::base_of(code);
    if base != code {
        if let Some(path) = file_in(dirs, &[base.to_string()]) {
            return load(&Source::File(path));
        }
    }
    None
}

// --- Persönliches Wörterbuch -----------------------------------------------------------------------------------

/// Dateiname des persönlichen Wörterbuchs einer Sprache.
pub fn personal_file(dir: &Path, code: &str) -> PathBuf {
    dir.join(format!("{code}.txt"))
}

type PersonalCache = HashMap<PathBuf, (Option<SystemTime>, u64, Arc<HashSet<String>>)>;

fn personal_cache() -> &'static Mutex<PersonalCache> {
    static CACHE: OnceLock<Mutex<PersonalCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn read_words(path: &Path) -> Arc<HashSet<String>> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Arc::new(HashSet::new());
    };
    let stamp = (meta.modified().ok(), meta.len());
    let mut cache = personal_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((m, l, words)) = cache.get(path) {
        if (*m, *l) == stamp {
            return words.clone();
        }
    }
    let words: HashSet<String> = std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let words = Arc::new(words);
    cache.insert(path.to_path_buf(), (stamp.0, stamp.1, words.clone()));
    words
}

/// Wörter des persönlichen Wörterbuchs für `code` (Code und Basissprache), klein geschrieben.
pub fn personal_words(code: &str, resources: &Resources) -> HashSet<String> {
    let Some(dir) = &resources.personal_dir else {
        return HashSet::new();
    };
    let mut out: HashSet<String> = read_words(&personal_file(dir, code))
        .iter()
        .cloned()
        .collect();
    let base = super::lang::base_of(code);
    if base != code {
        out.extend(read_words(&personal_file(dir, base)).iter().cloned());
    }
    out
}

/// „Zum Wörterbuch hinzufügen“ (ONPAGE-157): hängt `word` an das persönliche Wörterbuch der Sprache `code` an.
/// Ohne gesetzten Ordner ein Fehler `NotFound`.
pub fn add_personal_word(code: &str, word: &str, resources: &Resources) -> std::io::Result<()> {
    use std::io::Write;
    let dir = resources.personal_dir.as_ref().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no personal dictionary folder",
        )
    })?;
    let word = word.trim();
    if word.is_empty() || word.contains(['\n', '\r']) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid word",
        ));
    }
    if personal_words(code, resources).contains(&word.to_lowercase()) {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    let path = personal_file(dir, code);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{word}")?;
    file.sync_all()?;
    drop(file);
    personal_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&path);
    Ok(())
}
