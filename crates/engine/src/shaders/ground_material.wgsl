// The ground's material close up: the linear albedo of land, from a world's maps as sampled at
// the pixel (`GroundIn`, filled in by `fs_mesh` in scene.wgsl from group 2). The lab's to write
// (docs/planet-studio-plan.md, branch `planet`): this stand-in is the plants-and-soil map alone.
// The sea is the scene shader's.

struct GroundIn {
    // The plants and soil without rock and snow (globe_ground), linear (its texture is sRGB:
    // sampled, it is linear already); and the same a softer mip (about 2.5 levels down).
    ground: vec3<f32>,
    ground_soft: vec3<f32>,
    // The climate (climate.png): the year's mean temperature at sea level (°C), rain (m a year).
    t_sea_c: f32,
    rain_m: f32,
    // The rock unit here (rockid.png, its number in the bake's geology order).
    unit: u32,
    // The 600 m surface fields (a river tile's lower block), 0..1; surface_on 1 where a tile is
    // read (0 for now: the river tiles aren't read yet).
    wet: f32,
    scree: f32,
    bare: f32,
    surface_on: f32,
    // The height above the sea (m), the slope (rise over run, as the ground stands), ridge (+)
    // or hollow (−) at about 8 km (0 for now).
    h_m: f32,
    slope: f32,
    rel: f32,
    // A position steady on the ground (m, wrapped every 4,096 m), for noise; a pixel's
    // footprint (m).
    q: vec3<f32>,
    pixel_m: f32,
};

fn ground_material(i: GroundIn) -> vec3<f32> {
    return i.ground;
}
