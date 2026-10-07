//! Every shader module, as the engine puts it together (its files in order, the shared constants
//! before them), parses and validates: a file missing from a module, a name two files both
//! declare, or a constant left out shows here rather than when a device is made. No GPU needed.

use universe_engine::shaders;

#[test]
fn every_shader_module_parses_and_validates() {
    let mut modules = vec![("scene", shaders::scene()), ("pbr", shaders::pbr()), ("cloud cache", shaders::cloud_cache())];
    modules.extend(shaders::singles());
    for (name, src) in modules {
        let module = naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
    }
}
