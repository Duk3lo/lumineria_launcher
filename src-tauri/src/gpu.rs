use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuTier {
    None = 0,
    Integrated = 1,
    Discrete = 2,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub tier: String, // "discrete" | "integrated" | "none"
}

async fn detect_best_adapter() -> Option<(GpuTier, String)> {
    let instance = wgpu::Instance::default();
    let mut best: Option<(GpuTier, String)> = None;

    for adapter in instance.enumerate_adapters(wgpu::Backends::all()).await {
        let info = adapter.get_info();
        let tier = match info.device_type {
            wgpu::DeviceType::DiscreteGpu => GpuTier::Discrete,
            wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::VirtualGpu => GpuTier::Integrated,
            wgpu::DeviceType::Cpu | wgpu::DeviceType::Other => GpuTier::None,
        };

        let is_better = match &best {
            None => true,
            Some((best_tier, _)) => tier > *best_tier,
        };
        if is_better {
            best = Some((tier, info.name));
        }
    }

    best
}

pub async fn detect_gpu_tier() -> GpuTier {
    detect_best_adapter().await.map(|(tier, _)| tier).unwrap_or(GpuTier::None)
}

pub async fn has_good_gpu() -> bool {
    detect_gpu_tier().await >= GpuTier::Discrete
}

fn tier_label(tier: GpuTier) -> &'static str {
    match tier {
        GpuTier::Discrete => "discrete",
        GpuTier::Integrated => "integrated",
        GpuTier::None => "none",
    }
}

#[tauri::command]
pub async fn gpu_tier_label() -> String {
    tier_label(detect_gpu_tier().await).into()
}

#[tauri::command]
pub async fn get_gpu_info() -> GpuInfo {
    match detect_best_adapter().await {
        Some((tier, name)) => GpuInfo { name, tier: tier_label(tier).into() },
        None => GpuInfo { name: "No detectada".into(), tier: tier_label(GpuTier::None).into() },
    }
}