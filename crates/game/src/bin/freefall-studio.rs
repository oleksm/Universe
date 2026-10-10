//! The Freefall studio, alone: the interior studio's window (`[design]`), or its `--report`, `--fit`
//! and `--compare` printed.

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn")).init();
    let args: Vec<String> = std::env::args().collect();
    game::studio(&args, true);
}
