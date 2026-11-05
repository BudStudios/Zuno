//! End-to-end import tests against synthetic browser profiles created in a temp directory.

use std::path::Path;
use std::sync::{Arc, Mutex};

use super::*;
use crate::browser::engine::*;
use crate::browser::tabs::TabId;
use crate::storage::settings::SettingsStore;
use crate::storage::Storage;

#[derive(Default)]
struct MockEngine {
    cookies: Mutex<Vec<ImportedCookie>>,
}

impl RenderingEngine for MockEngine {
    fn create_view(&self, _: TabId, _: &url::Url, _: Bounds, _: bool, _: Arc<dyn EngineSink>) -> Result<Arc<dyn ContentView>> {
        Err(Error::msg("not used"))
    }
    fn control_download(&self, _: i64, _: DownloadAction) -> Result<()> {
        Ok(())
    }
    fn apply_profile(&self, _: ProfilePrefs) -> Result<()> {
        Ok(())
    }
    fn clear_browsing_data(&self, _: ClearKinds) -> Result<()> {
        Ok(())
    }
    fn import_cookies(&self, cookies: Vec<ImportedCookie>) -> Result<usize> {
        let n = cookies.len();
        self.cookies.lock().unwrap().extend(cookies);
        Ok(n)
    }
}

fn make_chrome(local: &Path) {
    let user_data = local.join("Google").join("Chrome").join("User Data");
    let profile = user_data.join("Default");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(user_data.join("Local State"), r#"{"profile":{"info_cache":{"Default":{"name":"Personal"}}}}"#).unwrap();
    std::fs::write(
        profile.join("Bookmarks"),
        r#"{"roots":{"bookmark_bar":{"type":"folder","name":"Bar","children":[
            {"type":"url","name":"Rust","url":"https://www.rust-lang.org/"},
            {"type":"url","name":"Bookmarklet","url":"javascript:void(0)"},
            {"type":"folder","name":"Dev","children":[{"type":"url","name":"Tauri","url":"https://tauri.app/"}]}]},
          "other":{"type":"folder","name":"Other","children":[]}}}"#,
    )
    .unwrap();
    std::fs::write(
        profile.join("Preferences"),
        r#"{"homepage":"https://home.example/","homepage_is_newtabpage":false,"session":{"restore_on_startup":5},
            "default_search_provider_data":{"template_url_data":{"url":"https://www.bing.com/search?q={searchTerms}"}}}"#,
    )
    .unwrap();
    let conn = rusqlite::Connection::open(profile.join("History")).unwrap();
    conn.execute_batch(
        "CREATE TABLE urls(id INTEGER PRIMARY KEY, url TEXT, title TEXT, visit_count INTEGER, last_visit_time INTEGER);
         CREATE TABLE visits(id INTEGER PRIMARY KEY, url INTEGER, visit_time INTEGER);
         INSERT INTO urls VALUES (1,'https://www.rust-lang.org/','Rust',2,13348540800000000);
         INSERT INTO urls VALUES (2,'https://login.example/cb?code=SECRET&state=x','Callback',1,13348540900000000);
         INSERT INTO urls VALUES (3,'chrome://settings/','Settings',1,13348541000000000);
         INSERT INTO visits VALUES (1,1,13348540800000000),(2,1,13348540850000000),(3,2,13348540900000000),(4,3,13348541000000000);",
    )
    .unwrap();
}

fn make_firefox(roaming: &Path) {
    let base = roaming.join("Mozilla").join("Firefox");
    let profile = base.join("Profiles").join("abc.default-release");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(base.join("profiles.ini"), "[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/abc.default-release\n").unwrap();
    let places = rusqlite::Connection::open(profile.join("places.sqlite")).unwrap();
    places
        .execute_batch(
            "CREATE TABLE moz_places(id INTEGER PRIMARY KEY, url TEXT, title TEXT);
             CREATE TABLE moz_historyvisits(id INTEGER PRIMARY KEY, place_id INTEGER, visit_date INTEGER);
             CREATE TABLE moz_bookmarks(id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, guid TEXT);
             INSERT INTO moz_places VALUES (1,'https://developer.mozilla.org/','MDN'),(2,'https://example.org/','Example');
             INSERT INTO moz_historyvisits VALUES (1,1,1704067200000000),(2,2,1704067300000000);
             INSERT INTO moz_bookmarks VALUES (1,2,NULL,0,0,'','root________');
             INSERT INTO moz_bookmarks VALUES (2,2,NULL,1,0,'toolbar','toolbar_____');
             INSERT INTO moz_bookmarks VALUES (3,1,1,2,0,'MDN','aaaaaaaaaaaa');
             INSERT INTO moz_bookmarks VALUES (4,2,NULL,2,1,'Folder','bbbbbbbbbbbb');
             INSERT INTO moz_bookmarks VALUES (5,1,2,4,0,'Example','cccccccccccc');",
        )
        .unwrap();
    let cookies = rusqlite::Connection::open(profile.join("cookies.sqlite")).unwrap();
    cookies
        .execute_batch(
            "CREATE TABLE moz_cookies(id INTEGER PRIMARY KEY, name TEXT, value TEXT, host TEXT, path TEXT, expiry INTEGER, isSecure INTEGER, isHttpOnly INTEGER, sameSite INTEGER);
             INSERT INTO moz_cookies VALUES (1,'sid','v1','.example.org','/',4102444800,1,1,1);
             INSERT INTO moz_cookies VALUES (2,'old','v2','.example.org','/',946684800,0,0,0);",
        )
        .unwrap();
    std::fs::write(profile.join("prefs.js"), "user_pref(\"browser.startup.page\", 3);\n").unwrap();
}

#[test]
fn imports_from_synthetic_chrome_and_firefox_profiles() {
    let tmp = tempfile::tempdir().unwrap();
    let local = tmp.path().join("Local");
    let roaming = tmp.path().join("Roaming");
    make_chrome(&local);
    make_firefox(&roaming);
    // SAFETY: only this test reads these variables (importer detection).
    unsafe {
        std::env::set_var("LOCALAPPDATA", &local);
        std::env::set_var("APPDATA", &roaming);
    }

    let sources = detect();
    let ids: Vec<&str> = sources.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["chrome", "firefox"]);
    assert_eq!(sources[0].profiles[0].name, "Personal");

    let storage = Storage::open_in_memory().unwrap();
    let settings_dir = tempfile::tempdir().unwrap();
    let settings = SettingsStore::load(settings_dir.path());
    let engine = MockEngine::default();
    let progress = |_: Progress| {};

    // Inspection reports availability and why cookies/passwords are not importable from Chrome.
    let (_, chrome) = open("chrome", "Default").unwrap();
    let inspection = inspect(chrome.as_ref(), &settings.get());
    let by = |c: Category| inspection.categories.iter().find(|i| i.category == c).unwrap().clone();
    assert_eq!(by(Category::Bookmarks).count, Some(3));
    assert_eq!(by(Category::History).count, Some(4));
    assert!(!by(Category::Cookies).available && by(Category::Cookies).reason.unwrap().contains("encrypts"));
    assert!(!by(Category::Passwords).available);
    assert_eq!(inspection.setting_changes.len(), 3);

    // Settings are not replaced without confirmation.
    let request = ImportRequest {
        source: "chrome".into(),
        profile: "Default".into(),
        categories: vec![Category::Bookmarks, Category::History, Category::Cookies, Category::Settings],
        confirm_settings_overwrite: false,
    };
    let results = run(&request, &storage, &settings, &engine, &progress).unwrap();
    let status = |c: Category| results.iter().find(|r| r.category == c).unwrap().clone();
    assert_eq!(status(Category::Bookmarks).status, Status::Imported);
    assert_eq!(status(Category::Bookmarks).count, 2, "javascript: bookmarklet skipped");
    assert_eq!(status(Category::History).count, 3, "chrome:// page skipped");
    assert_eq!(status(Category::Cookies).status, Status::Unavailable);
    assert_eq!(status(Category::Settings).status, Status::Skipped);
    assert_eq!(settings.get().search_engine, "duckduckgo");

    // History was sanitised: the authorization code never reaches Zuno's database.
    let history = storage.history_search("", None, 50).unwrap();
    assert!(history.iter().all(|h| !h.url.contains("SECRET")));
    assert!(history.iter().any(|h| h.url == "https://login.example/cb?state=x"));
    // Bookmarks land in their own folder; the structure is preserved.
    let all = storage.bookmarks_all().unwrap();
    let folder = all.iter().find(|b| b.title == "Imported from Google Chrome").unwrap();
    assert_eq!(folder.parent_id, Some(crate::storage::bookmarks::OTHER_BOOKMARKS));
    assert!(all.iter().any(|b| b.title == "Dev" && b.is_folder));

    // Re-importing history adds no duplicates.
    let again = ImportRequest { categories: vec![Category::History], ..request.clone() };
    let results = run(&again, &storage, &settings, &engine, &progress).unwrap();
    assert_eq!(results[0].count, 0);

    // Confirmed settings import applies the imported values.
    let confirmed = ImportRequest { categories: vec![Category::Settings], confirm_settings_overwrite: true, ..request };
    run(&confirmed, &storage, &settings, &engine, &progress).unwrap();
    let s = settings.get();
    assert_eq!((s.search_engine.as_str(), s.homepage.as_str()), ("bing", "https://home.example/"));

    // Firefox: bookmarks, history and unexpired cookies.
    let profile_id = sources[1].profiles[0].id.clone();
    let ff = ImportRequest {
        source: "firefox".into(),
        profile: profile_id,
        categories: vec![Category::Bookmarks, Category::History, Category::Cookies, Category::Passwords],
        confirm_settings_overwrite: false,
    };
    let results = run(&ff, &storage, &settings, &engine, &progress).unwrap();
    let status = |c: Category| results.iter().find(|r| r.category == c).unwrap().clone();
    assert_eq!(status(Category::Bookmarks).count, 2);
    assert_eq!(status(Category::History).count, 2);
    assert_eq!(status(Category::Cookies).count, 1, "expired cookie skipped");
    assert_eq!(status(Category::Passwords).status, Status::Unavailable);
    let cookie = engine.cookies.lock().unwrap()[0].clone();
    assert!(cookie.secure && cookie.http_only && cookie.domain == ".example.org");
}
