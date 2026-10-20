mod common;
use common::Env;
use std::{fs, io::Read};
// spec: OUT-244
// spec: OUT-227 [M2]
// spec: OUT-243
#[test]
fn standalone_diagnostics_respects_portable_home_and_omits_crawl_files() {
    let env = Env::new();
    let mut config = os_model::CrawlConfig::default();
    config.limits.max_urls = 17;
    fs::write(
        env.home.path().join("default.osconfig"),
        config.to_toml_string().unwrap(),
    )
    .unwrap();
    let logs = env.home.path().join("logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(
        logs.join("openspider.log"),
        "password=PASSWORD-FIXTURE\nCookie: COOKIE-FIXTURE",
    )
    .unwrap();
    fs::create_dir_all(env.crawls_dir()).unwrap();
    fs::write(env.crawls_dir().join("fixture.ospider"), "CRAWL-FIXTURE").unwrap();
    let output = env.out();
    let run = env.run(&["--diagnostics", output.to_str().unwrap()]);
    assert_eq!(run.code, 0, "{}", run.dump());
    let path = std::path::Path::new(run.stdout.trim());
    assert!(path.starts_with(output));
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        assert!(!entry.name().unwrap().ends_with(".ospider"));
        let mut raw = String::new();
        entry.read_to_string(&mut raw).unwrap();
        assert!(!raw.contains("FIXTURE"), "{raw}");
    }
    let mut diff = String::new();
    zip.by_name("config-diff.json")
        .unwrap()
        .read_to_string(&mut diff)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&diff).unwrap()["crawl"]["limits"]["max_urls"],
        17
    );
    let mut system = String::new();
    zip.by_name("system.txt")
        .unwrap()
        .read_to_string(&mut system)
        .unwrap();
    // system.txt ist JSON: Windows-Pfade stehen dort mit verdoppelten Backslashes.
    let home = serde_json::to_string(env.home.path().to_str().unwrap()).unwrap();
    assert!(system.contains(home.trim_matches('"')), "{system}");
}
