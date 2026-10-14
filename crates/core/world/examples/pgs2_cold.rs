//! PGS1 revision 4 package, read the way the engine would: files mapped, the cell index as the
//! load path, no tree and no section read whole. Measures a cold open and first query (the page
//! cache emptied of the package's files first), the pages touched, then answers every v8 fixture.
//! cargo run -p universe-world --release --example pgs2_cold -- PACKAGE_DIR FIXTURES.json OUT.json [MONOLITHIC.pgs]
//! cargo run -p universe-world --release --example pgs2_cold -- --goldens PGS2_FIXTURE_DIR OUT.json
use glam::DVec3;
use memmap2::Mmap;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    fs::File,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    time::Instant,
};

const NONE: u32 = u32::MAX;
const TIE: f64 = 1e-12;
const PAGE: usize = 4096;

fn hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// A mapped file that remembers which pages the reader asked for and, given the manifest's
/// opening record, checks each 64 KiB block against its kind-19 hash on first touch.
struct Map {
    name: String,
    map: Mmap,
    touched: RefCell<BTreeSet<usize>>,
    opening: Option<Opening>,
    verified: RefCell<BTreeSet<usize>>,
}
struct Opening {
    prefix: usize,
    block: usize,
    hashes: usize,
    blocks: usize,
}
impl Map {
    fn open(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let map = unsafe { Mmap::map(&file) }.map_err(|e| e.to_string())?;
        // Random access: no readahead beyond the page asked for, so touched pages are what was read.
        let _ = map.advise(memmap2::Advice::Random);
        Ok(Self { name: path.file_name().unwrap().to_string_lossy().into(), map, touched: RefCell::default(), opening: None, verified: RefCell::default() })
    }
    fn raw(&self, offset: usize, len: usize) -> Result<&[u8], String> {
        let bytes = self.map.get(offset..offset.checked_add(len).ok_or("overflow")?).ok_or_else(|| format!("{}: read outside file", self.name))?;
        if len > 0 {
            self.touched.borrow_mut().extend(offset / PAGE..=(offset + len - 1) / PAGE);
        }
        Ok(bytes)
    }
    /// The manifest's opening record: header and table hash, the block-hash section's hash.
    fn verify_opening(&mut self, o: &Value) -> Result<(), String> {
        let n = |k: &str| o[k].as_u64().map(|v| v as usize).ok_or_else(|| format!("{}: opening.{k}", self.name));
        let (prefix, block, hashes, blocks) = (n("prefix_bytes")?, n("block_bytes")?, n("block_hashes_offset")?, n("blocks")?);
        if hashes < prefix || blocks != (hashes - prefix).div_ceil(block) { return Err(format!("{}: opening blocks", self.name)); }
        if hex(self.raw(0, prefix)?) != o["prefix_sha256"] { return Err(format!("{}: header/table hash", self.name)); }
        if hex(self.raw(hashes, blocks * 32)?) != o["block_hashes_sha256"] { return Err(format!("{}: block hashes hash", self.name)); }
        self.opening = Some(Opening { prefix, block, hashes, blocks });
        Ok(())
    }
    fn at(&self, offset: usize, len: usize) -> Result<&[u8], String> {
        if let Some(o) = &self.opening {
            let end = (offset + len).min(o.hashes);
            if len > 0 && end > o.prefix.max(offset) {
                for k in (offset.max(o.prefix) - o.prefix) / o.block..=(end - 1 - o.prefix) / o.block {
                    if k < o.blocks && self.verified.borrow_mut().insert(k) {
                        let start = o.prefix + k * o.block;
                        // One read for the block, not sixteen page faults in turn.
                        let _ = self.map.advise_range(memmap2::Advice::WillNeed, start, (start + o.block).min(o.hashes) - start);
                        let bytes = self.raw(start, (start + o.block).min(o.hashes) - start)?;
                        let digest = Sha256::digest(bytes);
                        if digest.as_slice() != self.raw(o.hashes + k * 32, 32)? {
                            return Err(format!("{}: block {k} hash", self.name));
                        }
                    }
                }
            }
        }
        self.raw(offset, len)
    }
    fn u32(&self, offset: usize) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.at(offset, 4)?.try_into().unwrap()))
    }
    fn u64(&self, offset: usize) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.at(offset, 8)?.try_into().unwrap()))
    }
    fn f64(&self, offset: usize) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.at(offset, 8)?.try_into().unwrap()))
    }
    /// Pages of this file the kernel holds now (mincore), whoever asked for them.
    fn resident(&self) -> usize {
        let pages = self.map.len().div_ceil(PAGE);
        let mut v = vec![0u8; pages];
        let r = unsafe { libc::mincore(self.map.as_ptr() as *mut _, self.map.len(), v.as_mut_ptr()) };
        if r != 0 { return usize::MAX; }
        v.iter().filter(|&&b| b & 1 != 0).count()
    }
}

/// Drop a file's pages from the page cache (clean pages, no privilege needed); returns pages left.
fn evict(path: &Path) -> Result<usize, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    unsafe { libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) };
    let len = file.metadata().map_err(|e| e.to_string())?.len() as usize;
    if len == 0 { return Ok(0); }
    let map = unsafe { Mmap::map(&file) }.map_err(|e| e.to_string())?;
    let mut v = vec![0u8; len.div_ceil(PAGE)];
    unsafe { libc::mincore(map.as_ptr() as *mut _, len, v.as_mut_ptr()) };
    Ok(v.iter().filter(|&&b| b & 1 != 0).count())
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

/// One PGS1 file (version 1 or 2): header, strings and section table read; records on demand.
struct Pgs {
    map: Map,
    capabilities: u32,
    strings: Vec<String>,
    sections: HashMap<u32, Section>,
    windows: Vec<Window>,
}
impl Pgs {
    fn open(path: &Path, opening: &Value) -> Result<Self, String> {
        let mut map = Map::open(path)?;
        map.verify_opening(opening)?;
        if map.at(0, 4)? != b"PGS1" { return Err("not PGS1".into()); }
        let version = u16::from_le_bytes(map.at(4, 2)?.try_into().unwrap());
        let capabilities = map.u32(8)?;
        let want = match version { 1 => 7, 2 => 10, _ => return Err(format!("version {version}")) };
        let count = map.u32(32)? as usize;
        if count != want { return Err("header string count".into()); }
        let (mut at, mut strings) = (36, vec![]);
        for _ in 0..count {
            let n = map.u32(at)? as usize;
            strings.push(String::from_utf8(map.at(at + 4, n)?.to_vec()).map_err(|e| e.to_string())?);
            at += 4 + n;
        }
        at = at.div_ceil(8) * 8;
        let entries = map.u32(at)? as usize;
        let mut sections = HashMap::new();
        for k in 0..entries {
            let e = at + 8 + k * 72;
            let (kind, stride, flags) = (map.u32(e)?, map.u32(e + 12)?, map.u32(e + 8)?);
            let (offset, length, count) = (map.u64(e + 16)? as usize, map.u64(e + 24)? as usize, map.u64(e + 32)? as usize);
            if offset % 8 != 0 || offset + length > map.map.len() || (stride != 0 && length != count * stride as usize) {
                return Err(format!("section {kind} out of shape"));
            }
            let known = matches!(kind, 1..=13 | 15..=19);
            if flags & 1 != 0 && !known { return Err(format!("required section {kind} unknown")); }
            sections.insert(kind, Section { offset, count });
        }
        let w = *sections.get(&17).ok_or("no windows (kind 17)")?;
        let mut windows = vec![];
        for k in 0..w.count {
            let o = w.offset + k * 32;
            let r = |f: usize| map.u32(o + 4 * f);
            windows.push(Window { face: r(0)?, n: r(1)?, i0: r(2)?, j0: r(3)?, ni: r(4)?, nj: r(5)?, start: r(6)? });
        }
        Ok(Self { map, capabilities, strings, sections, windows })
    }
    fn section(&self, kind: u32) -> Result<Section, String> {
        self.sections.get(&kind).copied().ok_or_else(|| format!("{}: no section {kind}", self.map.name))
    }
    fn point(&self, i: u32) -> Result<(DVec3, f64), String> {
        let o = self.section(1)?.offset + i as usize * 40;
        Ok((DVec3::new(self.map.f64(o)?, self.map.f64(o + 8)?, self.map.f64(o + 16)?), self.map.f64(o + 24)?))
    }
    /// Vertices, body, (rock, pattern).
    fn face(&self, i: u32) -> Result<([u32; 3], u32, (u16, u16)), String> {
        let o = self.section(2)?.offset + i as usize * 32;
        let v = [self.map.u32(o)?, self.map.u32(o + 4)?, self.map.u32(o + 8)?];
        let c = self.map.u32(o + 28)?;
        Ok((v, self.map.u32(o + 24)?, (c as u16, (c >> 16) as u16)))
    }
    fn level(&self, body: u32) -> Result<f64, String> {
        let s = self.section(4)?;
        for k in 0..s.count {
            if self.map.u32(s.offset + k * 16)? == body { return self.map.f64(s.offset + k * 16 + 8); }
        }
        Err(format!("water body {body} missing"))
    }
    /// The faces of `q`'s cell (empty outside every window): every face that may hold q.
    fn candidates(&self, q: DVec3) -> Result<Vec<u32>, String> {
        let face = cube_face(q);
        let Some(w) = self.windows.iter().find(|w| w.face == face) else { return Ok(vec![]) };
        let (_, i, j) = cube_cell(q, w.n);
        if i < w.i0 || j < w.j0 || i >= w.i0 + w.ni || j >= w.j0 + w.nj { return Ok(vec![]); }
        let c = self.section(11)?.offset + (w.start + (j - w.j0) * w.ni + (i - w.i0)) as usize * 8;
        let (start, count) = (self.map.u32(c)? as usize, self.map.u32(c + 4)? as usize);
        let cf = self.section(12)?.offset;
        let bytes = self.map.at(cf + start * 4, count * 4)?;
        Ok(bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).collect())
    }
    /// As the canonical reader: chord-plane edges, tie 1e-12.
    fn geometry(&self, f: u32) -> Result<([DVec3; 3], [f64; 3], DVec3, f64), String> {
        let (v, _, _) = self.face(f)?;
        let [(a, ha), (c, hc), (d, hd)] = [self.point(v[0])?, self.point(v[1])?, self.point(v[2])?];
        let normal = (c - a).cross(d - a);
        let edges = [a.cross(c - a), c.cross(d - c), d.cross(a - d)].map(DVec3::normalize);
        let _ = (ha, hc, hd);
        Ok((edges, [ha, hc, hd], normal, normal.dot(a)))
    }
    /// A point's composed key, in order: a kept point by its root id, then added points in stored
    /// order (their rank among added points is monotone in the local id).
    fn key(&self, p: u32) -> Result<(u8, u32), String> {
        let root = self.map.u32(self.section(16)?.offset + p as usize * 4)?;
        Ok(if root == NONE { (1, p) } else { (0, root) })
    }
    /// The canonical order's key: the cell curve of the face's centroid (unnormalised mean of its
    /// stored points, on this file's grid), then its vertex triple under composed keys, rotated to
    /// start at the least.
    fn face_key(&self, f: u32) -> Result<(u64, [(u8, u32); 3]), String> {
        let v = self.face(f)?.0;
        let [a, b, c] = [self.point(v[0])?.0, self.point(v[1])?.0, self.point(v[2])?.0];
        let n = self.windows.first().ok_or("no windows")?.n;
        let k = [self.key(v[0])?, self.key(v[1])?, self.key(v[2])?];
        let r = (0..3).min_by_key(|&i| k[i]).unwrap();
        Ok((curve((a + b + c) / 3., n), [k[r], k[(r + 1) % 3], k[(r + 2) % 3]]))
    }
    /// The writer's rule, checked on touch: a region's faces stored in canonical (composed) order.
    fn check_order(&self, f: u32) -> Result<(), String> {
        let n = self.section(2)?.count as u32;
        let here = self.face_key(f)?;
        if (f > 0 && self.face_key(f - 1)? > here) || (f + 1 < n && self.face_key(f + 1)? < here) {
            return Err(format!("{}: region faces not in canonical order (face {f})", self.map.name));
        }
        Ok(())
    }
    fn holds(&self, f: u32, q: DVec3) -> Result<bool, String> {
        Ok(self.geometry(f)?.0.iter().all(|e| q.dot(*e) >= -TIE))
    }
    fn height(&self, f: u32, q: DVec3) -> Result<f64, String> {
        let (v, _, _) = self.face(f)?;
        let [a, b, c] = [self.point(v[0])?.0, self.point(v[1])?.0, self.point(v[2])?.0];
        let (_, h, normal, plane) = self.geometry(f)?;
        let p = q * (plane / normal.dot(q));
        let den = normal.length_squared();
        let v = (p - a).cross(c - a).dot(normal) / den;
        let w = (b - a).cross(p - a).dot(normal) / den;
        Ok((1.0 - v - w) * h[0] + v * h[1] + w * h[2])
    }
}

/// Cube face of a direction: the largest absolute coordinate, ties to the earlier of x, y, z;
/// faces +X, -X, +Y, -Y, +Z, -Z.
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
        x = (x | x << 16) & 0x0000FFFF0000FFFF;
        x = (x | x << 8) & 0x00FF00FF00FF00FF;
        x = (x | x << 4) & 0x0F0F0F0F0F0F0F0F;
        x = (x | x << 2) & 0x3333333333333333;
        (x | x << 1) & 0x5555555555555555
    };
    let (face, i, j) = cube_cell(q, n);
    ((face as u64) << 62) + (spread(i) | spread(j) << 1)
}

struct Region {
    lowest: u32,
    faces: u32,
    id: String,
    file: PathBuf,
    opening: Value,
    pgs: Option<Pgs>,
}
/// A package: root and manifest open, regions opened on first touch.
struct Package {
    root: Pgs,
    footprints: Map,
    regions: Vec<Region>,
    /// (root face, region): every footprint entry, sorted by root face.
    owner: Vec<(u32, usize)>,
    files: Vec<PathBuf>,
}
struct Answer {
    face: u64,
    owner: (Option<usize>, u32),
    tied: Vec<(Option<usize>, u32)>,
    height_m: f64,
    water: Value,
    categories: (u16, u16),
}
impl Package {
    fn open(dir: &Path) -> Result<Self, String> {
        let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("planet.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if manifest["format"] != "planet-graph-surface/2" { return Err("manifest format".into()); }
        let root = Pgs::open(&dir.join(manifest["root"]["file"].as_str().ok_or("root file")?), &manifest["root"]["opening"])?;
        let footprints = Map::open(&dir.join(manifest["footprints"]["file"].as_str().ok_or("footprints")?))?;
        // (Small: hashed whole. A planet of regions would want it under block hashes too.)
        if hex(footprints.raw(0, footprints.map.len())?) != manifest["footprints"]["sha256"] { return Err("footprints hash".into()); }
        let mut regions = vec![];
        let mut owner = vec![];
        let mut listed: Vec<&Value> = manifest["regions"].as_array().ok_or("regions")?.iter().collect();
        listed.sort_by_key(|r| r["id"].as_str().unwrap_or("").to_owned());
        for (k, r) in listed.into_iter().enumerate() {
            let (start, count) = (r["footprint"]["start"].as_u64().ok_or("start")? as usize, r["footprint"]["count"].as_u64().ok_or("count")? as usize);
            let mut lowest = NONE;
            for e in 0..count {
                let f = footprints.u32((start + e) * 4)?;
                lowest = lowest.min(f);
                owner.push((f, k));
            }
            regions.push(Region { lowest, faces: 0, id: r["id"].as_str().unwrap_or("").to_owned(), file: dir.join(r["file"].as_str().ok_or("region file")?), opening: r["opening"].clone(), pgs: None });
        }
        owner.sort();
        if owner.windows(2).any(|w| w[0].0 == w[1].0) { return Err("footprints overlap".into()); }
        let files = [dir.join("planet.json"), dir.join(manifest["root"]["file"].as_str().unwrap()), dir.join(manifest["footprints"]["file"].as_str().unwrap())]
            .into_iter().chain(regions.iter().map(|r| r.file.clone())).collect();
        Ok(Self { root, footprints, regions, owner, files })
    }
    fn region(&mut self, k: usize) -> Result<&Pgs, String> {
        if self.regions[k].pgs.is_none() {
            let pgs = Pgs::open(&self.regions[k].file, &self.regions[k].opening)?;
            if pgs.strings.get(8) != Some(&self.regions[k].id) {
                return Err(format!("{}: region id in the header differs from the manifest", pgs.map.name));
            }
            self.regions[k].faces = pgs.section(2)?.count as u32;
            self.regions[k].pgs = Some(pgs);
        }
        Ok(self.regions[k].pgs.as_ref().unwrap())
    }
    fn replaced_below(&self, f: u32) -> u64 {
        self.owner.partition_point(|&(r, _)| r < f) as u64
    }
    /// Composed face id: root faces minus footprints, each region's faces where its lowest
    /// footprint face stood, in the region file's own order (assumed canonical).
    fn composed(&mut self, file: Option<usize>, f: u32) -> Result<u64, String> {
        let at = match file { None => f, Some(k) => self.regions[k].lowest };
        let mut id = at as u64 - self.replaced_below(at);
        for k in 0..self.regions.len() {
            if self.regions[k].lowest < at {
                self.region(k)?;
                id += self.regions[k].faces as u64;
            }
        }
        Ok(id + if file.is_some() { f as u64 } else { 0 })
    }
    fn query(&mut self, direction: DVec3) -> Result<Answer, String> {
        // As the canonical reader: divided by the length (normalize() multiplies by its reciprocal).
        let q = direction / direction.length();
        // The whole tied set: every face, in root or region, whose closed chord triangle holds q.
        let mut tied: Vec<(Option<usize>, u32)> = vec![];
        let mut touched_regions = BTreeSet::new();
        for f in self.root.candidates(q)? {
            if !self.root.holds(f, q)? { continue; }
            match self.owner.binary_search_by_key(&f, |&(r, _)| r) {
                Ok(k) => { touched_regions.insert(self.owner[k].1); }
                Err(_) => tied.push((None, f)),
            }
        }
        for k in touched_regions {
            let region = self.region(k)?;
            for f in region.candidates(q)? {
                if region.holds(f, q)? {
                    region.check_order(f)?;
                    tied.push((Some(k), f));
                }
            }
        }
        let mut ids = vec![];
        for &(file, f) in &tied {
            ids.push((self.composed(file, f)?, file, f));
        }
        let &(face, file, f) = ids.iter().min().ok_or("no containing face")?;
        let owner = self.file(file);
        let height_m = owner.height(f, q)?;
        let categories = owner.face(f)?.2;
        let water = if self.root.capabilities & 2 == 0 { json!({"state":"unknown"}) } else {
            let mut wet = None;
            let mut shore = false;
            for &(file, g) in &tied {
                let body = self.file(file).face(g)?.1;
                if body == NONE { shore = true } else if wet.is_none() { wet = Some(body) }
            }
            match wet {
                None => json!({"state":"dry"}),
                Some(body) => {
                    let level = self.root.level(body)?;
                    json!({"state":"wet","body":body,"level_m":level,"depth_m":if shore {0.} else {(level - height_m).max(0.)}})
                }
            }
        };
        Ok(Answer { face, owner: (file, f), tied, height_m, water, categories })
    }
    fn file(&self, file: Option<usize>) -> &Pgs {
        match file { None => &self.root, Some(k) => self.regions[k].pgs.as_ref().unwrap() }
    }
    /// Blocks checked so far, by the section they start in.
    fn blocks(&self) -> Value {
        let mut out = serde_json::Map::new();
        for p in [Some(&self.root)].into_iter().chain(self.regions.iter().map(|r| r.pgs.as_ref())).flatten() {
            let Some(o) = &p.map.opening else { continue };
            let mut by: std::collections::BTreeMap<u32, usize> = Default::default();
            for &k in p.map.verified.borrow().iter() {
                let at = o.prefix + k * o.block;
                let kind = p.sections.iter().filter(|(_, s)| s.offset <= at).max_by_key(|(_, s)| s.offset).map(|(k, _)| *k).unwrap_or(0);
                *by.entry(kind).or_default() += 1;
            }
            out.insert(p.map.name.clone(), json!(by.iter().map(|(k, n)| (k.to_string(), json!(*n))).collect::<serde_json::Map<_, _>>()));
        }
        Value::Object(out)
    }
    fn maps(&self) -> Vec<&Map> {
        [&self.root.map, &self.footprints].into_iter().chain(self.regions.iter().filter_map(|r| r.pgs.as_ref().map(|p| &p.map))).collect()
    }
    fn touched(&self) -> Value {
        Value::Array(self.maps().iter().map(|m| {
            let pages = m.touched.borrow().len();
            json!({"file":m.name,"bytes":m.map.len(),"pages_read":pages,"bytes_in_pages_read":pages*PAGE,"pages_resident_after":m.resident()})
        }).collect())
    }
}

fn cold(dir: &Path, q: DVec3, label: &str) -> Result<Value, String> {
    let files: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|e| e.to_string())?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    let left: usize = files.iter().map(|p| evict(p)).collect::<Result<Vec<_>, _>>()?.iter().sum();
    let start = Instant::now();
    let mut package = Package::open(dir)?;
    let opened = start.elapsed().as_secs_f64() * 1e3;
    let answer = package.query(q)?;
    let ms = start.elapsed().as_secs_f64() * 1e3;
    let touched = package.touched();
    let read: usize = package.maps().iter().map(|m| m.touched.borrow().len() * PAGE).sum();
    Ok(json!({"query":label,"direction":q.to_array(),"pages_resident_before":left,"open_ms":opened,"open_and_first_query_ms":ms,
        "bytes_in_pages_read":read,"files":touched,"blocks_checked_by_section":package.blocks(),"answer":{"composed_face":answer.face,"height_m":answer.height_m,"water":answer.water,"rock":answer.categories.0,"pattern":answer.categories.1},
        "header_strings":package.root.strings, "manifest_files":package.files}))
}

/// The planets lab's revision-4 goldens: every query (owner, tie set, composed id, height,
/// categories), and each refused package refused.
fn goldens(root: &Path, out: &str) -> Result<(), Box<dyn std::error::Error>> {
    let index: Value = serde_json::from_slice(&std::fs::read(root.join("fixtures.json"))?)?;
    let queries: Value = serde_json::from_slice(&std::fs::read(root.join("queries.json"))?)?;
    let mut package = Package::open(&root.join(queries["package"].as_str().ok_or("package")?))?;
    let tolerance = queries["height_tolerance_m"].as_f64().ok_or("tolerance")?;
    let (mut pass, mut bit, mut worst, mut ties, mut failures) = (0, 0, 0f64, 0, vec![]);
    let all = queries["queries"].as_array().ok_or("queries")?;
    for q in all {
        let d = DVec3::from_slice(&q["direction"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>());
        let r = package.query(d)?;
        let name = |file: Option<usize>| match file { None => package.root.map.name.clone(), Some(k) => package.regions[k].pgs.as_ref().unwrap().map.name.clone() };
        let mut got: Vec<(String, u64)> = r.tied.iter().map(|&(file, f)| (name(file), f as u64)).collect();
        let mut want: Vec<(String, u64)> = q["candidates"].as_array().unwrap().iter().map(|c| (c["file"].as_str().unwrap().to_owned(), c["face"].as_u64().unwrap())).collect();
        got.sort();
        want.sort();
        let h = q["height_m"].as_f64().unwrap();
        worst = worst.max((r.height_m - h).abs());
        bit += (r.height_m.to_bits() == h.to_bits()) as usize;
        ties += (want.len() > 1) as usize;
        let owner = (name(r.owner.0), r.owner.1 as u64) == (q["owner"]["file"].as_str().unwrap().to_owned(), q["owner"]["face"].as_u64().unwrap());
        let ok = owner && got == want && q["composed_face"].as_u64() == Some(r.face) && (r.height_m - h).abs() <= tolerance
            && q["categories"]["rock"].as_u64() == Some(r.categories.0 as u64) && q["categories"]["pattern"].as_u64() == Some(r.categories.1 as u64);
        if ok { pass += 1 } else if failures.len() < 10 {
            failures.push(json!({"query":q,"got":{"composed_face":r.face,"owner":[name(r.owner.0),r.owner.1],"tied":got,"height_m":r.height_m}}));
        }
    }
    let mut refused = vec![];
    for case in index["refused"].as_array().ok_or("refused")? {
        let dir = root.join(case["dir"].as_str().unwrap());
        // Open, then ask every golden direction: a fault found on touch counts.
        let outcome = Package::open(&dir).and_then(|mut p| {
            for q in all {
                let d = DVec3::from_slice(&q["direction"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>());
                p.query(d)?;
            }
            Ok(())
        });
        refused.push(json!({"dir":case["dir"],"expect":case["expect"],"refused":outcome.is_err(),"reason":outcome.err()}));
    }
    let report = json!({"format":"pgs2-goldens-read/1","fixtures":root,"queries":all.len(),"passed":pass,"ties":ties,"height_bit_identical":bit,"height_max_abs_diff_m":worst,"failures":failures,"refused":refused});
    std::fs::write(out, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string(&json!({"queries":all.len(),"passed":pass,"ties":ties,"bit":bit,"worst":worst,"refused":refused.iter().map(|r| json!([r["dir"],r["refused"],r["reason"]])).collect::<Vec<_>>()}))?);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, root, out] = &a[..] && flag == "--goldens" {
        return goldens(Path::new(root), out);
    }
    let (dir, fixtures, out) = match &a[..] { [d, f, o] | [d, f, o, _] => (d, f, o), _ => return Err("usage: pgs2_cold PACKAGE_DIR FIXTURES.json OUT.json [MONOLITHIC.pgs]".into()) };
    // The canonical reader on the monolithic file, for bit-for-bit comparison.
    let canonical = match a.get(3) { Some(p) => Some(universe_world::worlds::pgs::Surface::read(&std::fs::read(p)?)?), None => None };
    let dir = Path::new(dir);
    // The first region's cap centre (read before the cold runs), then a root-only place (its antipode).
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("planet.json"))?)?;
    let centre = DVec3::from_slice(&manifest["regions"][0]["cap"]["centre"].as_array().ok_or("cap")?.iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>());
    let runs = vec![cold(dir, centre, "region cap centre")?, cold(dir, -centre, "antipode, root only")?, cold(dir, centre, "region cap centre, again")?];
    let fixtures: Value = serde_json::from_slice(&std::fs::read(fixtures)?)?;
    let mut package = Package::open(dir)?;
    let (mut same_height, mut same_face, mut same_water, mut same_categories, mut worst) = (0, 0, 0, 0, 0f64);
    let mut differences = vec![];
    let (mut canon_height, mut canon_face, mut canon_geometry, mut canon_worst) = (0, 0, 0, 0f64);
    let start = Instant::now();
    let queries = fixtures["queries"].as_array().ok_or("queries")?;
    for f in queries {
        let d = DVec3::from_slice(&f["direction"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>());
        let r = package.query(d)?;
        let h = f["height_m"].as_f64().unwrap();
        if let Some(c) = &canonical {
            let s = c.query(d)?;
            canon_height += (s.height_m.to_bits() == r.height_m.to_bits()) as usize;
            canon_face += (s.face as u64 == r.face) as usize;
            // The same triangle (as a set of stored vertex positions), whatever its id.
            let pgs = package.file(r.owner.0);
            let v = pgs.face(r.owner.1)?.0;
            let mut mine = [pgs.point(v[0])?.0, pgs.point(v[1])?.0, pgs.point(v[2])?.0].map(|p| p.to_array().map(f64::to_bits));
            let mut theirs = c.face_geometry(s.face).ok_or("face")?.positions.map(|p| p.to_array().map(f64::to_bits));
            mine.sort();
            theirs.sort();
            canon_geometry += (mine == theirs) as usize;
            canon_worst = canon_worst.max((s.height_m - r.height_m).abs());
        }
        worst = worst.max((r.height_m - h).abs());
        let face_ok = f["face"].as_u64() == Some(r.face);
        let water_ok = f["water"]["state"] == r.water["state"] && (r.water["state"] != "wet" || f["water"]["body"] == r.water["body"]);
        let cat_ok = f["categories"]["rock"].as_u64() == Some(r.categories.0 as u64) && f["categories"]["pattern"].as_u64() == Some(r.categories.1 as u64);
        same_height += (r.height_m.to_bits() == h.to_bits()) as usize;
        same_face += face_ok as usize;
        same_water += water_ok as usize;
        same_categories += cat_ok as usize;
        if (!face_ok || !water_ok || !cat_ok || r.height_m.to_bits() != h.to_bits()) && differences.len() < 10 {
            differences.push(json!({"fixture":f,"got":{"face":r.face,"height_m":r.height_m,"water":r.water}}));
        }
    }
    let warm_ms = start.elapsed().as_secs_f64() * 1e3;
    let report = json!({"format":"pgs2-cold-read/1","package":dir,"page_bytes":PAGE,
        "method":"files mapped (MADV_RANDOM); page cache emptied of the package's files by posix_fadvise DONTNEED, checked by mincore; pages_read = pages the reader asked for; pages_resident_after includes any kernel readahead. Manifest opening records checked at open (header+table sha256, kind-19 block-hash section sha256); each 64 KiB block checked against its hash on first touch (its pages counted as read). No section read whole; whole-file and per-section hashes not checked (acceptance's).",
        "cold":runs,
        "fixtures":{"queries":queries.len(),"height_bit_identical":same_height,"height_max_abs_diff_m":worst,"composed_face_identical":same_face,"water_identical":same_water,"categories_identical":same_categories,"warm_ms_total":warm_ms,"vs_canonical_reader":canonical.as_ref().map(|_| json!({"height_bit_identical":canon_height,"face_id_identical":canon_face,"owner_triangle_identical":canon_geometry,"height_max_abs_diff_m":canon_worst})),"differences":differences,
            "touched_after_all":package.touched()},
        "notes":["Composed face ids computed from the region file's own face order, assumed canonical (compose(): sorted by global vertex triples).","Ties: every face of root and region whose cell lists it and whose closed triangle holds the direction; the owner is the least composed id."]});
    std::fs::write(out, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string(&json!({"cold":report["cold"].as_array().unwrap().iter().map(|r| json!({"q":r["query"],"ms":r["open_and_first_query_ms"],"read":r["bytes_in_pages_read"],"before":r["pages_resident_before"]})).collect::<Vec<_>>(),"fixtures":{"n":queries.len(),"height":same_height,"face":same_face,"water":same_water,"cat":same_categories,"worst":worst,"warm_ms":warm_ms,"canonical":report["fixtures"]["vs_canonical_reader"]}}))?);
    Ok(())
}
