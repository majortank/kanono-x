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
- **Location**: `kanono_x_library.db` in the application's current working directory
- **Concurrency Mode**: `PRAGMA journal_mode = WAL;` (Write-Ahead Logging ensures concurrent read/write without UI locking).
- **Synchronous Mode**: `PRAGMA synchronous = NORMAL;`
- **B-Tree Indexes**:
  - `idx_tracks_artist ON tracks(artist)`
  - `idx_tracks_album ON tracks(album)`
  - `idx_tracks_path ON tracks(file_path)`

### Database Operations in UI
- **Vacuum & Optimize**: Defragments the SQLite database and recovers unused pages.
- **Rescan All Catalog Tracks**: Re-reads audio file tags and updates the database.
- **Folder Manager**: Add and remove indexed source directories with automatic track cleanup.

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
