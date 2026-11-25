# ⚡ Kanono X — Architecture & Advanced Features Manual

## 1. Overview & Evolution

**Kanono X** represents a major architectural upgrade from the original `kanono-player`. It was designed specifically to address latency and UI lag while introducing **Foobar2000-grade folder navigation**, **SQLite WAL catalog storage**, and an extensible **Omarchy-inspired theme engine** featuring authentic **CRT phosphor terminal aesthetics**.

---

## 2. Zero-Lag Engine Optimization

### The Problem in Legacy Kanono Player
The original version suffered from two major performance bottlenecks:
1. **Uncapped Repaint Busy-Loop**: During playback or visualizer animation, the application called `ctx.request_repaint()` on every iteration without timing control. On modern GPUs and high refresh-rate monitors, this caused egui to render at over 1,000 FPS, pegging CPU core utilization at 100% and causing thermal throttling and stutter.
2. **Unbounded Table Layout**: Every track in the library was rendered as a full widget grid simultaneously. When the library exceeded 1,000 tracks, egui choked attempting to compute bounding boxes for thousands of off-screen elements every frame.

### The Solution in Kanono X
1. **Adaptive 60 FPS Event Limiter**:
   ```rust
   if self.audio.is_playing && !self.audio.is_paused {
       self.visualizer_phase += 0.06;
       // Smooth 60 FPS repaint during active audio playback (16ms)
       ctx.request_repaint_after(std::time::Duration::from_millis(16));
   } else if self.scanning > 0 || self.yt_is_searching || self.yt_is_converting {
       ctx.request_repaint_after(std::time::Duration::from_millis(100));
   }
   ```
   When audio is paused or idle, CPU consumption drops to near zero.
2. **Virtualized Row Slicing with `show_rows`**:
   ```rust
   egui::ScrollArea::vertical()
       .show_rows(ui, row_height, num_rows, |ui, row_range| {
           for row_idx in row_range {
               let i = order[row_idx];
               // Only renders the ~25 rows currently visible on the screen!
           }
       });
   ```
   With virtual scrolling, Kanono X maintains a silky-smooth 144Hz framerate even when indexing **50,000+ tracks**.

---

## 3. Foobar2000-Style Folder Traversal

The library tab features a dedicated hierarchical directory tree manager:

- **Recursive Tree Hierarchy**: Automatically decomposes file paths into parent roots, folders, and subfolders.
- **Tree Node State**:
  - `▶` (Collapsed) / `▼` (Expanded) chevrons.
  - Track count indicators per directory (`total_tracks` including descendants).
- **Subfolder Matching Modes**:
  - `[✓ Subfolders]`: Selecting `/home/user/Music/Rock` includes tracks in `Rock/Classic`, `Rock/Prog`, etc.
  - `[ ] Subfolders`: Selecting `/home/user/Music/Rock` shows only tracks stored directly in `/Rock/`.
- **Active Path Breadcrumbs**:
  - Located directly above the track table: `📁 Path: /home/user/Music/Electronic/Synthwave [✖ Clear]`.
  - Clicking `✖` instantly resets the folder filter to view all library tracks.
- **Multi-Field Live Filter**:
  - Live filtering across **All Fields**, **Title**, **Artist**, **Album**, **File Name**, and **Folder**.
  - Real-time result counter: `Showing X of Y tracks`.

---

## 4. Multi-Theme System & CRT Cathode Ray Effects

Kanono X includes 8 built-in palettes matching the **Omarchy Desktop** styling and vintage **CRT terminals**:

| Theme Key | Name | Visual Style |
|---|---|---|
| `CrtGreen` | 📺 **CRT Green Phosphor** | Default flagship P1 monochrome green phosphor with cathode glow |
| `CrtAmber` | 📺 **CRT Amber Terminal** | Industrial warm P3 amber terminal glow |
| `CrtCyan` | 📺 **CRT Cyber Cyan** | High-energy radioactive cyan vector beam |
| `OmarchyEmerald` | 🌿 **Omarchy Obsidian Emerald** | Official Omarchy desktop obsidian slate with emerald highlights |
| `OmarchyCyberpunk` | ⚡ **Omarchy Cyberpunk Neon** | Obsidian void infused with neon purple, electric magenta, and cyan |
| `OmarchyNord` | ❄ **Omarchy Nord Frost** | Arctic Nord slate and glacial ice blue accents |
| `OmarchyDracula` | 🦇 **Omarchy Dracula** | Vampiric dark theme with neon pink, purple, and mint |
| `SolarizedLight` | ☀ **Solarized Daylight** | High-contrast daytime light mode |

### CRT Scanline Shader
When enabled (`Settings > Enable Retro CRT Scanlines & Cathode Ray Overlay`), an overlay painter draws horizontal beam scanlines with subtle cathode ray spacing across the entire viewport.

---

## 5. Resilient SQLite Media Catalog (WAL Mode)

Metadata and library states are persisted via an embedded SQLite database (`rusqlite` bundled):
- **Location**: `~/.local/share/kanono-x/kanono_x_library.db` (via the platform's XDG data directory)
- **Concurrency Mode**: `PRAGMA journal_mode = WAL;` (Write-Ahead Logging ensures concurrent read/write without UI locking).
- **Synchronous Mode**: `PRAGMA synchronous = NORMAL;`
- **Metadata Reader**: Pure-Rust `lofty` tag and audio-property parsing; no `ffprobe` subprocess is required.
- **B-Tree Indexes**:
  - `idx_tracks_artist ON tracks(artist)`
  - `idx_tracks_album ON tracks(album)`
  - `idx_tracks_path ON tracks(file_path)`

### Database Operations in UI
- **Vacuum & Optimize**: Defragments the SQLite database and recovers unused pages.
- **Rescan All Catalog Tracks**: Re-reads audio file tags and updates the database.
- **Folder Manager**: Add and remove indexed source directories with automatic track cleanup.

### Persistent duplicate index

Download preflight uses the same SQLite catalog to avoid unnecessary network transfers:

- Local tracks store file size and modification time, normalized artist/title/album keys, track number, MusicBrainz recording ID, ISRC, and an optional `fpcalc` fingerprint.
- Rescans upsert new and modified files and remove missing files under the scanned roots; downloads query the index without a full filesystem scan.
- Matching is conservative. Exact identifiers and fingerprints have priority, while metadata matching requires normalized artist/title, compatible version markers, and duration within 3% or two seconds.
- Multiple plausible metadata matches are treated as ambiguous and block automatic reuse.

The duplicate policy is stored in the `app_settings` table under `duplicate_policy`. The default is `skip`. Provider and Audius downloads stage files first, then move them into `Artist/Album`; missing album metadata uses `Artist/Singles`.

### Provider album fields

Provider tab output accepts these tab-separated fields:

```text
id, title, artist, duration_seconds, url, downloadable, support_url,
album, playlist, track_number
```

For yt-dlp, album and playlist metadata are read during result resolution. The album is preferred, with the playlist name used as a fallback for playlist and album URLs. Batch downloads process each result independently, so existing tracks can be reused without redownloading the rest of the playlist.

---

## 6. How to Run

```bash
cd app
cargo run --release
```

Or run the pre-built release binary directly:
```bash
./app/target/release/kanono-x
```
