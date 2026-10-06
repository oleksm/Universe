// The ground's runtime detail, the GPU's twin of `world::detail::offset` (the truth, in f64):
// held to it within its AGREE_M by the engine's test (tests/detail_agree.rs). The lab's
// generator; until it lands, a stand-in that adds nothing.
//
// The module that includes this one supplies the tile's samples, indices running two past its
// edges into its neighbours (the halo):
//   fn detail_height(i: i32, j: i32) -> f32        the ~150 m tile's differences (m, over the 600 m)
//   fn detail_fields(i: i32, j: i32) -> vec4<f32>  fd150, raw 0..255 (flow, area, threshold, ice)

// `at`: the place within its tile (m from the tile's origin, along u and v); `spacing`: the
// tile's sample spacing (m); `rock`: the rock unit; `seed`: the world's seed (low, high words);
// `cell`: the band (m): finer than about two cells left out.
fn detail(at: vec2<f32>, spacing: f32, rock: u32, seed: vec2<u32>, cell: f32) -> f32 {
    return 0.0;
}
