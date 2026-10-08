//! The compiled pipelines kept on disk between runs (wgpu's pipeline cache, where the GPU and driver
//! have one): the first run after a build otherwise compiles every pipeline in the driver, seconds
//! before the first frame. Opened with the device, given to every pipeline the engine makes (and to a
//! plugged-in ground's, through `GroundSetup`), written back after the first frame.
//!
//! The file is the adapter's own (named by vendor, device and driver); wgpu checks the data belongs to
//! this adapter and driver and starts empty when it doesn't.

use std::path::PathBuf;
use std::sync::OnceLock;

static CACHE: OnceLock<Option<(wgpu::PipelineCache, PathBuf)>> = OnceLock::new();

/// Where the adapter's cache is kept: the user's cache folder (`XDG_CACHE_HOME`, `~/.cache`, or on
/// Windows `LOCALAPPDATA`).
fn path(info: &wgpu::AdapterInfo) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let name: String = format!("{:04x}-{:04x}-{}", info.vendor, info.device, info.driver_info).chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    Some(base.join("universe-engine").join(format!("pipelines-{name}.bin")))
}

/// Open the cache for `device` (once; nothing where the device lacks the feature).
pub(crate) fn open(device: &wgpu::Device, info: &wgpu::AdapterInfo) {
    CACHE.get_or_init(|| {
        if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
            return None;
        }
        let path = path(info)?;
        let data = std::fs::read(&path).ok();
        // SAFETY: the data is what an earlier run's `get_data` wrote for this adapter (the file is named
        // by it); wgpu validates its header and, with `fallback`, starts empty when it won't do.
        let cache = unsafe { device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor { label: Some("pipelines"), data: data.as_deref(), fallback: true }) };
        log::info!("pipeline cache: {} ({})", path.display(), data.as_ref().map_or("new".to_string(), |d| format!("{} KB read", d.len() / 1024)));
        Some((cache, path))
    });
}

/// The cache, for a pipeline's descriptor (None: none).
pub fn get() -> Option<&'static wgpu::PipelineCache> {
    CACHE.get().and_then(|c| c.as_ref()).map(|(c, _)| c)
}

/// Write the cache back (after the first frame: every pipeline made by then is in it).
pub(crate) fn save() {
    let Some((cache, path)) = CACHE.get().and_then(|c| c.as_ref()) else { return };
    let Some(data) = cache.get_data() else { return };
    if std::fs::read(path).is_ok_and(|old| old == data) {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // (Written beside and moved into place: a run stopped half-way leaves the old file whole.)
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, &data).is_ok() && std::fs::rename(&tmp, path).is_ok() {
        log::info!("pipeline cache: {} KB written", data.len() / 1024);
    }
}
