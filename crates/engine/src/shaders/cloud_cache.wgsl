// The cloud cache's compute pass (see clouds.wgsl, "The cloud cache"): its own module, after
// air.wgsl and clouds.wgsl (concatenated: air + clouds + this), so its bindings don't meet the
// scene's.

// The pass: one texel of one level of one shell.
@group(0) @binding(0) var<uniform> cc_u: CloudCache;
@group(0) @binding(1) var<uniform> cc_clouds: Clouds;
@group(0) @binding(2) var cc_out: texture_storage_2d_array<rgba16float, write>;
@group(0) @binding(3) var cc_cm: texture_2d<f32>;
@group(0) @binding(4) var cc_ce: texture_2d<f32>;
@group(0) @binding(5) var cc_ca: texture_2d<f32>;
// The first layer this dispatch fills (x): the engine fills a few layers a frame.
@group(0) @binding(6) var<uniform> cc_layer0: vec4<u32>;

@compute @workgroup_size(8, 8, 1)
fn cloud_cache_fill(@builtin(global_invocation_id) id: vec3<u32>) {
    let layer = i32(id.z + cc_layer0.x);
    let level = layer / 3;
    let shell = layer % 3;
    if (i32(id.x) >= CC_N || i32(id.y) >= CC_N || level >= CC_LEVELS) {
        return;
    }
    let half = cc_u.half_m[level];
    let texel = 2.0 * half / f32(CC_N);
    let xy = (vec2<f32>(f32(id.x), f32(id.y)) + 0.5) * texel - vec2<f32>(half);
    let dir = cc_unproject(xy, cc_u);
    let f = cloud_field(dir, cc_clouds, cc_cm, cc_ce, cc_ca);
    let cols = cloud_columns(dir, f, cc_u.center.w * 0.001, cc_clouds.time_s, texel * 0.001, shell + 1);
    var dt = cols.low;
    if (shell == 1) {
        dt = cols.mid;
    } else if (shell == 2) {
        dt = cols.high;
    }
    textureStore(cc_out, vec2<i32>(id.xy), layer, vec4<f32>(dt.x, dt.y, 0.0, 1.0));
}

