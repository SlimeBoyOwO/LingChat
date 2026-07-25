use crate::error::{Error, Result};
use ndarray::{s, Array1, Array2};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Data {
    pub shape: [usize; 2],
    pub data: Vec<Vec<f32>>,
}

pub fn load_style<P: AsRef<[u8]>>(path: P) -> Result<Array2<f32>> {
    let data: Data = serde_json::from_slice(path.as_ref())?;
    Ok(Array2::from_shape_vec(
        data.shape,
        data.data.iter().flatten().copied().collect(),
    )?)
}

pub fn get_style_vector(
    style_vectors: &Array2<f32>,
    style_id: i32,
    weight: f32,
) -> Result<Array1<f32>> {
    if style_vectors.shape().len() != 2 {
        return Err(Error::StyleError(
            "Invalid shape for style vectors".to_string(),
        ));
    }
    if style_id < 0 || style_id >= style_vectors.shape()[0] as i32 {
        return Err(Error::StyleError(format!(
            "Invalid style ID: {}. Max ID: {}",
            style_id,
            style_vectors.shape()[0] - 1
        )));
    }
    let mean = style_vectors.slice(s![0, ..]).to_owned();
    let style_vector = style_vectors.slice(s![style_id as usize, ..]).to_owned();
    let diff = (style_vector - &mean) * weight;
    Ok(mean + &diff)
}
