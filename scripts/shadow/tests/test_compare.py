"""Tests für scripts/shadow/compare.py (nur Standardbibliothek): python3 -m unittest discover scripts/shadow/tests"""

import contextlib
import http.server
import io
import json
import os
import sys
import tempfile
import threading
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SHADOW = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(SHADOW))
sys.path.insert(0, SHADOW)

import compare as c  # noqa: E402

DATA = os.path.join(HERE, "data")


def load_json(name):
    with open(os.path.join(DATA, name), encoding="utf-8") as fh:
        return json.load(fh)


class NormalizeTests(unittest.TestCase):
    def test_scheme_host_fragment_port(self):
        self.assertEqual(c.normalize_url("HTTPS://Example.COM:443/Pfad#frag"), "https://example.com/Pfad")
        self.assertEqual(c.normalize_url("http://example.com:80/"), "http://example.com/")
        self.assertEqual(c.normalize_url("http://example.com:8080/x"), "http://example.com:8080/x")

    def test_slash_rule(self):
        self.assertEqual(c.normalize_url("https://example.com"), "https://example.com/")
        self.assertEqual(c.normalize_url("https://example.com/a/"), "https://example.com/a/")
        self.assertNotEqual(c.normalize_url("https://example.com/a"), c.normalize_url("https://example.com/a/"))

    def test_percent_encoding_and_query(self):
        self.assertEqual(c.normalize_url("https://example.com/%7euser/%c3%a4"), "https://example.com/~user/%C3%A4")
        self.assertEqual(c.normalize_url("https://example.com/a?b=1&a=2"), "https://example.com/a?b=1&a=2")
        self.assertEqual(c.normalize_url("https://example.com/a?"), "https://example.com/a")

    def test_userinfo_and_whitespace(self):
        self.assertEqual(c.normalize_url("  https://u:p@Example.com/x "), "https://example.com/x")

    def test_invalid_port_does_not_raise(self):
        self.assertEqual(c.normalize_url("https://example.com:abc/"), "https://example.com:abc/")


class NamesTests(unittest.TestCase):
    def test_file_names_follow_the_export_rules(self):
        self.assertEqual(c.export_file_name("Response Codes:Client Error (4xx)"), "response_codes_client_error_4xx.csv")
        self.assertEqual(
            c.export_file_name("Response Codes:Internal Blocked by Robots.txt"),
            "response_codes_internal_blocked_by_robots.txt.csv",
        )
        self.assertEqual(c.export_file_name("Internal:All"), "internal_all.csv")

    def test_placeholders_are_resolved_with_the_default_limits(self):
        expected = {
            "URL:Over {url.max_chars} Characters": "url_over_115_characters.csv",
            "Page Titles:Over {title.max_chars} Characters": "page_titles_over_60_characters.csv",
            "Page Titles:Below {title.min_chars} Characters": "page_titles_below_30_characters.csv",
            "Meta Description:Over {meta_description.max_chars} Characters": "meta_description_over_155_characters.csv",
            "Meta Description:Below {meta_description.min_chars} Characters": "meta_description_below_70_characters.csv",
            "H1:Over {h1.max_chars} Characters": "h1_over_70_characters.csv",
            "H2:Over {h2.max_chars} Characters": "h2_over_70_characters.csv",
            "Images:Over {image.max_kb} KB": "images_over_100_kb.csv",
            "Images:Alt Text Over {image.alt_max_chars} Characters": "images_alt_text_over_100_characters.csv",
            "Page Titles:Over {title.max_px} Pixels": "page_titles_over_561_pixels.csv",
        }
        for name, file in expected.items():
            self.assertEqual(c.export_file_name(name), file, name)

    def test_placeholders_use_the_values_of_the_osconfig(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "x.osconfig")
            with open(path, "w", encoding="utf-8") as fh:
                fh.write("schema_version = 2\n[title]\nmax_chars = 55\n[image]\nmax_kb = 250\n")
            limits = c.load_limits(path)
        self.assertEqual(c.export_file_name("Page Titles:Over {title.max_chars} Characters", limits=limits),
                         "page_titles_over_55_characters.csv")
        self.assertEqual(c.export_file_name("Images:Over {image.max_kb} KB", limits=limits), "images_over_250_kb.csv")
        self.assertEqual(c.export_file_name("Page Titles:Below {title.min_chars} Characters", limits=limits),
                         "page_titles_below_30_characters.csv")

    def test_shipped_osconfig_keeps_the_default_thresholds(self):
        # shadow.osconfig setzt keine eigenen Schwellen; sonst müssten die Dateinamen daraus berechnet werden.
        self.assertEqual(c.load_limits(os.path.join(SHADOW, "shadow.osconfig")), c.DEFAULT_LIMITS)

    def test_every_export_name_matches_the_files_of_a_real_export(self):
        # real-export-names.txt: CSV-Dateinamen eines echten Exports (Release-Build 2f38c4a, Loopback-Crawl) mit
        # genau der Namensliste aus rule-map.toml; jede Kennung muss auf eine dieser Dateien abgebildet werden.
        with open(os.path.join(DATA, "real-export-names.txt"), encoding="utf-8") as fh:
            recorded = {line.strip() for line in fh if line.strip()}
        names = c.export_filters(c.load_rule_map(os.path.join(SHADOW, "rule-map.toml")))
        self.assertEqual(len(recorded), len(names))
        computed = {c.export_file_name(n) for n in names}
        self.assertEqual(computed, recorded)

    def test_pattern_search_finds_a_single_export_when_the_thresholds_differ(self):
        with tempfile.TemporaryDirectory() as tmp:
            with open(os.path.join(tmp, "page_titles_over_72_characters.csv"), "w", encoding="utf-8") as fh:
                fh.write("Address\nhttps://e.test/\n")
            found = c.find_export_file(tmp, "Page Titles:Over {title.max_chars} Characters")
            self.assertEqual(os.path.basename(found), "page_titles_over_72_characters.csv")
            # zweideutig: kein Treffer statt Raten
            with open(os.path.join(tmp, "page_titles_over_80_characters.csv"), "w", encoding="utf-8") as fh:
                fh.write("Address\n")
            self.assertIsNone(c.find_export_file(tmp, "Page Titles:Over {title.max_chars} Characters"))
            self.assertIsNone(c.find_export_file(tmp, "Page Titles:Missing"))

    def test_overlong_names_are_rejected(self):
        with self.assertRaises(ValueError):
            c.export_file_name("Tab:" + "x" * 130)


class RuleMapTests(unittest.TestCase):
    def test_sample_map_is_valid(self):
        self.assertEqual(c.validate_rule_map(c.load_rule_map(os.path.join(DATA, "map.toml"))), [])

    def test_problems_are_reported(self):
        bad = {
            "schema": 1,
            "rule": [
                {"autonomo": "r1", "openspider": ["Internal:All"], "class": "ohne Gegenstück", "reason": "x"},
                {"autonomo": "r1", "openspider": [], "class": "gleichwertig", "reason": "x"},
                {"autonomo": "r2", "openspider": ["KeinDoppelpunkt"], "class": "teilweise", "reason": ""},
                {"autonomo": "r3", "openspider": ["Internal:All"], "class": "nix", "reason": "y"},
            ],
        }
        text = "\n".join(c.validate_rule_map(bad))
        for expected in ("ohne Gegenstück darf keine", "doppelt", "braucht mindestens", "reason ist identisch",
                         "keine Tab:Filter-Kennung", "reason fehlt", "class muss"):
            self.assertIn(expected, text)

    def test_unknown_filter_ids_are_reported(self):
        data = {"schema": 1, "export": {"base": ["Internal:Nope"]}, "rule": []}
        problems = c.validate_rule_map(data, {"internal:all"})
        self.assertTrue(any("Internal:Nope" in p for p in problems))

    def test_shipped_rule_map_matches_the_filter_files(self):
        known = c.known_filter_ids(
            os.path.join(REPO, "docs/plan/M1-filters.toml"), os.path.join(REPO, "docs/plan/M2-filters.toml")
        )
        data = c.load_rule_map(os.path.join(SHADOW, "rule-map.toml"))
        self.assertEqual(c.validate_rule_map(data, known), [])
        self.assertIn("Internal:All", c.export_filters(data))

    def test_export_list_is_deduplicated_and_includes_mapped_filters(self):
        data = c.load_rule_map(os.path.join(DATA, "map.toml"))
        names = c.export_filters(data)
        self.assertEqual(names[0], "Internal:All")
        self.assertEqual(len(names), len({n.lower() for n in names}))
        self.assertIn("Security:HTTP URLs", names)

    def test_exports_command_escapes_commas(self):
        buf = io.StringIO()
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "m.toml")
            with open(path, "w", encoding="utf-8") as fh:
                fh.write('schema = 1\n[export]\nbase = ["Tab:A,B", "Tab:C"]\n')
            with contextlib.redirect_stdout(buf):
                self.assertEqual(c.main(["exports", "--rule-map", path]), 0)
        self.assertEqual(buf.getvalue().strip(), "Tab:A\\,B,Tab:C")


class CompareTests(unittest.TestCase):
    def setUp(self):
        self.rule_map = c.load_rule_map(os.path.join(DATA, "map.toml"))
        self.os_data = c.load_os_exports(os.path.join(DATA, "os"), c.export_filters(self.rule_map))
        self.result = c.compare_site("example.com", self.os_data, load_json("autonomo.json"), self.rule_map, "2026-10-17T02:05:00Z")
        self.rules = {r["autonomo"]: r for r in self.result["rules"]}

    def test_counts_intersection_and_jaccard(self):
        r = self.rules["demo_status_4xx"]
        self.assertEqual((r["count_autonomo"], r["count_openspider"], r["intersection"]), (4, 7, 2))
        self.assertEqual((r["only_autonomo"], r["only_openspider"]), (2, 5))
        self.assertAlmostEqual(r["jaccard"], 2 / 9, places=4)

    def test_examples_are_limited_sorted_and_per_direction(self):
        r = self.rules["demo_status_4xx"]
        self.assertEqual(r["examples_only_autonomo"], ["https://example.com/old1", "https://example.com/old2"])
        self.assertEqual(len(r["examples_only_openspider"]), 5)
        self.assertEqual(r["examples_only_openspider"], sorted(r["examples_only_openspider"]))

    def test_normalization_applies_to_both_sides(self):
        # Fragment, Standardport und Host-Schreibweise unterscheiden die Seiten nicht.
        r = self.rules["demo_title_fehlt"]
        self.assertEqual((r["intersection"], r["only_autonomo"], r["only_openspider"], r["jaccard"]), (2, 0, 0, 1.0))

    def test_class_without_counterpart_is_not_compared(self):
        r = self.rules["demo_nur_autonomo"]
        self.assertFalse(r["comparable"])
        self.assertEqual(r["count_autonomo"], 1)
        self.assertNotIn("jaccard", r)

    def test_missing_export_is_flagged(self):
        r = self.rules["demo_export_fehlt"]
        self.assertFalse(r["comparable"])
        self.assertEqual(r["missing_exports"], ["Security:HTTP URLs"])

    def test_unmapped_sides_are_listed(self):
        self.assertEqual(self.result["autonomo_unmapped"], {"demo_unbekannt": 2})
        self.assertEqual(self.result["openspider_unmapped"], {"Internal:All": 3})

    def test_placeholder_filters_are_found_and_compared(self):
        rule_map = {"schema": 1, "rule": [{"autonomo": "t60", "class": "gleichwertig", "reason": "r",
                                           "openspider": ["Page Titles:Over {title.max_chars} Characters"]}]}
        with tempfile.TemporaryDirectory() as tmp:
            with open(os.path.join(tmp, "page_titles_over_60_characters.csv"), "w", encoding="utf-8-sig") as fh:
                fh.write("Address,Title 1\nhttps://e.test/a,x\nhttps://e.test/b,y\n")
            data = c.load_os_exports(tmp, c.export_filters(rule_map))
        result = c.compare_site("e.test", data, {"findings": {"t60": ["https://e.test/a", "https://e.test/c"]}}, rule_map, None)
        r = result["rules"][0]
        self.assertTrue(r["comparable"])
        self.assertEqual((r["count_openspider"], r["intersection"]), (2, 1))

    def test_export_without_address_column_is_a_problem_not_an_empty_set(self):
        rule_map = {"schema": 1, "rule": [{"autonomo": "r", "class": "gleichwertig", "reason": "r",
                                           "openspider": ["Page Titles:Missing"]}]}
        with tempfile.TemporaryDirectory() as tmp:
            with open(os.path.join(tmp, "page_titles_missing.csv"), "w", encoding="utf-8") as fh:
                fh.write("Adresse,Titel\nhttps://e.test/a,x\n")
            data = c.load_os_exports(tmp, c.export_filters(rule_map))
        self.assertIsNone(data["Page Titles:Missing"])
        self.assertIn("Page Titles:Missing", data.unreadable)
        result = c.compare_site("e.test", data, {"findings": {"r": ["https://e.test/a"]}}, rule_map, None)
        r = result["rules"][0]
        self.assertFalse(r["comparable"])
        self.assertEqual(r["unreadable_exports"], ["Page Titles:Missing"])
        self.assertEqual(r["missing_exports"], [])
        self.assertIn("Page Titles:Missing", result["unreadable_exports"])
        self.assertNotIn("Page Titles:Missing", result["openspider_unmapped"])
        md = c.render_markdown(result)
        self.assertIn("Export unlesbar: Page Titles:Missing", md)
        self.assertIn("Unlesbare Exporte", md)

    def test_time_gap_between_the_crawls(self):
        self.assertEqual(self.result["time_gap_s"], 300)

    def test_empty_on_both_sides_counts_as_identical(self):
        stats = c._stats(set(), set())
        self.assertEqual(stats["jaccard"], 1.0)

    def test_markdown_lists_deviations_with_examples(self):
        md = c.render_markdown(self.result)
        self.assertIn("| demo_status_4xx | gleichwertig |", md)
        self.assertIn("### demo_status_4xx", md)
        self.assertIn("https://example.com/old1", md)
        self.assertIn("Autonomo-Regeln ohne Zuordnung", md)
        self.assertNotIn("### demo_title_fehlt", md)

    def test_cli_compare_writes_json_and_markdown(self):
        with tempfile.TemporaryDirectory() as tmp, contextlib.redirect_stdout(io.StringIO()):
            rc = c.main([
                "compare", "--site", "example.com", "--exports", os.path.join(DATA, "os"),
                "--autonomo", os.path.join(DATA, "autonomo.json"), "--rule-map", os.path.join(DATA, "map.toml"),
                "--run-meta", os.path.join(DATA, "run.json"), "--progress", os.path.join(DATA, "progress.log"), "--out", tmp,
            ])
            self.assertEqual(rc, 0)
            with open(os.path.join(tmp, "example.com.json"), encoding="utf-8") as fh:
                out = json.load(fh)
            self.assertEqual(out["metrics"]["peak_rss_kb"], 123456)
            self.assertTrue(os.path.isfile(os.path.join(tmp, "example.com.md")))


class MetricsTests(unittest.TestCase):
    def test_rates_and_peak(self):
        with open(os.path.join(DATA, "progress.log"), encoding="utf-8") as fh:
            m = c.compute_metrics(fh.read().splitlines(), load_json("run.json"))
        self.assertEqual(m["urls_crawled"], 60)
        self.assertEqual(m["mean_rate"], 2.0)
        self.assertEqual(m["max_rate_reported"], 2.0)
        self.assertEqual(m["max_rate_window"], 3.0)
        self.assertEqual(m["peak_rss_kb"], 123456)

    def test_short_runs_report_the_bounds_of_the_mean_rate(self):
        lines = ['{"phase":"crawl","crawled":4,"elapsed_s":1,"rate":2.0}']
        m = c.compute_metrics(lines, {})
        self.assertEqual((m["mean_rate"], m["mean_rate_low"]), (4.0, 2.0))
        self.assertTrue(m["mean_rate_coarse"])
        with open(os.path.join(DATA, "progress.log"), encoding="utf-8") as fh:
            long_run = c.compute_metrics(fh.read().splitlines(), {})
        self.assertLess(long_run["mean_rate_low"], long_run["mean_rate"])
        self.assertTrue(long_run["mean_rate_coarse"])  # 30 s < 60 s

    def test_no_progress_lines(self):
        m = c.compute_metrics(["kein JSON"], {})
        self.assertEqual((m["urls_crawled"], m["mean_rate"]), (0, 0.0))


class AutonomoReadTests(unittest.TestCase):
    def test_extract_findings_row_list_and_mapping(self):
        rows = [{"regel": "r1", "url": "https://e.test/a"}, {"rule_id": "r1", "adresse": "https://e.test/b"}, {"regel": "r2"}]
        self.assertEqual(c.extract_findings(rows, c.FIELD_DEFAULTS["rule"], c.FIELD_DEFAULTS["url"]),
                         {"r1": ["https://e.test/a", "https://e.test/b"]})
        mapping = {"befunde": {"r1": ["https://e.test/a", {"url": "https://e.test/b"}]}}
        self.assertEqual(c.extract_findings(mapping, c.FIELD_DEFAULTS["rule"], c.FIELD_DEFAULTS["url"]),
                         {"r1": ["https://e.test/a", "https://e.test/b"]})

    def test_unknown_shape_reports_structure_without_values(self):
        with self.assertRaises(ValueError) as ctx:
            c.extract_findings({"summe": 5, "name": "geheim"}, ["regel"], ["url"])
        self.assertNotIn("geheim", str(ctx.exception))
        self.assertIn("summe", str(ctx.exception))

    def test_nearest_finished_crawl_is_chosen(self):
        crawls = [
            {"id": "a", "finished_at": "2026-10-10T02:00:00Z"},
            {"id": "b", "finished_at": "2026-10-17T03:00:00Z"},
            {"id": "c", "finished_at": "2026-10-24T02:00:00Z"},
            {"id": "d", "finished_at": None},
        ]
        self.assertEqual(c.pick_nearest(crawls, "2026-10-17T02:05:00Z")["id"], "b")
        self.assertIsNone(c.pick_nearest([], "2026-10-17T02:05:00Z"))

    def test_crawl_candidates_filter_site_and_status(self):
        payload = {"crawls": [
            {"id": 1, "domain": "caloone.de", "status": "fertig", "finished_at": "2026-10-17T02:00:00Z"},
            {"id": 2, "domain": "caloone.de", "status": "laeuft"},
            {"id": 3, "domain": "rotweine.org", "status": "fertig", "finished_at": "2026-10-17T02:00:00Z"},
        ]}
        got = c._crawl_candidates(payload, c.FIELD_DEFAULTS, "caloone.de")
        self.assertEqual([x["id"] for x in got], ["1"])

    def test_select_only_guard(self):
        self.assertEqual(c.check_select_only("SELECT a, b FROM t WHERE x = 1;"), "SELECT a, b FROM t WHERE x = 1")
        for bad in ("DELETE FROM t", "SELECT 1; DROP TABLE t", "UPDATE t SET a = 1", "WITH x AS (DELETE FROM t RETURNING *) SELECT * FROM x",
                    "SELECT 1; SELECT 2", "COPY t TO '/tmp/x'"):
            with self.assertRaises(SystemExit):
                c.check_select_only(bad)

    def test_token_is_read_from_a_file_or_folder_and_never_from_comments(self):
        with tempfile.TemporaryDirectory() as tmp:
            with open(os.path.join(tmp, "a.env"), "w", encoding="utf-8") as fh:
                fh.write("# TOKEN_X=kommentar\nOTHER=1\nexport TOKEN_X='wert-123'\n")
            self.assertEqual(c.read_env_value(tmp, "TOKEN_X"), "wert-123")
            self.assertEqual(c.read_env_value(os.path.join(tmp, "a.env"), "TOKEN_X"), "wert-123")
            self.assertIsNone(c.read_env_value(tmp, "FEHLT"))

    def test_remote_hosts_are_refused(self):
        with self.assertRaises(SystemExit):
            c._api_get("http://example.com/api", "x", "t", 1)


class _Redirecting(http.server.BaseHTTPRequestHandler):
    target = ""
    seen = []

    def do_GET(self):
        _Redirecting.seen.append(self.headers.get("Authorization"))
        self.send_response(302)
        self.send_header("Location", _Redirecting.target)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, *args):
        pass


class _Ignoring(http.server.BaseHTTPRequestHandler):
    """Antwortet auf jede Anfrage mit derselben Seite, ignoriert also limit/offset."""

    hits = 0

    def do_GET(self):
        _Ignoring.hits += 1
        if self.path.endswith("/onpage/crawls"):
            body = [{"id": "7", "domain": "caloone.de", "status": "fertig", "finished_at": "2026-10-17T03:00:00Z"}]
        else:
            body = {"befunde": [{"regel": "r1", "url": f"https://caloone.de/{i}"} for i in range(2)]}
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


def _serve(handler):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


class _Recorder(http.server.BaseHTTPRequestHandler):
    seen = []

    def _answer(self):
        _Recorder.seen.append((self.command, self.path, self.headers.get("Authorization")))
        if self.path.endswith("/onpage/crawls"):
            body = [{"id": "7", "domain": "caloone.de", "status": "fertig", "finished_at": "2026-10-17T03:00:00Z"}]
        else:
            body = {"befunde": [{"regel": "r1", "url": "https://caloone.de/a"}, {"regel": "r1", "url": "https://caloone.de/b"}]}
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    do_GET = _answer
    do_POST = do_PUT = do_PATCH = do_DELETE = _answer

    def log_message(self, *args):
        pass


class ApiFetchTests(unittest.TestCase):
    def test_fetch_uses_only_get_and_never_prints_the_token(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _Recorder)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as tmp:
                env = os.path.join(tmp, "autonomo.env")
                with open(env, "w", encoding="utf-8") as fh:
                    fh.write("AUTONOMO_EXTERN_TOKEN=geheimer-testwert\n")
                cfg_path = os.path.join(tmp, "source.toml")
                with open(cfg_path, "w", encoding="utf-8") as fh:
                    fh.write(f'[api]\nverified = true\nbase_url = "http://127.0.0.1:{server.server_address[1]}/api/extern"\n'
                             f'env_path = "{env}"\n')
                out = os.path.join(tmp, "out", "a.json")
                buf = io.StringIO()
                _Recorder.seen.clear()
                with contextlib.redirect_stdout(buf):
                    rc = c.main(["fetch-autonomo", "--site", "caloone.de", "--near", "2026-10-17T02:05:00Z",
                                 "--source", "api", "--config", cfg_path, "--out", out])
                self.assertEqual(rc, 0)
                self.assertNotIn("geheimer-testwert", buf.getvalue())
                with open(out, encoding="utf-8") as fh:
                    saved = json.load(fh)
                self.assertNotIn("geheimer-testwert", json.dumps(saved))
                self.assertEqual(saved["findings"], {"r1": ["https://caloone.de/a", "https://caloone.de/b"]})
                self.assertEqual(saved["crawl"]["id"], "7")
                self.assertEqual({m for m, _, _ in _Recorder.seen}, {"GET"})
                self.assertTrue(all(auth == "Bearer geheimer-testwert" for _, _, auth in _Recorder.seen))
        finally:
            server.shutdown()
            server.server_close()

    def test_redirects_are_refused_and_the_token_never_follows(self):
        other = _serve(_Recorder)
        first = _serve(_Redirecting)
        try:
            _Recorder.seen.clear()
            _Redirecting.seen.clear()
            _Redirecting.target = f"http://127.0.0.1:{other.server_address[1]}/fremd"
            with self.assertRaises(SystemExit) as ctx:
                c._api_get(f"http://127.0.0.1:{first.server_address[1]}/api", "x", "geheimer-testwert", 5)
            self.assertIn("302", str(ctx.exception))
            self.assertNotIn("geheimer-testwert", str(ctx.exception))
            self.assertEqual(len(_Redirecting.seen), 1)
            self.assertEqual(_Recorder.seen, [])  # das Ziel der Weiterleitung wurde nie angefragt
        finally:
            for srv in (other, first):
                srv.shutdown()
                srv.server_close()

    def test_paging_stops_when_the_api_ignores_limit_and_offset(self):
        server = _serve(_Ignoring)
        try:
            with tempfile.TemporaryDirectory() as tmp:
                env = os.path.join(tmp, "autonomo.env")
                with open(env, "w", encoding="utf-8") as fh:
                    fh.write("AUTONOMO_EXTERN_TOKEN=t\n")
                cfg = {"api": {"verified": True, "base_url": f"http://127.0.0.1:{server.server_address[1]}/api/extern",
                               "env_path": env, "page_size": 2}}
                _Ignoring.hits = 0
                with contextlib.redirect_stderr(io.StringIO()) as err:
                    result = c.fetch_via_api("caloone.de", "2026-10-17T02:05:00Z", cfg)
            self.assertEqual(result["findings"], {"r1": ["https://caloone.de/0", "https://caloone.de/1"]})
            self.assertEqual(_Ignoring.hits, 3)  # Liste, Seite 1, identische Seite 2
            self.assertIn("identische Antwort", err.getvalue())
        finally:
            server.shutdown()
            server.server_close()

    def test_unverified_configuration_stops_before_any_request(self):
        for source in ("api", "psql"):
            with tempfile.TemporaryDirectory() as tmp:
                cfg_path = os.path.join(tmp, "source.toml")
                with open(cfg_path, "w", encoding="utf-8") as fh:
                    fh.write("[api]\nverified = false\n[psql]\nverified = false\n")
                with self.assertRaises(SystemExit):
                    c.main(["fetch-autonomo", "--site", "caloone.de", "--near", "2026-10-17T02:05:00Z",
                            "--source", source, "--config", cfg_path, "--out", os.path.join(tmp, "x.json")])
                self.assertFalse(os.path.exists(os.path.join(tmp, "x.json")))

    def test_shipped_source_config_is_unverified_and_loopback_only(self):
        import tomllib
        with open(os.path.join(SHADOW, "autonomo-source.toml"), "rb") as fh:
            cfg = tomllib.load(fh)
        self.assertFalse(cfg["api"]["verified"])
        self.assertFalse(cfg["psql"]["verified"])
        self.assertTrue(cfg["api"]["base_url"].startswith("http://127.0.0.1:"))


if __name__ == "__main__":
    unittest.main()
