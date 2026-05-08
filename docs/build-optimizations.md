# Build optimizations & hydration fix

## Problem summary

After deploying to Cloud Run (us-central1), the app showed a persistent **"Loading…"** spinner and WASM hydration errors in the browser console:

```
Cannot read properties of undefined (reading 'toString')   [hydrate_node crash]
POST / 405 Method Not Allowed                              [wrong server-fn URL]
```

Three independent bugs combined to cause this.

---

## Fix 1 — Remove `enable_out_of_order_streaming()`

**File:** `src/web/server.rs`

```rust
// Before
.serve_dioxus_application(
    ServeConfig::new().enable_out_of_order_streaming(),
    crate::web::app::App,
)

// After
.serve_dioxus_application(
    ServeConfig::new(),
    crate::web::app::App,
)
```

**Why it broke things:** Dioxus 0.7 out-of-order streaming chunks the HTML response and streams suspense boundaries as they resolve. The WASM client reconstructs server function URLs from the streamed HTML. With streaming enabled, the reconstructed URL came out as `/` instead of the correct `/api/…` path, causing every server function call to hit `POST /` (which returns 405). The mismatch also triggered a `hydrate_node` crash during DOM reconciliation.

**Root cause:** Dioxus 0.7 streaming + fullstack server functions have a known interaction bug where the base URL for server functions is derived from the streamed document root, not the actual endpoint registry.

---

## Fix 2 — Remove `--wasm-split`

**File:** `Dockerfile`

```dockerfile
# Before
dx build --release --package jogga --features=wasm-split --wasm-split

# After
dx build --release --package jogga
```

**Why it broke things:** `wasm-split` splits the compiled WASM binary into a main module plus per-route lazy-loaded modules. On SSR-rendered routes that have no `use_resource` (e.g. `/login`, `/reset-password`, the DNQ 404 page), the server sends complete HTML. The main WASM module loads and immediately tries to hydrate that HTML before the route-specific split module has loaded. This causes a hydration mismatch crash on those routes.

Routes with `use_resource` (e.g. the profile page) are safe because they render a `SuspenseBoundary` / loading state in SSR, so there's nothing to mis-hydrate against.

**Trade-off:** Without `wasm-split`, the full WASM binary is downloaded on first load (~1.3 MB gzipped after fix 3). With `wasm-split` this would be smaller on initial load but requires Dioxus to fix the hydration ordering. Re-enable when the framework handles it correctly.

---

## Fix 3 — `strip = "debuginfo"` unblocks `wasm-opt`

**File:** `Cargo.toml`

```toml
[profile]
release = { strip = "debuginfo" }
```

**Why this matters:** `wasm-opt` (Binaryen's optimizer, run automatically by `dx build --release`) crashed with:

```
SIGABRT: compile unit size was incorrect — unsupported DWARF
```

The DWARF debug info embedded in the release WASM used a section format that `wasm-opt` does not support. Stripping debug info before `wasm-opt` runs eliminates the problematic DWARF sections.

**Effect:** WASM binary dropped from **~5 MB** (unoptimized, wasm-opt skipped) to **~1.3 MB** (wasm-opt applied with `opt-level = "z"`, LTO, `codegen-units = 1` from the `release-wasm` profile). This is roughly a 75% reduction in transfer size and directly improves Time-to-Interactive.

---

## Remaining latency

After all three fixes, TTFB is ~1.2 s from Singapore (Cloudflare SIN edge → us-central1 Iowa). This is geographic latency and is not addressable without moving the Cloud Run region or adding edge caching for SSR HTML. Server-side processing time is ~12 ms.
