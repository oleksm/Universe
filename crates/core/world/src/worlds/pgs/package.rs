//! PGS1 revision 4 packages: a root and regions, each replacing whole root faces, read mapped.
//! The cell index is the load path: no tree, no section read whole; the manifest's opening
//! records are checked at open and every 64 KiB block against its kind-19 hash on first touch.
//! A region's records are read on its first touch. Composed face ids follow the contract: root
//! faces ascending, a region's faces (stored, canonical order) where its lowest footprint face
//! stood, replaced faces dropped.
//!
//! Revision 5 levels: a region may list coarser files of the same footprint (`levels[]`, each a
//! vertex subset of level 0 with a measured deviation). A query with a tolerance is answered from
//! the coarsest representation (the root, a level, level 0) whose declared deviation is at or below
//! it, nothing finer opened; tolerance 0 is revision 4 exactly. Composed ids, neighbours and face
//! geometry are level 0's alone.
use super::{FaceGeometry, NONE, Sample, TIE, Water};
use glam::DVec3;
use memmap2::Mmap;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

fn hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

#[derive(Clone, Copy)]
struct Section {
    offset: usize,
    count: usize,
}
#[derive(Clone, Copy)]
struct Window {
    face: u32,
    n: u32,
    i0: u32,
    j0: u32,
    ni: u32,
    nj: u32,
    start: u32,
}

/// One mapped PGS1 file: header, strings and table read; records on demand, each block checked
/// on first touch.
struct File {
    name: String,
    map: Mmap,
    /// Kept for releasing the page cache (`release`).
    handle: std::fs::File,
    capabilities: u32,
    strings: Vec<String>,
    sections: HashMap<u32, Section>,
    windows: Vec<Window>,
    prefix: usize,
    block: usize,
    hashes: usize,
    blocks: usize,
    verified: Vec<AtomicU64>,
    /// Bytes read so far: header and table, the hash section, every block checked.
    read: AtomicU64,
    opened: Instant,
    first_block_ms: OnceLock<f64>,
    /// The manifest's height bounds: a point outside them is refused when read.
    heights: (f64, f64),
    /// Milliseconds after open of the last block touch (residency).
    used: AtomicU64,
    /// Times the file's pages were released.
    releases: AtomicU64,
}

impl File {
    fn open(path: &Path, entry: &Value) -> Result<Self, String> {
        let opening = &entry["opening"];
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let fail = |what: &str| format!("{name}: {what}");
        let file = std::fs::File::open(path).map_err(|e| fail(&e.to_string()))?;
        // SAFETY: the package is immutable while the process runs; every block read is checked
        // against its hash before use.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| fail(&e.to_string()))?;
        let _ = map.advise(memmap2::Advice::Random);
        let n = |k: &str| opening[k].as_u64().map(|v| v as usize).ok_or_else(|| fail(&format!("opening.{k}")));
        let (prefix, block, hashes, blocks) = (n("prefix_bytes")?, n("block_bytes")?, n("block_hashes_offset")?, n("blocks")?);
        if block == 0 || hashes < prefix || hashes + blocks * 32 > map.len() || blocks != (hashes - prefix).div_ceil(block) {
            return Err(fail("opening record out of shape"));
        }
        if hex(&map[..prefix]) != opening["prefix_sha256"] {
            return Err(fail("header/table sha256 differs from the manifest"));
        }
        if hex(&map[hashes..hashes + blocks * 32]) != opening["block_hashes_sha256"] {
            return Err(fail("block hashes sha256 differs from the manifest"));
        }
        let at = |o: usize, l: usize| map.get(o..o + l).ok_or_else(|| fail("read outside file"));
        let u32_at = |o: usize| at(o, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let u64_at = |o: usize| at(o, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()) as usize);
        if at(0, 4)? != b"PGS1" { return Err(fail("not PGS1")); }
        let version = u16::from_le_bytes(at(4, 2)?.try_into().unwrap());
        let capabilities = u32_at(8)?;
        if capabilities & 16 == 0 { return Err(fail("no cell index (capability bit 4)")); }
        // Revision 5 adds an eleventh string, `level`; ten means level 0.
        let want = match (version, u32_at(32)?) { (1, 7) => 7, (2, 10) => 10, (2, 11) => 11, (1 | 2, _) => return Err(fail("header string count")), (v, _) => return Err(fail(&format!("version {v}"))) };
        let (mut o, mut strings) = (36, vec![]);
        for _ in 0..want {
            let l = u32_at(o)? as usize;
            strings.push(String::from_utf8(at(o + 4, l)?.to_vec()).map_err(|_| fail("header string"))?);
            o += 4 + l;
        }
        o = o.div_ceil(8) * 8;
        let entries = u32_at(o)? as usize;
        if o + 8 + entries * 72 != prefix { return Err(fail("prefix is not header and table")); }
        let mut sections = HashMap::new();
        let mut last = (0, 0);
        for k in 0..entries {
            let e = o + 8 + k * 72;
            let (kind, flags, stride) = (u32_at(e)?, u32_at(e + 8)?, u32_at(e + 12)? as usize);
            let (offset, length, count) = (u64_at(e + 16)?, u64_at(e + 24)?, u64_at(e + 32)?);
            if offset % 8 != 0 || offset < prefix || offset + length > map.len() || (stride != 0 && length != count * stride) {
                return Err(fail(&format!("section {kind} out of shape")));
            }
            if flags & 1 != 0 && !matches!(kind, 1..=13 | 15..=20) { return Err(fail(&format!("required section {kind} unknown"))); }
            if kind == 14 { return Err(fail("compact points are unsupported")); }
            // Sections ascend by kind, except the block hashes, last whatever their number.
            if kind != 19 && last.1 != 19 && kind < last.1 { return Err(fail("sections out of kind order")); }
            last = last.max((offset, kind));
            sections.insert(kind, Section { offset, count });
        }
        if last.1 != 19 || sections[&19].offset != hashes || sections[&19].count != blocks {
            return Err(fail("block hashes are not the last section where the manifest says"));
        }
        for kind in [1, 2, 11, 12, 17] {
            if !sections.contains_key(&kind) { return Err(fail(&format!("missing section {kind}"))); }
        }
        let w = sections[&17];
        let mut windows = vec![];
        for k in 0..w.count {
            let r = |f: usize| u32_at(w.offset + k * 32 + 4 * f);
            windows.push(Window { face: r(0)?, n: r(1)?, i0: r(2)?, j0: r(3)?, ni: r(4)?, nj: r(5)?, start: r(6)? });
        }
        let heights = (entry["height_min_m"].as_f64().ok_or_else(|| fail("manifest height_min_m"))?, entry["height_max_m"].as_f64().ok_or_else(|| fail("manifest height_max_m"))?);
        let verified = (0..blocks.div_ceil(64)).map(|_| AtomicU64::new(0)).collect();
        let file = Self {
            name, map, handle: file, capabilities, strings, sections, windows, prefix, block, hashes, blocks,
            verified, read: AtomicU64::new((prefix + blocks * 32) as u64), opened: Instant::now(), first_block_ms: OnceLock::new(),
            heights, used: AtomicU64::new(0), releases: AtomicU64::new(0),
        };
        for (field, kind) in [("faces", 2), ("points", 1)] {
            if entry[field].as_u64() != Some(file.count(kind) as u64) { return Err(file.fail(&format!("{field} differ from the manifest"))); }
        }
        // (Windows and cells sit in blocks too: check what was just read.)
        file.at(w.offset, w.count * 32)?;
        Ok(file)
    }
    fn fail(&self, what: &str) -> String {
        format!("{}: {what}", self.name)
    }
    /// `len` bytes at `offset`, every block they lie in checked first.
    fn at(&self, offset: usize, len: usize) -> Result<&[u8], String> {
        let bytes = self.map.get(offset..offset.checked_add(len).ok_or("overflow")?).ok_or_else(|| self.fail("read outside file"))?;
        let end = (offset + len).min(self.hashes);
        self.used.store((self.opened.elapsed().as_secs_f64() * 1e3) as u64, Ordering::Relaxed);
        if len == 0 || end <= self.prefix.max(offset) {
            return Ok(bytes);
        }
        for k in (offset.max(self.prefix) - self.prefix) / self.block..=(end - 1 - self.prefix) / self.block {
            let (word, bit) = (&self.verified[k / 64], 1u64 << (k % 64));
            if word.load(Ordering::Acquire) & bit != 0 { continue; }
            let start = self.prefix + k * self.block;
            let stop = (start + self.block).min(self.hashes);
            // One read for the block, not a page fault per 4 KiB in turn.
            let _ = self.map.advise_range(memmap2::Advice::WillNeed, start, stop - start);
            if Sha256::digest(&self.map[start..stop]).as_slice() != &self.map[self.hashes + k * 32..self.hashes + k * 32 + 32] {
                return Err(self.fail(&format!("block {k} sha256 differs from kind 19")));
            }
            if word.fetch_or(bit, Ordering::AcqRel) & bit == 0 {
                self.read.fetch_add((stop - start) as u64, Ordering::Relaxed);
                self.first_block_ms.get_or_init(|| self.opened.elapsed().as_secs_f64() * 1e3);
            }
        }
        Ok(bytes)
    }
    fn u32(&self, o: usize) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.at(o, 4)?.try_into().unwrap()))
    }
    fn f64(&self, o: usize) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.at(o, 8)?.try_into().unwrap()))
    }
    fn count(&self, kind: u32) -> usize {
        self.sections.get(&kind).map_or(0, |s| s.count)
    }
    fn section(&self, kind: u32) -> Result<Section, String> {
        self.sections.get(&kind).copied().ok_or_else(|| self.fail(&format!("no section {kind}")))
    }
    fn point(&self, i: u32) -> Result<(DVec3, f64), String> {
        let o = self.section(1)?.offset + i as usize * 40;
        let b = self.at(o, 32)?;
        let f = |k: usize| f64::from_le_bytes(b[k * 8..k * 8 + 8].try_into().unwrap());
        if !(self.heights.0..=self.heights.1).contains(&f(3)) { return Err(self.fail(&format!("point {i} height outside the manifest's bounds"))); }
        Ok((DVec3::new(f(0), f(1), f(2)), f(3)))
    }
    /// Vertices, neighbours, body, (rock, pattern).
    fn face(&self, i: u32) -> Result<([u32; 3], [u32; 3], u32, (u16, u16)), String> {
        let o = self.section(2)?.offset + i as usize * 32;
        let b = self.at(o, 32)?;
        let u = |k: usize| u32::from_le_bytes(b[k * 4..k * 4 + 4].try_into().unwrap());
        Ok(([u(0), u(1), u(2)], [u(3), u(4), u(5)], u(6), (u(7) as u16, (u(7) >> 16) as u16)))
    }
    fn root_point(&self, i: u32) -> Result<u32, String> {
        self.u32(self.section(16)?.offset + i as usize * 4)
    }
    /// The faces of q's cell (none outside every window): every face that may hold q.
    fn candidates(&self, q: DVec3) -> Result<Vec<u32>, String> {
        let face = cube_face(q);
        let Some(w) = self.windows.iter().find(|w| w.face == face) else { return Ok(vec![]) };
        let (_, i, j) = cube_cell(q, w.n);
        if i < w.i0 || j < w.j0 || i >= w.i0 + w.ni || j >= w.j0 + w.nj { return Ok(vec![]); }
        let c = self.section(11)?.offset + (w.start + (j - w.j0) * w.ni + (i - w.i0)) as usize * 8;
        let (start, count) = (self.u32(c)? as usize, self.u32(c + 4)? as usize);
        let bytes = self.at(self.section(12)?.offset + start * 4, count * 4)?;
        Ok(bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect())
    }
    fn corners(&self, f: u32) -> Result<([DVec3; 3], [f64; 3]), String> {
        let v = self.face(f)?.0;
        let [(a, ha), (b, hb), (c, hc)] = [self.point(v[0])?, self.point(v[1])?, self.point(v[2])?];
        Ok(([a, b, c], [ha, hb, hc]))
    }
    /// As the canonical reader: chord-plane edges, tie 1e-12.
    fn holds(&self, f: u32, q: DVec3) -> Result<bool, String> {
        let ([a, c, d], _) = self.corners(f)?;
        Ok([a.cross(c - a), c.cross(d - c), d.cross(a - d)].iter().all(|e| q.dot(e.normalize()) >= -TIE))
    }
    /// The canonical order's key: the cell curve of the face's centroid on this file's grid,
    /// then its vertex triple under composed keys (kept by root id, then added in stored order).
    fn order_key(&self, f: u32) -> Result<(u64, [(u8, u32); 3]), String> {
        let v = self.face(f)?.0;
        let ([a, b, c], _) = self.corners(f)?;
        let key = |p: u32| -> Result<(u8, u32), String> {
            let r = self.root_point(p)?;
            Ok(if r == NONE { (1, p) } else { (0, r) })
        };
        let k = [key(v[0])?, key(v[1])?, key(v[2])?];
        let r = (0..3).min_by_key(|&i| k[i]).unwrap();
        Ok((curve((a + b + c) / 3., self.windows[0].n), [k[r], k[(r + 1) % 3], k[(r + 2) % 3]]))
    }
    /// The writer's rule, checked on touch: stored order non-decreasing in the canonical key.
    fn check_order(&self, f: u32) -> Result<(), String> {
        let n = self.count(2) as u32;
        let here = self.order_key(f)?;
        if (f > 0 && self.order_key(f - 1)? > here) || (f + 1 < n && self.order_key(f + 1)? < here) {
            return Err(self.fail(&format!("region faces not in canonical order (face {f})")));
        }
        Ok(())
    }
    /// Bytes of this file the page cache holds now (mincore), whoever read them; None off Unix.
    fn resident(&self) -> Option<u64> {
        #[cfg(unix)]
        {
            let pages = self.map.len().div_ceil(4096);
            let mut v = vec![0u8; pages];
            // SAFETY: the mapping is live for self's lifetime; v holds one byte per page.
            let r = unsafe { libc::mincore(self.map.as_ptr() as *mut _, self.map.len(), v.as_mut_ptr()) };
            (r == 0).then(|| v.iter().filter(|&&b| b & 1 != 0).count() as u64 * 4096)
        }
        #[cfg(not(unix))]
        None
    }
    /// Drop this file's pages: from the mapping (madvise) and from the page cache (fadvise), and
    /// forget which blocks were checked, so a later touch reads and checks them again.
    fn release(&self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: the mapping is read-only and file-backed, so dropping its pages loses nothing
            // (a later read faults them back from the file); the descriptor is valid for self's
            // lifetime and fadvise is advice only.
            unsafe {
                libc::madvise(self.map.as_ptr() as *mut _, self.map.len(), libc::MADV_DONTNEED);
                libc::posix_fadvise(self.handle.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
            }
        }
        for w in &self.verified { w.store(0, Ordering::Release); }
        self.releases.fetch_add(1, Ordering::Relaxed);
    }
    fn idle_ms(&self) -> f64 {
        self.opened.elapsed().as_secs_f64() * 1e3 - self.used.load(Ordering::Relaxed) as f64
    }
    fn report(&self) -> Value {
        let checked: u32 = self.verified.iter().map(|w| w.load(Ordering::Relaxed).count_ones()).sum();
        json!({"file":self.name,"bytes":self.map.len(),"bytes_read":self.read.load(Ordering::Relaxed),"blocks":self.blocks,"blocks_checked":checked,"resident_bytes":self.resident(),
            "first_block_ms_after_open":self.first_block_ms.get(),"releases":self.releases.load(Ordering::Relaxed),"idle_ms":self.idle_ms()})
    }
}

/// Cube face of a direction: largest absolute coordinate, ties to the earlier of x, y, z.
fn cube_face(q: DVec3) -> u32 {
    let a = q.abs();
    let axis = if a.x >= a.y && a.x >= a.z { 0 } else if a.y >= a.z { 1 } else { 2 };
    2 * axis + (q[axis as usize] < 0.) as u32
}
fn cube_cell(q: DVec3, n: u32) -> (u32, u32, u32) {
    let face = cube_face(q);
    let a = q.abs();
    let (s, t, m) = match face { 0 | 1 => (q.y, q.z, a.x), 2 | 3 => (q.z, q.x, a.y), _ => (q.x, q.y, a.z) };
    let cell = |u: f64| (((u + 1.) / 2. * n as f64).floor() as i64).clamp(0, n as i64 - 1) as u32;
    (face, cell(s / m), cell(t / m))
}
/// The cell curve: cube face, then the Morton code of (i, j) (i in the even bits, j in the odd).
fn curve(q: DVec3, n: u32) -> u64 {
    let spread = |x: u32| {
        let mut x = x as u64;
        x = (x | x << 16) & 0x0000_FFFF_0000_FFFF;
        x = (x | x << 8) & 0x00FF_00FF_00FF_00FF;
        x = (x | x << 4) & 0x0F0F_0F0F_0F0F_0F0F;
        x = (x | x << 2) & 0x3333_3333_3333_3333;
        (x | x << 1) & 0x5555_5555_5555_5555
    };
    let (face, i, j) = cube_cell(q, n);
    ((face as u64) << 62) + (spread(i) | spread(j) << 1)
}

/// A region: its manifest entry, opened (header and table) at package open, records on touch.
struct Region {
    id: String,
    path: PathBuf,
    entry: Value,
    footprint: Vec<u32>,
    lowest: u32,
    faces: u32,
    added: usize,
    /// Opened (header, table, hash section) on first touch.
    file: OnceLock<Result<File, String>>,
    open_ms: OnceLock<f64>,
    /// Kind 18: (region face, edge, root face across), sorted by across. Read on first touch.
    boundary: OnceLock<Result<Vec<(u32, u8, u32)>, String>>,
    touched: OnceLock<f64>,
    /// Revision 5: coarser files, by increasing k and deviation; the root's deviation inside
    /// the footprint (infinite when not declared: the root never answers for the region).
    levels: Vec<Level>,
    root_deviation_max_m: f64,
}

impl Region {
    /// The reader's rule: the coarsest representation whose declared deviation is at or below
    /// the tolerance.
    fn representation(&self, tolerance_m: f64) -> Rep {
        if tolerance_m >= self.root_deviation_max_m { return Rep::Root; }
        match self.levels.iter().rposition(|l| l.deviation_max_m <= tolerance_m) {
            Some(i) => Rep::Level(i),
            None => Rep::Fine,
        }
    }
}

/// The manifest's level records, refused before anything is opened when out of order.
fn levels_of(id: &str, r: &Value, folder: &Path) -> Result<(Vec<Level>, f64), String> {
    let fail = |what: &str| format!("region {id}: {what}");
    let mut levels = vec![];
    for l in r["levels"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let k = l["level"].as_u64().ok_or_else(|| fail("level k"))? as u32;
        let (max, rms) = (l["deviation_max_m"].as_f64().ok_or_else(|| fail("level deviation_max_m"))?, l["deviation_rms_m"].as_f64().ok_or_else(|| fail("level deviation_rms_m"))?);
        if !(max.is_finite() && rms.is_finite() && rms >= 0.0 && rms <= max) { return Err(fail("level deviation not finite, or RMS above the maximum")); }
        let path = folder.join(l["file"].as_str().ok_or_else(|| fail("level file"))?);
        levels.push(Level { k, deviation_max_m: max, path, entry: l.clone(), file: OnceLock::new(), open_ms: OnceLock::new(), touched: OnceLock::new() });
    }
    if levels.first().is_some_and(|l| l.k < 1) || levels.windows(2).any(|w| w[1].k <= w[0].k) { return Err(fail("levels not listed by increasing level from 1")); }
    if levels.windows(2).any(|w| w[1].deviation_max_m < w[0].deviation_max_m) { return Err(fail("level deviations decrease")); }
    let root = match r.get("root_deviation_max_m") {
        None => f64::INFINITY,
        Some(v) => v.as_f64().filter(|d| d.is_finite() && *d >= 0.0).ok_or_else(|| fail("root_deviation_max_m"))?,
    };
    if levels.last().is_some_and(|l| root < l.deviation_max_m) { return Err(fail("root deviation below the last level's")); }
    Ok((levels, root))
}

/// A region's coarser file (revision 5): same footprint, a vertex subset of level 0, its
/// deviation from level 0 measured by the writer and declared in the manifest.
struct Level {
    k: u32,
    deviation_max_m: f64,
    path: PathBuf,
    entry: Value,
    file: OnceLock<Result<File, String>>,
    open_ms: OnceLock<f64>,
    touched: OnceLock<f64>,
}

/// Which file answers a query inside a region's footprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rep {
    /// The root's own faces (the region left closed).
    Root,
    /// Level 0, the region's own file.
    Fine,
    /// The region's `levels[i]` (an index into the list, not k).
    Level(usize),
}

/// Where composed face ids come from: a run of root faces, or a region's faces.
#[derive(Clone, Copy, Debug)]
enum Segment {
    Root { composed: u64, root: u32, len: u32 },
    Region { composed: u64, region: usize, len: u32 },
}
impl Segment {
    fn composed(&self) -> u64 {
        match *self { Segment::Root { composed, .. } | Segment::Region { composed, .. } => composed }
    }
}

pub(crate) struct Package {
    folder: PathBuf,
    manifest_sha256: String,
    root: File,
    regions: Vec<Region>,
    /// (root face, region): every footprint entry, sorted by root face.
    owner: Vec<(u32, usize)>,
    segments: Vec<Segment>,
    faces: u64,
    bodies: BTreeMap<u32, f64>,
    pub(crate) max_height: f64,
    pub(crate) radius_m: f64,
    opened_ms: f64,
    opened_at: Instant,
}

impl std::fmt::Debug for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Package({}, {} regions, {} faces)", self.folder.display(), self.regions.len(), self.faces)
    }
}

pub(crate) struct Opened {
    pub package: Package,
    pub capabilities: u32,
    pub provenance: [String; 7],
}

impl Package {
    /// Open the manifest, the root and each region's header and table; nothing read whole.
    pub(crate) fn open(folder: &Path) -> Result<Opened, String> {
        let start = Instant::now();
        let manifest_bytes = std::fs::read(folder.join("planet.json")).map_err(|e| format!("planet.json: {e}"))?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes).map_err(|e| format!("planet.json: {e}"))?;
        if manifest["format"] != "planet-graph-surface/2" { return Err("planet.json: format is not planet-graph-surface/2".into()); }
        let name = |v: &Value, what: &str| v["file"].as_str().map(|f| folder.join(f)).ok_or_else(|| format!("planet.json: {what} file"));
        let root = File::open(&name(&manifest["root"], "root")?, &manifest["root"])?;
        if !root.strings.get(8).is_none_or(|id| id.is_empty()) { return Err("root.pgs names a region id".into()); }
        let footprints = std::fs::read(name(&manifest["footprints"], "footprints")?).map_err(|e| format!("footprints: {e}"))?;
        // (Small: hashed whole.)
        if hex(&footprints) != manifest["footprints"]["sha256"] { return Err("footprints.bin sha256 differs from the manifest".into()); }
        let footprints: Vec<u32> = footprints.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect();
        let mut listed: Vec<&Value> = manifest["regions"].as_array().ok_or("planet.json: regions")?.iter().collect();
        listed.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let (mut regions, mut owner) = (vec![], vec![]);
        for (k, r) in listed.into_iter().enumerate() {
            let id = r["id"].as_str().ok_or("planet.json: region id")?.to_owned();
            let (s, n) = (r["footprint"]["start"].as_u64().ok_or("footprint start")? as usize, r["footprint"]["count"].as_u64().ok_or("footprint count")? as usize);
            let footprint = footprints.get(s..s + n).ok_or("footprint outside footprints.bin")?.to_vec();
            if footprint.is_empty() || footprint.windows(2).any(|w| w[0] >= w[1]) || footprint.iter().any(|&f| f as usize >= root.count(2)) {
                return Err(format!("region {id}: footprint empty, unsorted or outside the root"));
            }
            owner.extend(footprint.iter().map(|&f| (f, k)));
            let path = name(r, "region")?;
            let count = |f: &str| r[f].as_u64().ok_or_else(|| format!("region {id}: manifest {f}"));
            let (faces, added) = (u32::try_from(count("faces")?).map_err(|_| "faces")?, count("added_points")? as usize);
            let (levels, root_deviation_max_m) = levels_of(&id, r, folder)?;
            regions.push(Region { id, path, entry: r.clone(), lowest: footprint[0], footprint, faces, added, file: OnceLock::new(), open_ms: OnceLock::new(), boundary: OnceLock::new(), touched: OnceLock::new(), levels, root_deviation_max_m });
        }
        owner.sort();
        if owner.windows(2).any(|w| w[0].0 == w[1].0) { return Err("two regions claim the same root face (footprints overlap)".into()); }
        // Composed order: root runs between replaced faces; a region where its lowest stood.
        let (mut segments, mut composed, mut root_at) = (vec![], 0u64, 0u32);
        for &(r, k) in &owner {
            if r > root_at {
                segments.push(Segment::Root { composed, root: root_at, len: r - root_at });
                composed += (r - root_at) as u64;
            }
            if regions[k].lowest == r {
                let len = regions[k].faces;
                segments.push(Segment::Region { composed, region: k, len });
                composed += len as u64;
            }
            root_at = r + 1;
        }
        let n_root = root.count(2) as u32;
        if n_root > root_at {
            segments.push(Segment::Root { composed, root: root_at, len: n_root - root_at });
            composed += (n_root - root_at) as u64;
        }
        // Water bodies: the root lists them all; small, read whole.
        let mut bodies = BTreeMap::new();
        if root.capabilities & 2 != 0 {
            let s = root.section(4)?;
            for k in 0..s.count {
                let id = root.u32(s.offset + k * 16)?;
                let level = root.f64(s.offset + k * 16 + 8)?;
                if id == NONE || !level.is_finite() || bodies.insert(id, level).is_some() { return Err("invalid water body".into()); }
            }
        }
        let radius_m = header_radius(&root)?;
        // The height bound: the manifest's, root and every region (none opened for it).
        let mut max_height = bodies.values().copied().fold(root.heights.1, f64::max);
        for r in &regions {
            max_height = max_height.max(r.entry["height_max_m"].as_f64().ok_or_else(|| format!("region {}: manifest height_max_m", r.id))?);
        }
        let provenance: [String; 7] = std::array::from_fn(|i| root.strings[i].clone());
        let package = Package {
            folder: folder.to_owned(), manifest_sha256: hex(&manifest_bytes), root, regions, owner, segments, faces: composed, bodies, max_height, radius_m,
            opened_ms: start.elapsed().as_secs_f64() * 1e3, opened_at: start,
        };
        Ok(Opened { capabilities: package.root.capabilities, provenance, package })
    }

    pub(crate) fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub(crate) fn face_count(&self) -> usize {
        self.faces as usize
    }
    /// Composed points: the root's, then each region's added points (from the manifest).
    pub(crate) fn point_count(&self) -> usize {
        self.root.count(1) + self.regions.iter().map(|r| r.added).sum::<usize>()
    }

    /// A region's file, opened on first touch and held to its manifest entry.
    fn region_file(&self, k: usize) -> Result<&File, String> {
        let r = &self.regions[k];
        r.file.get_or_init(|| {
            let clock = Instant::now();
            let file = File::open(&r.path, &r.entry)?;
            if file.strings.get(8) != Some(&r.id) { return Err(format!("{}: region id in the header differs from the manifest", file.name)); }
            if file.capabilities != self.root.capabilities { return Err(format!("{}: capabilities differ from the root's", file.name)); }
            if !file.sections.contains_key(&16) || !file.sections.contains_key(&18) { return Err(format!("{}: missing root points or boundary", file.name)); }
            r.open_ms.get_or_init(|| clock.elapsed().as_secs_f64() * 1e3);
            log::info!("PGS1 package: region {} opened ({:.2} ms)", r.id, clock.elapsed().as_secs_f64() * 1e3);
            Ok(file)
        }).as_ref().map_err(Clone::clone)
    }
    /// A region's level file (`levels[i]`), opened on first touch and held to its manifest
    /// entry: the region's id, its k, the root's capabilities, kind 20 one entry per added point,
    /// each naming one of level 0's added points.
    fn level_file(&self, k: usize, i: usize) -> Result<&File, String> {
        let (r, l) = (&self.regions[k], &self.regions[k].levels[i]);
        l.file.get_or_init(|| {
            let clock = Instant::now();
            let file = File::open(&l.path, &l.entry)?;
            if file.strings.get(8) != Some(&r.id) { return Err(file.fail("region id in the header differs from the manifest")); }
            if file.strings.get(10) != Some(&l.k.to_string()) { return Err(file.fail("level in the header differs from the manifest")); }
            if file.capabilities != self.root.capabilities { return Err(file.fail("capabilities differ from the root's")); }
            for kind in [16, 18, 20] {
                if !file.sections.contains_key(&kind) { return Err(file.fail(&format!("missing section {kind}"))); }
            }
            let added = l.entry["added_points"].as_u64().ok_or_else(|| file.fail("manifest added_points"))? as usize;
            if file.count(20) != added { return Err(file.fail("kind 20 is not one entry per added point")); }
            l.open_ms.get_or_init(|| clock.elapsed().as_secs_f64() * 1e3);
            log::info!("PGS1 package: region {} level {} opened ({:.2} ms)", r.id, l.k, clock.elapsed().as_secs_f64() * 1e3);
            Ok(file)
        }).as_ref().map_err(Clone::clone)
    }
    /// Kind 20 of a level point (an added point's index among level 0's added points), held
    /// below level 0's added-point count.
    fn level_source(&self, k: usize, i: usize, added: u32) -> Result<u32, String> {
        let file = self.level_file(k, i)?;
        let s = file.u32(file.section(20)?.offset + added as usize * 4)?;
        if s as usize >= self.regions[k].added { return Err(file.fail("kind 20 names no level-0 added point")); }
        Ok(s)
    }
    fn rep_file(&self, k: usize, rep: Rep) -> Result<&File, String> {
        match rep { Rep::Root => Ok(&self.root), Rep::Fine => self.region_file(k), Rep::Level(i) => self.level_file(k, i) }
    }
    fn file(&self, region: Option<usize>) -> Result<&File, String> {
        match region { None => Ok(&self.root), Some(k) => self.region_file(k) }
    }
    fn touch(&self, k: usize) {
        self.regions[k].touched.get_or_init(|| self.opened_at.elapsed().as_secs_f64() * 1e3);
    }
    fn composed(&self, region: Option<usize>, f: u32) -> u64 {
        let at = match region { Some(k) => self.regions[k].lowest, None => f };
        let s = self.segments.partition_point(|s| match *s {
            Segment::Root { root, .. } => root <= at,
            Segment::Region { region: k, .. } => self.regions[k].lowest <= at,
        });
        match (region, self.segments[s - 1]) {
            (None, Segment::Root { composed, root, .. }) => composed + (f - root) as u64,
            (Some(_), Segment::Region { composed, .. }) => composed + f as u64,
            _ => unreachable!("composed id of a replaced face"),
        }
    }
    /// A representation's face count (the root's: its footprint faces).
    fn rep_faces(&self, k: usize, rep: Rep) -> u64 {
        let r = &self.regions[k];
        match rep {
            Rep::Root => r.footprint.len() as u64,
            Rep::Fine => r.faces as u64,
            Rep::Level(i) => r.levels[i].entry["faces"].as_u64().unwrap_or(0),
        }
    }
    /// The composed id over the mixture a tolerance names (every region in the representation it
    /// resolves to): the revision-4 rule, root faces ascending, a region's faces where its lowest
    /// footprint face stood, a footprint the root answers for kept as root faces.
    fn composed_at(&self, tolerance_m: f64, region: Option<usize>, rep: Rep, f: u32) -> u64 {
        if tolerance_m == 0.0 && rep == Rep::Fine { return self.composed(region, f); }
        // The root face where this face's run stands, and the offset within the run.
        let (at, within) = match (region, rep) {
            (None, _) | (Some(_), Rep::Root) => (f, 0),
            (Some(k), _) => (self.regions[k].lowest, f as u64),
        };
        let mut id = at as u64 + within;
        for &(root_face, k) in &self.owner {
            if root_face >= at { break; }
            let rk = self.regions[k].representation(tolerance_m);
            if rk == Rep::Root { continue; }
            id -= 1;
            if self.regions[k].lowest == root_face { id += self.rep_faces(k, rk); }
        }
        id
    }
    fn local(&self, id: u64) -> Option<(Option<usize>, u32)> {
        let s = self.segments.partition_point(|s| s.composed() <= id).checked_sub(1)?;
        match self.segments[s] {
            Segment::Root { composed, root, len } => (id < composed + len as u64).then(|| (None, root + (id - composed) as u32)),
            Segment::Region { composed, region, len } => (id < composed + len as u64).then(|| (Some(region), (id - composed) as u32)),
        }
    }
    fn region_of(&self, root_face: u32) -> Option<usize> {
        self.owner.binary_search_by_key(&root_face, |&(f, _)| f).ok().map(|i| self.owner[i].1)
    }

    pub(crate) fn query(&self, direction: DVec3) -> Result<Sample, String> {
        self.query_to(direction, 0.0).map(|(s, _)| s)
    }
    /// Answered to `tolerance_m` (see the module notes): the sample, and the owner face's
    /// corners in the representation that answered. `face` is the composed id over the mixture
    /// the tolerance names (`composed_at`); at tolerance 0, level 0's.
    pub(crate) fn query_to(&self, direction: DVec3, tolerance_m: f64) -> Result<(Sample, ([DVec3; 3], [f64; 3])), String> {
        let length = direction.length();
        if !direction.is_finite() || !length.is_finite() || length == 0.0 {
            return Err("invalid query direction".into());
        }
        if !(tolerance_m >= 0.0) { return Err("invalid query tolerance".into()); }
        let q = direction / length;
        // The whole tied set: every face, root or region, whose cell lists it and whose closed
        // chord triangle holds q; inside a footprint, the faces of the representation the
        // tolerance names (one per region, the same for every query at this tolerance).
        // (region, representation, face); ordered as composed ids, a representation's faces
        // where its region stands.
        let mut tied: Vec<(Option<usize>, Rep, u32)> = vec![];
        let mut regions = vec![];
        for f in self.root.candidates(q)? {
            if !self.root.holds(f, q)? { continue; }
            match self.region_of(f) {
                Some(k) => match self.regions[k].representation(tolerance_m) {
                    Rep::Root => tied.push((Some(k), Rep::Root, f)),
                    rep => if !regions.contains(&(k, rep)) { regions.push((k, rep)) },
                },
                None => tied.push((None, Rep::Fine, f)),
            }
        }
        for (k, rep) in regions {
            match rep {
                Rep::Level(i) => { self.regions[k].levels[i].touched.get_or_init(|| self.opened_at.elapsed().as_secs_f64() * 1e3); }
                _ => self.touch(k),
            }
            let file = self.rep_file(k, rep)?;
            for f in file.candidates(q)? {
                if file.holds(f, q)? {
                    file.check_order(f)?;
                    tied.push((Some(k), rep, f));
                }
            }
        }
        let key = |&(r, rep, f): &(Option<usize>, Rep, u32)| self.composed_at(tolerance_m, r, rep, f);
        let &(region, rep, f) = tied.iter().min_by_key(|t| key(t)).ok_or("no containing face")?;
        let file = match region { None => &self.root, Some(k) => self.rep_file(k, rep)? };
        let ([a, b, c], h) = file.corners(f)?;
        let height_m = height(q, [a, b, c], h);
        let categories = file.face(f)?.3;
        let face = self.composed_at(tolerance_m, region, rep, f);
        let water = if self.root.capabilities & 2 == 0 { Water::Unknown } else {
            let (mut wet, mut shore) = (None, false);
            for &(r, rep, g) in &tied {
                let body = match r { None => &self.root, Some(k) => self.rep_file(k, rep)? }.face(g)?.2;
                if body == NONE { shore = true } else if wet.is_none() { wet = Some(body) }
            }
            match wet {
                None => Water::Dry,
                Some(body) => {
                    let level_m = *self.bodies.get(&body).ok_or("face names a missing water body")?;
                    Water::Wet { body, level_m, depth_m: if shore { 0.0 } else { (level_m - height_m).max(0.0) } }
                }
            }
        };
        Ok((Sample { face: u32::try_from(face).map_err(|_| "composed face id beyond u32")?, height_m, water, categories: (self.root.capabilities & 32 != 0).then_some(categories) }, ([a, b, c], h)))
    }

    fn boundary(&self, k: usize) -> Result<&[(u32, u8, u32)], String> {
        let rows = self.regions[k].boundary.get_or_init(|| {
            let file = self.region_file(k)?;
            let s = file.section(18)?;
            let mut rows = Vec::with_capacity(s.count);
            for i in 0..s.count {
                let b = file.at(s.offset + i * 12, 12)?;
                if b[5..8] != [0; 3] { return Err(file.fail("boundary row padding")); }
                rows.push((u32::from_le_bytes(b[0..4].try_into().unwrap()), b[4], u32::from_le_bytes(b[8..12].try_into().unwrap())));
            }
            Ok(rows)
        });
        rows.as_deref().map_err(Clone::clone)
    }
    /// The root point ids at either end of a face's edge `e` (NONE for a region's added point).
    fn edge_root_points(&self, region: Option<usize>, f: u32, e: usize) -> Result<[u32; 2], String> {
        let file = self.file(region)?;
        let v = file.face(f)?.0;
        let ends = [v[e], v[(e + 1) % 3]];
        match region {
            None => Ok(ends),
            Some(_) => Ok([file.root_point(ends[0])?, file.root_point(ends[1])?]),
        }
    }
    /// The face across a root edge (ends as root point ids) inside region `k`, from its boundary rows
    /// whose root face across is `across`.
    fn across_in_region(&self, k: usize, across: u32, ends: [u32; 2]) -> Result<u64, String> {
        let rows = self.boundary(k)?;
        let first = rows.partition_point(|r| r.2 < across);
        for &(face, edge, a) in &rows[first..] {
            if a != across { break; }
            let e = self.edge_root_points(Some(k), face, edge as usize)?;
            if e == [ends[1], ends[0]] || e == ends {
                return Ok(self.composed(Some(k), face));
            }
        }
        Err(format!("region {}: no boundary row across root face {across}", self.regions[k].id))
    }
    fn neighbour(&self, region: Option<usize>, f: u32, e: usize, n: u32) -> Result<u32, String> {
        let id = match region {
            None => match self.region_of(n) {
                None => self.composed(None, n),
                Some(k) => self.across_in_region(k, f, self.edge_root_points(None, f, e)?)?,
            },
            Some(k) if n == NONE => {
                // Across the footprint boundary: the row names the root face across.
                let rows = self.boundary(k)?;
                let &(_, _, across) = rows.iter().find(|r| r.0 == f && r.1 as usize == e).ok_or_else(|| format!("region {}: face {f} edge {e} has no boundary row", self.regions[k].id))?;
                let ends = self.edge_root_points(region, f, e)?;
                match self.region_of(across) {
                    None => self.composed(None, across),
                    // Across is itself replaced: find our footprint face on that edge (the root
                    // face across's neighbour over the same ends), then the other region's face.
                    Some(k2) => {
                        let (v, nb, _, _) = self.root.face(across)?;
                        let mine = (0..3).find(|&i| { let m = [v[i], v[(i + 1) % 3]]; m == [ends[1], ends[0]] || m == ends })
                            .map(|i| nb[i]).ok_or("boundary edge not on the root face across")?;
                        self.across_in_region(k2, mine, ends)?
                    }
                }
            }
            Some(k) => self.composed(Some(k), n),
        };
        u32::try_from(id).map_err(|_| "composed face id beyond u32".into())
    }
    pub(crate) fn face_geometry(&self, id: u32) -> Option<FaceGeometry> {
        let (region, f) = self.local(id as u64)?;
        let file = self.file(region).map_err(|e| log::error!("PGS1 package: {e}")).ok()?;
        let (_, nb, body, _) = file.face(f).ok()?;
        let (positions, bed_heights_m) = file.corners(f).ok()?;
        let mut neighbours = [NONE; 3];
        for e in 0..3 {
            neighbours[e] = self.neighbour(region, f, e, nb[e]).map_err(|e| log::error!("PGS1 package: {e}")).ok()?;
        }
        Some(FaceGeometry { positions, bed_heights_m, neighbours, body, water_level_m: self.bodies.get(&body).copied() })
    }

    /// Gates 1 and 2 for one region's levels (heavy; a lab check, never at run time): every level
    /// point byte-identical to its level-0 point (kept points by root id, added points by kind
    /// 20), and the deviation re-measured at the overlay's vertices, level-0 points (the level's
    /// height there from its own faces) and every proper crossing of a level edge with a level-0
    /// edge, in each footprint face's plane as the writer measures it.
    pub(crate) fn verify_levels(&self, id: &str) -> Result<Value, String> {
        let k = self.regions.iter().position(|r| r.id == id).ok_or("no such region")?;
        let fine = self.region_file(k)?;
        let r_units = 1.0 / self.radius_m;
        let points = |f: &File| -> Result<Vec<(DVec3, f64, u32)>, String> {
            (0..f.count(1) as u32).map(|i| Ok({ let (p, h) = f.point(i)?; (p, h, f.root_point(i)?) })).collect()
        };
        let raw = |f: &File, i: u32| -> Result<Vec<u8>, String> { Ok(f.at(f.section(1)?.offset + i as usize * 40, 40)?.to_vec()) };
        let fp = points(fine)?;
        let fine_added: Vec<u32> = (0..fp.len() as u32).filter(|&i| fp[i as usize].2 == NONE).collect();
        let by_root: HashMap<u32, u32> = fp.iter().enumerate().filter(|p| p.1.2 != NONE).map(|(i, p)| (p.2, i as u32)).collect();
        let edges = |f: &File| -> Result<Vec<(u32, u32)>, String> {
            let mut e = Vec::with_capacity(f.count(2) * 3);
            for t in 0..f.count(2) as u32 {
                let v = f.face(t)?.0;
                for j in 0..3 { let (a, b) = (v[j], v[(j + 1) % 3]); e.push((a.min(b), a.max(b))); }
            }
            e.sort_unstable();
            e.dedup();
            Ok(e)
        };
        let fe = edges(fine)?;
        let mut out = vec![];
        for (i, level) in self.regions[k].levels.iter().enumerate() {
            let file = self.level_file(k, i)?;
            let lp = points(file)?;
            // Level point → level-0 point.
            let mut l2f = Vec::with_capacity(lp.len());
            let mut added = 0u32;
            for (j, p) in lp.iter().enumerate() {
                let f = if p.2 == NONE {
                    let s = self.level_source(k, i, added)?;
                    added += 1;
                    fine_added[s as usize]
                } else {
                    *by_root.get(&p.2).ok_or_else(|| file.fail(&format!("point {j}: root point {} not in level 0", p.2)))?
                };
                if raw(file, j as u32)? != raw(fine, f)? { return Err(file.fail(&format!("point {j} is not byte-identical to level-0 point {f}"))); }
                l2f.push(f);
            }
            let (mut worst, mut sq, mut n) = (0f64, 0f64, 0u64);
            let mut add = |d: f64| { worst = worst.max(d); sq += d * d; n += 1; };
            // Crossings, per footprint face plane.
            let le = edges(file)?;
            let cell = level.entry["spacing_m"].as_f64().unwrap_or(1000.0) * r_units;
            let (mut cross_worst, mut cross_n) = (0f64, 0u64);
            let (mut point_worst, mut points_n) = (0f64, 0u64);
            for &rf in &self.regions[k].footprint {
                let ([a, b, c], _) = self.root.corners(rf)?;
                // (Divided by the norm, as the writer does: the same rounding.)
                let e1 = (b - a) / (b - a).length();
                let n0 = (b - a).cross(c - a);
                let mut nrm = n0 / n0.length();
                if nrm.dot(a + b + c) < 0.0 { nrm = -nrm; }
                let e2 = nrm.cross(e1);
                let inv = glam::DMat3::from_cols(a, b, c).inverse();
                let on = |p: DVec3| { let w = inv * p; (w.x + w.y + w.z - 1.0).abs() <= 1e-9 && w.min_element() >= -1e-12 };
                let xy = |p: DVec3| { let d = p - a; glam::DVec2::new(d.dot(e1), d.dot(e2)) };
                // Level-0 points on this face against the level's own faces (a point on an edge
                // two footprint faces share counts on each, as the writer measures).
                for &(p, h, _) in fp.iter().filter(|p| on(p.0)) {
                    let q = p.normalize();
                    let mut best = None;
                    for f in file.candidates(q)? {
                        if file.holds(f, q)? { best = Some(f); break; }
                    }
                    let f = best.ok_or_else(|| file.fail("a level-0 point outside the level's faces"))?;
                    let (c, hc) = file.corners(f)?;
                    let d = (height(q, c, hc) - h).abs();
                    point_worst = point_worst.max(d);
                    points_n += 1;
                    add(d);
                }
                let lon: Vec<bool> = lp.iter().map(|p| on(p.0)).collect();
                let fon: Vec<bool> = fp.iter().map(|p| on(p.0)).collect();
                let lxy: Vec<glam::DVec2> = lp.iter().map(|p| xy(p.0)).collect();
                let fxy: Vec<glam::DVec2> = fp.iter().map(|p| xy(p.0)).collect();
                let key = |x: f64, y: f64| ((x / cell).floor() as i64, (y / cell).floor() as i64);
                let mut grid: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
                let face_le: Vec<(u32, u32)> = le.iter().copied().filter(|&(p, q)| lon[p as usize] && lon[q as usize]).collect();
                for (ei, &(p, q)) in face_le.iter().enumerate() {
                    let (u, v) = (lxy[p as usize], lxy[q as usize]);
                    let m = 1e-6 * cell;
                    let (lo, hi) = (key(u.x.min(v.x) - m, u.y.min(v.y) - m), key(u.x.max(v.x) + m, u.y.max(v.y) + m));
                    for gi in lo.0..=hi.0 { for gj in lo.1..=hi.1 { grid.entry((gi, gj)).or_default().push(ei as u32); } }
                }
                let mut cand = vec![];
                for &(fa, fb) in fe.iter().filter(|&&(p, q)| fon[p as usize] && fon[q as usize]) {
                    let (u, v) = (fxy[fa as usize], fxy[fb as usize]);
                    let m = 1e-6 * cell;
                    let (lo, hi) = (key(u.x.min(v.x) - m, u.y.min(v.y) - m), key(u.x.max(v.x) + m, u.y.max(v.y) + m));
                    cand.clear();
                    for gi in lo.0..=hi.0 { for gj in lo.1..=hi.1 { if let Some(c) = grid.get(&(gi, gj)) { cand.extend_from_slice(c); } } }
                    cand.sort_unstable();
                    cand.dedup();
                    for &ei in &cand {
                        let (la, lb) = face_le[ei as usize];
                        let (fla, flb) = (l2f[la as usize], l2f[lb as usize]);
                        if fla == fa || fla == fb || flb == fa || flb == fb { continue; }
                        let (a0, a1, b0, b1) = (lxy[la as usize], lxy[lb as usize], u, v);
                        let (da, db) = (a1 - a0, b1 - b0);
                        let den = da.x * db.y - da.y * db.x;
                        if den.abs() == 0.0 { continue; }
                        let d0 = b0 - a0;
                        let t = (d0.x * db.y - d0.y * db.x) / den;
                        let s = (d0.x * da.y - d0.y * da.x) / den;
                        let eps = 1e-12;
                        if !(t > eps && t < 1.0 - eps && s > eps && s < 1.0 - eps) { continue; }
                        let (hla, hlb) = (lp[la as usize].1, lp[lb as usize].1);
                        let (hfa, hfb) = (fp[fa as usize].1, fp[fb as usize].1);
                        let d = ((hla + (hlb - hla) * t) - (hfa + (hfb - hfa) * s)).abs();
                        cross_worst = cross_worst.max(d);
                        cross_n += 1;
                        add(d);
                    }
                }
            }
            let rms = (sq / n.max(1) as f64).sqrt();
            let declared = (level.deviation_max_m, level.entry["deviation_rms_m"].as_f64().unwrap_or(f64::NAN), level.entry["deviation_points"].as_u64());
            // A declared maximum may stand above the measured one (held up to the level before's),
            // never below it.
            let measured_declared = level.entry["deviation_measured_max_m"].as_f64().unwrap_or(declared.0);
            if declared.0 + 1e-6 < worst { return Err(file.fail(&format!("declared deviation {} m is below the measured {worst} m", declared.0))); }
            out.push(json!({"level":level.k,"file":file.name,"points":lp.len(),"points_byte_identical":true,
                "deviation_at_level0_points":{"max_m":point_worst,"count":points_n},
                "deviation_at_crossings":{"max_m":cross_worst,"count":cross_n},
                "measured":{"max_m":worst,"rms_m":rms,"count":n},
                "declared":{"max_m":declared.0,"measured_max_m":measured_declared,"rms_m":declared.1,"count":declared.2},
                "max_reproduced_1e-6":(worst - measured_declared).abs() <= 1e-6,"rms_reproduced_1e-6":(rms - declared.1).abs() <= 1e-6,
                "declared_not_below_measured":declared.0 + 1e-6 >= worst}));
        }
        Ok(json!({"region":id,"levels":out}))
    }

    /// Every opened region file: (region, representation, file).
    fn opened_files(&self) -> impl Iterator<Item = (usize, Rep, &File)> {
        self.regions.iter().enumerate().flat_map(|(k, r)| {
            let fine = r.file.get().and_then(|f| f.as_ref().ok()).map(|f| (k, Rep::Fine, f));
            fine.into_iter().chain(r.levels.iter().enumerate().filter_map(move |(i, l)| l.file.get().and_then(|f| f.as_ref().ok()).map(|f| (k, Rep::Level(i), f))))
        })
    }
    /// Residency: release every region file (level 0 or a level) untouched for `idle_ms`, then,
    /// while the region files' resident bytes exceed `budget`, the least lately touched. The root
    /// stays. A released file reads and checks its blocks again when next touched (its pages are
    /// the file's, never changed: a read racing the release faults them back unaltered).
    pub(crate) fn trim(&self, idle_ms: f64, budget: u64) -> Value {
        let mut files: Vec<(f64, u64, usize, Rep, &File)> = self.opened_files().map(|(k, rep, f)| (f.idle_ms(), f.resident().unwrap_or(0), k, rep, f)).collect();
        files.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        let before = total;
        let mut released = vec![];
        for &(idle, resident, _, _, f) in &files {
            if resident == 0 { continue; }
            if idle >= idle_ms || total > budget {
                f.release();
                total -= resident;
                released.push(json!({"file":f.name,"resident_bytes":resident,"idle_ms":idle}));
            }
        }
        json!({"budget_bytes":budget,"idle_ms":idle_ms,"resident_before":before,"resident_after_estimate":total,"released":released})
    }

    /// Per file: bytes read (header, table, checked blocks), when its first block was read.
    pub(crate) fn report(&self) -> Value {
        json!({"format":"pgs2-package-reads/1","folder":self.folder,"manifest_sha256":self.manifest_sha256,"open_ms":self.opened_ms,
            "root":self.root.report(),
            "regions":self.regions.iter().map(|r| {
                let mut v = match r.file.get() {
                    Some(Ok(f)) => f.report(),
                    Some(Err(e)) => json!({"refused":e}),
                    None => json!({"file":r.path.file_name().map(|n| n.to_string_lossy().into_owned()),"opened":false,"bytes_read":0}),
                };
                v["open_ms"] = json!(r.open_ms.get());
                v["id"] = json!(r.id);
                v["footprint"] = json!(r.footprint);
                v["path"] = json!(r.path);
                v["touched"] = json!(r.touched.get().is_some());
                v["first_query_ms_after_package_open"] = json!(r.touched.get());
                v["root_deviation_max_m"] = json!(r.root_deviation_max_m.is_finite().then_some(r.root_deviation_max_m));
                v["levels"] = json!(r.levels.iter().map(|l| {
                    let mut w = match l.file.get() {
                        Some(Ok(f)) => f.report(),
                        Some(Err(e)) => json!({"refused":e}),
                        None => json!({"file":l.path.file_name().map(|n| n.to_string_lossy().into_owned()),"opened":false,"bytes_read":0}),
                    };
                    w["level"] = json!(l.k);
                    w["deviation_max_m"] = json!(l.deviation_max_m);
                    w["open_ms"] = json!(l.open_ms.get());
                    w["first_query_ms_after_package_open"] = json!(l.touched.get());
                    w
                }).collect::<Vec<_>>());
                v
            }).collect::<Vec<_>>()})
    }
}

fn header_radius(root: &File) -> Result<f64, String> {
    let r = f64::from_le_bytes(root.map[24..32].try_into().unwrap());
    if !r.is_finite() || r <= 0.0 { return Err("root radius".into()); }
    Ok(r)
}

/// Chord-plane barycentric height, as the canonical reader computes it.
pub(super) fn height(q: DVec3, [a, b, c]: [DVec3; 3], h: [f64; 3]) -> f64 {
    let normal = (b - a).cross(c - a);
    let plane = normal.dot(a);
    let p = q * (plane / normal.dot(q));
    let den = normal.length_squared();
    let v = (p - a).cross(c - a).dot(normal) / den;
    let w = (b - a).cross(p - a).dot(normal) / den;
    (1.0 - v - w) * h[0] + v * h[1] + w * h[2]
}
