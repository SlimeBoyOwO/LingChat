use crate::error::Result;
pub use tokenizers::Tokenizer;

pub fn get_tokenizer<P: AsRef<[u8]>>(p: P) -> Result<Tokenizer> {
    let tokenizer = Tokenizer::from_bytes(p)?;
    Ok(tokenizer)
}

pub fn tokenize(text: &str, tokenizer: &Tokenizer) -> Result<(Vec<i64>, Vec<i64>)> {
    let mut token_ids = vec![1];
    let mut attention_masks = vec![1];
    for content in text.chars() {
        let token = tokenizer.encode(content.to_string(), false)?;
        let ids = token.get_ids();
        token_ids.extend(ids.iter().map(|&x| x as i64));
        attention_masks.extend(token.get_attention_mask().iter().map(|&x| x as i64));
    }
    token_ids.push(2);
    attention_masks.push(1);
    Ok((token_ids, attention_masks))
}
