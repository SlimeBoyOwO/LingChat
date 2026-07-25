use crate::error::Result;
use ndarray::{array, Array1, Array2, Array3, Axis, Ix3};
use ort::session::{builder::GraphOptimizationLevel, Session};

#[allow(clippy::vec_init_then_push, unused_variables)]
pub fn load_model<P: AsRef<[u8]>>(model_file: P, bert: bool) -> Result<Session> {
    let mut exp = Vec::new();
    #[cfg(feature = "tensorrt")]
    {
        if bert {
            exp.push(
                ort::execution_providers::TensorRTExecutionProvider::default()
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
        let mut cuda = ort::execution_providers::CUDAExecutionProvider::default();
        #[cfg(feature = "cuda_tf32")]
        {
            cuda = cuda.with_tf32(true);
        }
        exp.push(cuda.build());
    }
    #[cfg(feature = "directml")]
    {
        exp.push(ort::execution_providers::DirectMLExecutionProvider::default().build());
    }
    #[cfg(feature = "coreml")]
    {
        exp.push(ort::execution_providers::CoreMLExecutionProvider::default().build());
    }
    exp.push(ort::execution_providers::CPUExecutionProvider::default().build());
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
