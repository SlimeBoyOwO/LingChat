use crate::error::Result;
use ndarray::{array, Array1, Array2, Array3, Axis, Ix3};
use ort::session::{builder::GraphOptimizationLevel, Session};

/// 推理硬件设备选择（热切换用）。
///
/// 由调用方（LingChat TTS 设置）指定，创建 Session 时固化到 EP 列表；
/// 切换设备 = 重建 Session（unload 后下次加载生效）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InferenceDevice {
    /// CPU 推理（默认，全平台可用）
    #[default]
    Cpu,
    /// DirectML GPU（Windows，DX12 显卡）
    Gpu,
    /// DirectML NPU（Windows，Intel NPU 等）
    Npu,
    /// DirectML 指定设备（device_id 由 DirectML 枚举）
    Specific(i32),
}

#[allow(clippy::vec_init_then_push, unused_variables)]
pub fn load_model<P: AsRef<[u8]>>(model_file: P, bert: bool) -> Result<Session> {
    load_model_with_device(model_file, bert, InferenceDevice::Cpu)
}

/// 按指定推理设备加载模型。设备列表按优先级排列，DirectML 不可用时
/// 自动回退 CPU（ORT 的 EP fallback 语义）。
#[allow(clippy::vec_init_then_push, unused_variables)]
pub fn load_model_with_device<P: AsRef<[u8]>>(
    model_file: P,
    bert: bool,
    device: InferenceDevice,
) -> Result<Session> {
    let mut exp: Vec<ort::ep::ExecutionProviderDispatch> = Vec::new();
    #[cfg(feature = "tensorrt")]
    {
        if bert {
            exp.push(
                ort::ep::TensorRT::default()
                    .with_fp16(true)
                    .with_profile_min_shapes("input_ids:1x1,attention_mask:1x1")
                    .with_profile_max_shapes("input_ids:1x100,attention_mask:1x100")
                    .with_profile_opt_shapes("input_ids:1x25,attention_mask:1x25")
                    .build(),
            );
        }
    }
    #[cfg(feature = "cuda")]
    {
        #[allow(unused_mut)]
        let mut cuda = ort::ep::CUDA::default();
        #[cfg(feature = "cuda_tf32")]
        {
            cuda = cuda.with_tf32(true);
        }
        exp.push(cuda.build());
    }
    #[cfg(feature = "directml")]
    {
        use ort::ep::directml::{DeviceFilter, PerformancePreference};
        match device {
            InferenceDevice::Gpu => exp.push(
                ort::ep::DirectML::default()
                    .with_device_filter(DeviceFilter::Gpu)
                    .with_performance_preference(PerformancePreference::HighPerformance)
                    .build(),
            ),
            InferenceDevice::Npu => exp.push(
                ort::ep::DirectML::default()
                    .with_device_filter(DeviceFilter::Npu)
                    .with_performance_preference(PerformancePreference::Default)
                    .build(),
            ),
            InferenceDevice::Specific(id) => {
                exp.push(ort::ep::DirectML::default().with_device_id(id).build())
            }
            InferenceDevice::Cpu => {}
        }
    }
    #[cfg(feature = "coreml")]
    {
        exp.push(ort::ep::CoreML::default().build());
    }
    #[cfg(all(feature = "webgpu", target_os = "linux"))]
    {
        // WebGPU EP：仅 Linux（Dawn→Vulkan）。macOS/Android 不做硬件适配，走 CPU。
        // 与 DirectML 一样按用户选择的设备：Gpu 用默认设备，Specific 指定 id。
        use ort::ep::webgpu::DawnBackendType;
        match device {
            InferenceDevice::Gpu => exp.push(
                ort::ep::WebGPU::default()
                    .with_dawn_backend_type(DawnBackendType::Vulkan)
                    .build(),
            ),
            InferenceDevice::Specific(id) => exp.push(
                ort::ep::WebGPU::default()
                    .with_dawn_backend_type(DawnBackendType::Vulkan)
                    .with_device_id(id)
                    .build(),
            ),
            InferenceDevice::Cpu | InferenceDevice::Npu => {}
        }
    }
    exp.push(ort::ep::CPU::default().build());
    #[cfg(any(feature = "directml", feature = "webgpu"))]
    {
        // 诊断：打印选中的推理设备。WebGPU（Linux）下用于验证 deviceId 是否生效。
        eprintln!(
            "[sbv2_core] load_model_with_device device={:?} bert={} EP数量={}",
            device,
            bert,
            exp.len()
        );
    }
    Ok(Session::builder()?
        .with_execution_providers(exp)?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(num_cpus::get_physical())?
        .with_parallel_execution(true)?
        .with_inter_threads(num_cpus::get_physical())?
        .commit_from_memory(model_file.as_ref())?)
}

#[allow(clippy::too_many_arguments)]
pub fn synthesize(
    session: &mut Session,
    bert_ori: Array2<f32>,
    x_tst: Array1<i64>,
    mut spk_ids: Array1<i64>,
    tones: Array1<i64>,
    lang_ids: Array1<i64>,
    style_vector: Array1<f32>,
    sdp_ratio: f32,
    length_scale: f32,
    noise_scale: f32,
    noise_scale_w: f32,
) -> Result<Array3<f32>> {
    let bert_ori = bert_ori.insert_axis(Axis(0));
    let bert_ori = bert_ori.as_standard_layout();
    let bert = ort::value::TensorRef::from_array_view(&bert_ori)?;
    let mut x_tst_lengths = array![x_tst.shape()[0] as i64];
    let x_tst_lengths = ort::value::TensorRef::from_array_view(&mut x_tst_lengths)?;
    let mut x_tst = x_tst.insert_axis(Axis(0));
    let x_tst = ort::value::TensorRef::from_array_view(&mut x_tst)?;
    let mut lang_ids = lang_ids.insert_axis(Axis(0));
    let lang_ids = ort::value::TensorRef::from_array_view(&mut lang_ids)?;
    let mut tones = tones.insert_axis(Axis(0));
    let tones = ort::value::TensorRef::from_array_view(&mut tones)?;
    let mut style_vector = style_vector.insert_axis(Axis(0));
    let style_vector = ort::value::TensorRef::from_array_view(&mut style_vector)?;
    let sid = ort::value::TensorRef::from_array_view(&mut spk_ids)?;
    let sdp_ratio = vec![sdp_ratio];
    let sdp_ratio = ort::value::TensorRef::from_array_view((vec![1_i64], sdp_ratio.as_slice()))?;
    let length_scale = vec![length_scale];
    let length_scale =
        ort::value::TensorRef::from_array_view((vec![1_i64], length_scale.as_slice()))?;
    let noise_scale = vec![noise_scale];
    let noise_scale =
        ort::value::TensorRef::from_array_view((vec![1_i64], noise_scale.as_slice()))?;
    let noise_scale_w = vec![noise_scale_w];
    let noise_scale_w =
        ort::value::TensorRef::from_array_view((vec![1_i64], noise_scale_w.as_slice()))?;
    let outputs = session.run(ort::inputs! {
        "x_tst" =>  x_tst,
        "x_tst_lengths" => x_tst_lengths,
        "sid" => sid,
        "tones" => tones,
        "language" => lang_ids,
        "bert" => bert,
        "style_vec" => style_vector,
        "sdp_ratio" => sdp_ratio,
        "length_scale" => length_scale,
        "noise_scale" => noise_scale,
        "noise_scale_w" => noise_scale_w,
    })?;
    let audio_array = outputs["output"]
        .try_extract_array::<f32>()?
        .into_dimensionality::<Ix3>()?
        .to_owned();
    Ok(audio_array)
}
