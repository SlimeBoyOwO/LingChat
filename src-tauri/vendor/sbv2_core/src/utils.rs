pub fn intersperse<T>(slice: &[T], sep: T) -> Vec<T>
where
    T: Clone,
{
    let mut result = vec![sep.clone(); slice.len() * 2 + 1];
    result
        .iter_mut()
        .step_by(2)
        .zip(slice.iter())
        .for_each(|(r, s)| *r = s.clone());
    result
}
