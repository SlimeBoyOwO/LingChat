use crate::error::Result;
use ndarray::{Array2, Ix2};
use ort::session::Session;
use ort::value::TensorRef;

pub fn predict(
    session: &mut Session,
    token_ids: Vec<i64>,
    attention_masks: Vec<i64>,
) -> Result<Array2<f32>> {
    let outputs = session.run(
        ort::inputs! {
            "input_ids" => TensorRef::from_array_view((vec![1, token_ids.len() as i64], token_ids.as_slice()))?,
            "attention_mask" => TensorRef::from_array_view((vec![1, attention_masks.len() as i64], attention_masks.as_slice()))?,
        }
    )?;
    // DeBERTa exports use different names for the same single feature output.
    // Read it positionally so compatible models are not tied to one exporter.
    let output = outputs[0]
        .try_extract_array::<f32>()?
        .into_dimensionality::<Ix2>()?
        .to_owned();
    Ok(output)
}
