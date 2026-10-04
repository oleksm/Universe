// How much of the sun's disc is behind what's drawn (see `sunprobe.rs`):
// the depth buffer read at points spread over the disc on screen (a
// sunflower spiral), each marked covered if something nearer than the sun
// stands there. Run after the scene (mode 0: marks set) and after the front
// layer (mode 1: covered there too), as each has its own depth.

struct Probe {
    centre: vec2<f32>,
    radius: f32,
    // Depth just this side of the sun (reversed: nearer is larger).
    threshold: f32,
    size: vec2<f32>,
    mode: u32,
    pad: u32,
};

@group(0) @binding(0) var depth: texture_depth_multisampled_2d;
@group(0) @binding(1) var<uniform> probe: Probe;
// Per point: 0 clear, 1 covered, 2 off the screen (not seen either way).
@group(0) @binding(2) var<storage, read_write> marks: array<u32, 64>;

@compute @workgroup_size(64)
fn main(@builtin(local_invocation_index) i: u32) {
    let r = probe.radius * sqrt((f32(i) + 0.5) / 64.0);
    let a = f32(i) * 2.3999632;
    let p = probe.centre + vec2<f32>(cos(a), sin(a)) * r;
    if (any(p < vec2<f32>(0.0)) || any(p >= probe.size)) {
        if (probe.mode == 0u) {
            marks[i] = 2u;
        }
        return;
    }
    let covered = textureLoad(depth, vec2<i32>(p), 0) > probe.threshold;
    if (probe.mode == 0u) {
        marks[i] = select(0u, 1u, covered);
    } else if (covered && marks[i] == 0u) {
        marks[i] = 1u;
    }
}
