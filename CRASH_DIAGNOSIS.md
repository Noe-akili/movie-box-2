# Android Launch Crash — Static APK + Source Diagnosis (NO adb/logcat)

Status: root cause **NOT yet nailed to a single line**. Two live candidates
remain — (A) a stale/cached native `.so` (build-isolation), (B) a JS-boot white
screen. The suspected `webview-data-url` panic is **disproven**. This doc
captures every static probe so the next agent can take over and finish with a
clean rebuild + `strings` on the fresh artifact.

Workspace: `/root/Desktop/movie-box-2`. Crates: tauri 2.12.4, cli 2.12.2,
wry 0.57.0, tauri-runtime-wry 2.12.2. ext_new APK: `/tmp/apk/`.

## 1. CONFIRMED FIXED (not the crash)
- Cleartext → `usesCleartextTraffic="true"` in ext_new manifest; no
  `network-security-config` override blocking `http://tauri.localhost`. Webview
  can load the asset-protocol URL now (cleartext error gone).
- Icon → `logo` / bundle icons fixed.
- JNI entrypoint → `classes.dex` identical old↔new; `libmovie_box_lib.so`
  exports `Java_*` + `RegisterNatives` (no `JNI_OnLoad`); clean ELF PIE/AArch64,
  no TEXTREL, valid DT_NEEDED. JNI wiring intact.
- `server-rust` IO path (`app()`) → no `.unwrap`/`.expect`/`.abort` on the
  request path; `.exists()`/`ServeDir` safe; `config()` defaults when `.env`
  absent; `movie_box_server::app(dist_dir)` builds the router lazily (no IO).
- **`data:`-URL panic (`webview-data-url)`** → RULED OUT (3× confirmed):
  - Tauri 2.12.4 injects the global init script UNCONDITIONALLY via
    `on_webview_created` (`tauri-2.12.4/src/manager/webview.rs:527-549`) + wry
    `main_pipe.rs:180` (`initialization_scripts` loop, no cfg gate).
  - `core.js` defines `window.__TAURI_INTERNALS__` + `.invoke` regardless of the
    `webview-data-url` feature.
  - Mobile URL = `http://tauri.localhost` (asset protocol), NOT `data:`.
  - Pattern = `brownfield` (asset protocol) in BOTH old & new
    `assets/tauri.conf.json` ⇒ NOT the `data:`-URL guard
    (`webview.rs:502-506`, gated on `#[cfg(feature="webview-data-url")]`).
  ⇒ Enabling `webview-data-url` is a NO-OP for this crash.
- **`server_origin`** → RULED OUT as the boot invoke. Body string
  `http://moviebox.localhost` IS in the `.so` (×1) but the dispatch NAME
  `server_origin` is absent (`grep -aoc = 0`); `api_request` = 1. The running
  bundled JS does NOT call `server_origin` either (literal 0 ⇒ no
  `invoke("server_origin")` in the bundle). App never asks for it / never gets
  it ⇒ mutually-consistent stale pair ⇒ NOT the boot invoke. (It IS a staleness
  symptom — see §2.)

## 2. STALENESS / BUILD-ARTIFACT INCONSISTENCY (real anomaly, may be causal)
- `git show f5ce173` (HEAD) = a 2-line "ascii byte string" warning fix in
  `src-tauri/src/lib.rs`; does NOT touch `generate_handler!` or `server_origin`.
- Source HEAD = `generate_handler![server_origin, api_request]` (lib.rs:245),
  `fn server_origin()` (lib.rs:58), `PROTOCOL_ORIGIN` (lib.rs:23).
  `src/main.js:20` = `tauri.core.invoke("server_origin")`.
- But ext_new `libmovie_box_lib.so`: `server_origin` dispatch = 0, `api_request`
  = 1, yet already carries 5402266-era features (`Backend`, `api_request`,
  `run().unwrap_or_else(abort)`, `create_dir_all`, `getPluginManager`/
  `onWebViewCreated`, `[movie-box] erreur au lancement`).
- A lib.rs with `Backend`+`api_request`+abort BUT `generate_handler!` listing
  only `api_request` (no `server_origin`) matches **NO git commit** (5402266 and
  later list `server_origin`) ⇒ **the shipped `.so` is a stale/cached cargo-NDK
  artifact** (incremental build did not recompile `src-tauri/src/lib.rs` after
  `server_origin` was wired into the macro).
- dist JS likewise stale vs HEAD `main.js` (no `server_origin` call) ⇒ dist and
  `.so` come from different points in history (frankenstein build). On-disk HEAD
  cannot be trusted to predict the running APK.
- Because the running bundle doesn't call `server_origin`, this staleness is the
  #1 build-hygiene finding but is **not by itself the launch crash** (the stale
  pair is mutually consistent). It IS why "is `server_origin` in the `.so`" must
  be re-checked after a clean rebuild.

## 3. TWO REMAINING LIVE CANDIDATES (not statically resolved)

### A. `run()` hard-abort via `unwrap_or_else(abort)` (lib.rs:245-252)
```
.invoke_handler(tauri::generate_handler![server_origin, api_request])
.run(tauri::generate_context!())
.unwrap_or_else(|error| {
    eprintln!("[movie-box] erreur au lancement : {error}");
    std::process::abort()
});
```
- `[movie-box] erreur au lancement :` IS in `.so` rodata ⇒ this branch is
  compiled in and reachable on mobile.
- `setup()` is clean (no `?` that Errs on Android: `app_data_dir` has
  `unwrap_or_else`, `create_dir_all` is `let _=`, `router.set` is `let _`,
  `config()` defaults, `movie_box_server::app()` lazy). So abort, if any, is
  AFTER setup — in tauri/wry mobile webview creation (Kotlin
  `TauriActivity.getPluginManager`+`onWebViewCreated`, webview.rs:529-548).
- `getPluginManager`/`onWebViewCreated` ×1 in `.so`; old & new dex identical ⇒
  old reached webview load (it hit a cleartext error) ⇒ webview creation did NOT
  abort in old ⇒ this path worked. If NEW aborts, the trigger is a
  newly-introduced difference — Rust delta old→new = the `setup()` +
  `run().unwrap_or_else(abort)` + moviebox protocol. Nothing in `setup()`
  Errs on Android ⇒ A is probably NOT firing unless a version-skew
  (cli 2.12.2 Kotlin vs tauri 2.12.4 runtime) broke the `on_webview_created`/
  init-script JNI contract.
- Open static check: grep the fresh `.so` for **tauri::Error Display strings**
  (match against `tauri-2.12.4/src/error.rs` / `Error` enum `Display`). Current
  grep only surfaced generic tokens; no definitive Display text. If no abort
  string appears at launch, the crash is NOT the `run()` abort.

### B. JS boot white-screen (not a native abort)
- Webview loads `http://tauri.localhost/index.html` → JS boots.
- Init: `core.js` defines `window.__TAURI_INTERNALS__` (webview.rs:175) +
  `.invoke` (core.js:81) ⇒ `invoke` available; `__TAURI__` via
  `__TAURI_IIFE__` (bundle.global.js). No `webview-data-url` gate.
  ⇒ JS can call `invoke('api_request', {path:'/api/home'})`.
- `api_request` dispatch IS registered (`api_request` ×1 in `.so`) ⇒ resolves.
  Router set in `setup()` (OnceLock); early invoke returns 503
  `'backend en cours de démarrage, réessaie'` (lib.rs:200) — JS retries.
- `/api/home` → TMDB (key empty → "indisponible" rails) + internal API default
  `http://127.0.0.1:8000` (`INTERNAL_API_ENABLED=true` mobile, lib.rs:222) — but
  `services/internal.rs` delegates to `moviebox.rs::source` which fetches
  `https://h5.aoneroom.com`/`h5-api.aoneroom.com` (NOT 127.0.0.1). So
  `127.0.0.1:8000` is just a label here; real fetches go HTTPS.
- No eager `.unwrap`/`.expect`/`.abort` in `moviebox.rs` (only header
  `.parse().unwrap()` on static literals; `Mutex::lock().unwrap()` = poison-only),
  `tmdb.rs`, `internal.rs`.
- **`http.rs:16` `Client::builder().build().expect("client HTTP")`** — could panic
  on Android IF reqwest is built with `native-tls`/openssl. Must check
  `server-rust/Cargo.toml` reqwest features (rustls vs native). APK linked ⇒ not
  a link error, but `build().expect` still panics at runtime if the TLS backend
  fails to initialize.
- `routes/downloads.rs`/`./internal.rs` `.unwrap()` on `HeaderValue::from_str`
  for `bytes */{size}` — reachable only on range/stream requests (post-boot) ⇒
  unlikely launch crash.
- `handle_protocol_request` (lib.rs:145-179) returns 500/502 JSON (no panic);
  media-proto callback (189-211) returns 503 when router absent (no panic).

## 4. FILES READ (paths + ranges)
- `src-tauri/src/lib.rs` 145-256 (`run`, `setup`, `handle_protocol_request`,
  `register_asynchronous_uri_scheme_protocol`, `invoke_handler`).
- `src-tauri/tauri.conf.json` (source: `withGlobalTauri:true`, no `pattern` key,
  `frontendDist:"../dist"`).
- ext_new `assets/tauri.conf.json` (pattern=`brownfield`, withGlobalTauri=true,
  assetProtocol.enable=false) + ext_OLD `assets/tauri.conf.json`
  (pattern=`brownfield` too).
- `server-rust/src/lib.rs`, `src/config.rs` (full).
- `server-rust/src/http.rs` 1-186, `cache.rs`, `middleware.rs`, `error.rs`.
- `server-rust/src/services/moviebox.rs` (full), `tmdb.rs`, `internal.rs`,
  `routes/api.rs`, `routes/internal.rs`, `routes/downloads.rs`.
- `tauri-2.12.4/src/manager/webview.rs` 440-690 (init-script injection at 175/187;
  data-URL guard 502-506; asset-protocol url 468-498; `on_webview_created`
  Android closure 527-549).
- `tauri-2.12.4/scripts/core.js` (full; `__TAURI_INTERNALS__` defined by tauri).
- `tauri-2.12.4/scripts/` (init.js, ipc.js, ipc-protocol.js, bundle.global.js,
  pattern.js, isolation.js, freeze_prototype.js, process-ipc-message-fn.js).
- `wry-0.57.0/src/android/main_pipe.rs` (CreateWebView `initialization_scripts`
  loop at 180 — UNCONDITIONAL).
- `wry-0.57.0/src/android/kotlin/WryActivity.kt`, `RustWebViewClient.kt`.
- `git show f5ce173` (HEAD diff = 2-line ascii byte-string warning fix).

## 5. NEXT AGENT MUST DO (to close the root cause)
1. **Clean rebuild** APK (clear `~/.cargo` registry cache + NDK/gradle
   `app/build` + cargo fingerprint; `touch` `src-tauri/src/lib.rs`; run
   `cargo build` then `tauri mobile build`). Then:
   `grep -aoc server_origin <fresh>.so` MUST be >=1 and `api_request` >=1.
   - If still 0 ⇒ cargo/gradle still caching ⇒ bust harder.
2. `strings <fresh>.so | grep -aoE '...'` and match tauri `Error` Display
   strings (§3-A). If no abort string at launch ⇒ NOT the `run()` abort.
3. Read `server-rust/Cargo.toml` reqwest features (rustls vs native-tls/openssl)
   — if `native-tls`+openssl missing on NDK, `http.rs:16` `.expect` panics on
   first fetch (§3-B).
4. If rebuild is clean & `server_origin` present & no abort string ⇒ JS-side.
   Then diff freshly-bundled `index.html`+vite-manifest+chunk-hashes vs
   `ext_new/assets/`, and add a try/catch around the boot `invoke` in
   `src/main.js` (+ 503-retry handling) + a `window.onerror` to capture the
   JS error (logcat substitute).

## 6. VERDICT (honest, evidence-scoped)
- The suspected `webview-data-url` global-init panic is **disproven** (init
  scripts injected unconditionally in tauri 2.12.4; brownfield ≠ data URL;
  `__TAURI_INTERNALS__` defined by `webview.rs:175`/`core.js`). **Do NOT add the
  `webview-data-url` feature** — it is a no-op here.
- The strongest STATIC finding is a **stale/cached native build** (`.so` does not
  match HEAD source: missing `server_origin` dispatch). Fix = **clean rebuild**
  (no source edit).
- The actual launch-crash mechanism is unresolved statically: either (A) `run()`
  abort (need the `tauri::Error` Display string from the fresh `.so`) or (B) a JS
  boot rejection after `api_request` (need logcat / a JS error reporter).
- Candidate code edits (pending §5 confirmation): NONE applied yet.

## 7. REPRODUCIBLE COMMANDS
```
grep -aoc server_origin /tmp/apk/ext_new/lib/arm64-v8a/libmovie_box_lib.so   # 0
grep -aoc api_request  /tmp/apk/ext_new/lib/arm64-v8a/libmovie_box_lib.so    # 1
unzip -p /tmp/apk/app-universal-release.apk assets/tauri.conf.json \
  | grep -ao '"pattern":{"use":"[a-z]*"}'                              # brownfield (old==new)
git --no-pager show f5ce173 --stat
```
