// The sun's light as the shadow map lets it through: one copy, for the scene's meshes and the
// models (concatenated into scene.wgsl's module and pbr.wgsl's). The including module declares
// `g` (Globals, with the shadow cascades' matrices and texels), `shadow_map` and `shadow_cmp`.

// What the shadow map says of the sun at `p` (camera-relative), on a
// surface facing `n`: 1 lit, 0 in shadow (2x2 filtered at the edge).
// Looked up a texel and a half off the surface, so it doesn't shadow itself.
fn sunlit(p: vec3<f32>, n: vec3<f32>) -> f32 {
    if (g.shadow.z == 0.0) {
        return 1.0;
    }
    return sunlit_near(p, n) * sunlit_ground(p, n);
}

// What the ground cascade says: the shadows of mountains, kilometres long.
// (Its texels are coarse: the ground offsets further along its normal.)
fn sunlit_ground(p: vec3<f32>, n: vec3<f32>) -> f32 {
    if (g.shadow2.w == 0.0) {
        return 1.0;
    }
    let s = g.shadow_ground * vec4<f32>(p + n * g.shadow2.z * 2.5, 1.0);
    let c = vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5);
    if (all(c > vec2<f32>(0.0)) && all(c < vec2<f32>(1.0)) && s.z > 0.0 && s.z < 1.0) {
        // (Faded out toward the box's edge, so its end isn't a line.)
        let edge = min(min(c.x, 1.0 - c.x), min(c.y, 1.0 - c.y));
        return mix(1.0, pcf(c, 3, s.z), smoothstep(0.0, 0.1, edge));
    }
    return 1.0;
}

// What the fine cascades say (what casts near the eye: ships, stations, rocks).
fn sunlit_near(p: vec3<f32>, n: vec3<f32>) -> f32 {
    if (g.shadow2.y > 0.0) {
        let tight = g.shadow_tight * vec4<f32>(p + n * g.shadow2.x * 1.5, 1.0);
        let c = vec2<f32>(tight.x * 0.5 + 0.5, 0.5 - tight.y * 0.5);
        if (all(c > vec2<f32>(0.02)) && all(c < vec2<f32>(0.98)) && tight.z > 0.0 && tight.z < 1.0) {
            return pcf(c, 2, tight.z);
        }
    }
    let near = g.shadow_near * vec4<f32>(p + n * g.shadow.x * 1.5, 1.0);
    let a = vec2<f32>(near.x * 0.5 + 0.5, 0.5 - near.y * 0.5);
    if (all(a > vec2<f32>(0.01)) && all(a < vec2<f32>(0.99)) && near.z > 0.0 && near.z < 1.0) {
        return pcf(a, 0, near.z);
    }
    let far = g.shadow_far * vec4<f32>(p + n * g.shadow.y * 1.5, 1.0);
    let b = vec2<f32>(far.x * 0.5 + 0.5, 0.5 - far.y * 0.5);
    if (all(b > vec2<f32>(0.0)) && all(b < vec2<f32>(1.0)) && far.z > 0.0 && far.z < 1.0) {
        return pcf(b, 1, far.z);
    }
    return 1.0;
}

// (A shadow map texel, its uv: `SHADOW_TEXEL`, written by shaders.rs.)

// 3x3 compared samples (each itself filtered 2x2): soft edges, no stair steps.
fn pcf(uv: vec2<f32>, layer: i32, depth: f32) -> f32 {
    var lit = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            lit += textureSampleCompareLevel(shadow_map, shadow_cmp, uv + vec2<f32>(f32(x), f32(y)) * SHADOW_TEXEL, layer, depth);
        }
    }
    return lit / 9.0;
}
