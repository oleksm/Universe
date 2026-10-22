//! Which of a patch's triangles a direction from the planet's centre may pass through: their bounds on
//! the patch's own gnomonic plane (a ray from the centre through a flat triangle is a point in its
//! projection there: planes through the centre project to lines), in buckets. A ground query tests the
//! triangles of one bucket instead of all of the patch's.

use universe_engine::glam::DVec3;

pub struct TriIndex {
    centre: DVec3,
    east: DVec3,
    north: DVec3,
    lo: (f64, f64),
    cell: f64,
    side: usize,
    /// Bucket b's triangles: `items[start[b]..start[b + 1]]` (their places in the list built from).
    start: Vec<u32>,
    items: Vec<u32>,
}

impl TriIndex {
    /// The index of `tris` (corners, any length from the centre) on the plane at `centre` (unit).
    pub fn new(centre: DVec3, tris: impl ExactSizeIterator<Item = [DVec3; 3]> + Clone) -> Self {
        let east = DVec3::Y.cross(centre).try_normalize().unwrap_or(DVec3::X);
        let north = centre.cross(east);
        let mut index = Self { centre, east, north, lo: (0.0, 0.0), cell: 1.0, side: 1, start: vec![0, 0], items: vec![] };
        let boxes: Vec<Option<[f64; 4]>> = tris
            .map(|t| {
                let p = t.map(|v| index.plane(v));
                let p: Option<Vec<(f64, f64)>> = p.into_iter().collect();
                p.map(|p| {
                    let (xs, ys) = (p.iter().map(|q| q.0), p.iter().map(|q| q.1));
                    [xs.clone().fold(f64::MAX, f64::min), ys.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.fold(f64::MIN, f64::max)]
                })
            })
            .collect();
        let all = boxes.iter().flatten().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]);
        let n = boxes.iter().flatten().count();
        if n == 0 {
            return index;
        }
        // (About four triangles a bucket.)
        index.side = ((n as f64 / 4.0).sqrt().ceil() as usize).clamp(1, 512);
        index.lo = (all[0], all[1]);
        index.cell = ((all[2] - all[0]).max(all[3] - all[1]) / index.side as f64).max(1e-12);
        let range = |b: &[f64; 4]| (index.bucket(b[0], b[1]), index.bucket(b[2], b[3]));
        let mut count = vec![0u32; index.side * index.side + 1];
        for b in boxes.iter().flatten() {
            let ((x0, y0), (x1, y1)) = range(b);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    count[y * index.side + x + 1] += 1;
                }
            }
        }
        for i in 1..count.len() {
            count[i] += count[i - 1];
        }
        let mut fill = count.clone();
        let mut items = vec![0; *count.last().unwrap() as usize];
        for (t, b) in boxes.iter().enumerate() {
            let Some(b) = b else { continue };
            let ((x0, y0), (x1, y1)) = range(b);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let at = &mut fill[y * index.side + x];
                    items[*at as usize] = t as u32;
                    *at += 1;
                }
            }
        }
        index.start = count;
        index.items = items;
        index
    }

    /// Where `d` falls on the plane; None: on its far side (or at its rim).
    fn plane(&self, d: DVec3) -> Option<(f64, f64)> {
        let k = d.dot(self.centre);
        (k > 1e-9 * d.length()).then(|| (d.dot(self.east) / k, d.dot(self.north) / k))
    }

    fn bucket(&self, x: f64, y: f64) -> (usize, usize) {
        let f = |v: f64| (((v / self.cell).floor().max(0.0)) as usize).min(self.side - 1);
        (f(x - self.lo.0), f(y - self.lo.1))
    }

    /// The triangles `d` may pass through (their places in the list built from).
    pub fn candidates(&self, d: DVec3) -> &[u32] {
        let Some((x, y)) = self.plane(d) else { return &[] };
        let span = self.cell * self.side as f64;
        if x < self.lo.0 - self.cell || y < self.lo.1 - self.cell || x > self.lo.0 + span + self.cell || y > self.lo.1 + span + self.cell {
            return &[];
        }
        let (bx, by) = self.bucket(x, y);
        let b = by * self.side + bx;
        &self.items[self.start[b] as usize..self.start[b + 1] as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every direction through a triangle finds it among its bucket's candidates.
    #[test]
    fn tri_index_finds_every_triangle_a_ray_passes() {
        let centre = DVec3::new(0.3, 0.8, 0.5).normalize();
        let east = DVec3::Y.cross(centre).normalize();
        let north = centre.cross(east);
        let at = |x: f64, y: f64| (centre + east * x + north * y).normalize() * 1000.0;
        let n = 20;
        let tris: Vec<[DVec3; 3]> = (0..n * n)
            .flat_map(|k| {
                let (i, j) = ((k % n) as f64 * 0.001, (k / n) as f64 * 0.001);
                [[at(i, j), at(i + 0.001, j), at(i + 0.001, j + 0.001)], [at(i, j), at(i + 0.001, j + 0.001), at(i, j + 0.001)]]
            })
            .collect();
        let index = TriIndex::new(centre, tris.iter().copied());
        for (t, tri) in tris.iter().enumerate() {
            let middle = (tri[0] + tri[1] + tri[2]).normalize();
            assert!(index.candidates(middle).contains(&(t as u32)), "triangle {t}");
            assert!(crate::flight::triangle_height(middle, *tri, 1000.0).is_some());
        }
        assert!(index.candidates(-centre).is_empty());
        assert!(index.candidates(at(0.5, 0.5).normalize()).is_empty());
        assert!(index.candidates(middle_of(&tris[0])).len() < 16);
    }

    fn middle_of(t: &[DVec3; 3]) -> DVec3 {
        (t[0] + t[1] + t[2]).normalize()
    }
}
