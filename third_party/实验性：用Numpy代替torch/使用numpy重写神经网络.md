# Emotion-Model-Trainer-main 项目概览

## 项目结构
```text
Emotion-Model-Trainer-main/
├── dummy_numpy_bert_model/
│   ├── model_config.json
│   └── model_weights.npz
├── dummy_numpy_tokenizer_files/
│   ├── tokenizer_config.json
│   └── vocab.json
├── emotion_model_18emo/
│   ├── config.json
│   ├── label_mapping.json
│   ├── model.safetensors
│   ├── special_tokens_map.json
│   ├── tokenizer_config.json
│   ├── training_args.bin
│   └── vocab.txt
├── numpy_bert_model/
│   ├── model_config.json
│   ├── model_weights.npz
│   ├── tokenizer_config.json
│   └── vocab.json
├── BERT.py
├── NumpyBertTokenizer.py
├── README.md
├── addData.py
├── conversion.py
├── csvCleaner.py
├── emotion_data_manual.csv
├── inference.py
├── startTrain.py
├── testModel.py
└── 情绪一栏.txt
```

## 文件内容
### 文件: `README.md`

```markdown
# 情感分类器训练器
> 一个简单的用来训练情感语句分类模型的python程序

## 如何使用？
1. 在csv模型里输入情感语句和对应的情感标签
2. 使用csvCleaner.py处理csv文件防止字符污染
3. 运行startTrain.py等待训练完毕，完成后自动生成模型品质报告
4. 使用testModel.py测试模型

## 额外功能
> 可以用addData.py人工审核添加情绪
```

### 文件: `readme.md`

```markdown
# 情感分类器训练器
> 一个简单的用来训练情感语句分类模型的python程序

## 如何使用？
1. 在csv模型里输入情感语句和对应的情感标签
2. 使用csvCleaner.py处理csv文件防止字符污染
3. 运行startTrain.py等待训练完毕，完成后自动生成模型品质报告
4. 使用testModel.py测试模型

## 额外功能
> 可以用addData.py人工审核添加情绪
```

### 文件: `BERT.py`

```python
import numpy as np
import json
import math
import concurrent.futures
import os

def gelu(x):
    """Gaussian Error Linear Unit.
    This is a smoother version of ReLU.
    Original paper: https://arxiv.org/abs/1606.08415
    Approximation from: https://github.com/huggingface/transformers/blob/main/src/transformers/activations.py#L42
    """
    return 0.5 * x * (1.0 + np.tanh(math.sqrt(2.0 / math.pi) * (x + 0.044715 * np.power(x, 3.0))))

def softmax(x, axis=-1):
    """Compute softmax values for each sets of scores in x."""
    e_x = np.exp(x - np.max(x, axis=axis, keepdims=True))
    return e_x / np.sum(e_x, axis=axis, keepdims=True)

def layer_norm(x, gamma, beta, epsilon=1e-12):
    """Applies Layer Normalization.
    x: input tensor (batch_size, seq_len, hidden_size)
    gamma: scale tensor (hidden_size,)
    beta: offset tensor (hidden_size,)
    epsilon: small float to avoid division by zero
    """
    mean = np.mean(x, axis=-1, keepdims=True)
    variance = np.var(x, axis=-1, keepdims=True)
    x_normalized = (x - mean) / np.sqrt(variance + epsilon)
    return gamma * x_normalized + beta

class BertEmbeddingsNumpy:
    def __init__(self, config, weights):
        self.word_embeddings = weights['bert.embeddings.word_embeddings.weight']
        self.position_embeddings = weights['bert.embeddings.position_embeddings.weight']
        self.token_type_embeddings = weights['bert.embeddings.token_type_embeddings.weight']

        self.gamma = weights['bert.embeddings.LayerNorm.weight']
        self.beta = weights['bert.embeddings.LayerNorm.bias']

        self.vocab_size = config['vocab_size']
        self.hidden_size = config['hidden_size']
        self.max_position_embeddings = config['max_position_embeddings']
        self.type_vocab_size = config['type_vocab_size']
        self.layer_norm_eps = config.get('layer_norm_eps', 1e-12) 

    def forward(self, input_ids, token_type_ids=None, position_ids=None):
        seq_length = input_ids.shape[1]
        batch_size = input_ids.shape[0]

        if position_ids is None:
            position_ids = np.arange(seq_length, dtype=np.int64)
            position_ids = np.expand_dims(position_ids, axis=0)
            if batch_size > 1:
                 position_ids = np.tile(position_ids, (batch_size, 1))

        if token_type_ids is None:
            token_type_ids = np.zeros_like(input_ids, dtype=np.int64)

        words_embeds = self.word_embeddings[input_ids]
        position_embeds = self.position_embeddings[position_ids]
        token_type_embeds = self.token_type_embeddings[token_type_ids]

        embeddings = words_embeds + position_embeds + token_type_embeds
        embeddings = layer_norm(embeddings, self.gamma, self.beta, epsilon=self.layer_norm_eps)
        return embeddings

class BertSelfAttentionNumpy:
    def __init__(self, config, weights, layer_idx):
        self.num_attention_heads = config['num_attention_heads']
        self.attention_head_size = config['hidden_size'] // self.num_attention_heads
        self.all_head_size = self.num_attention_heads * self.attention_head_size

        self.query_w = weights[f'bert.encoder.layer.{layer_idx}.attention.self.query.weight'].T
        self.query_b = weights[f'bert.encoder.layer.{layer_idx}.attention.self.query.bias']
        self.key_w = weights[f'bert.encoder.layer.{layer_idx}.attention.self.key.weight'].T
        self.key_b = weights[f'bert.encoder.layer.{layer_idx}.attention.self.key.bias']
        self.value_w = weights[f'bert.encoder.layer.{layer_idx}.attention.self.value.weight'].T
        self.value_b = weights[f'bert.encoder.layer.{layer_idx}.attention.self.value.bias']

    def transpose_for_scores(self, x):
        new_x_shape = x.shape[:-1] + (self.num_attention_heads, self.attention_head_size)
        x = x.reshape(new_x_shape)
        return x.transpose(0, 2, 1, 3)

    def forward(self, hidden_states, attention_mask=None):
        query_layer = np.dot(hidden_states, self.query_w) + self.query_b
        key_layer = np.dot(hidden_states, self.key_w) + self.key_b
        value_layer = np.dot(hidden_states, self.value_w) + self.value_b

        query_layer = self.transpose_for_scores(query_layer)
        key_layer = self.transpose_for_scores(key_layer)
        value_layer = self.transpose_for_scores(value_layer)

        attention_scores = np.matmul(query_layer, key_layer.transpose(0, 1, 3, 2))
        attention_scores = attention_scores / math.sqrt(self.attention_head_size)

        if attention_mask is not None:
            attention_scores = attention_scores + attention_mask

        attention_probs = softmax(attention_scores, axis=-1)
        context_layer = np.matmul(attention_probs, value_layer)
        context_layer = context_layer.transpose(0, 2, 1, 3)
        new_context_layer_shape = context_layer.shape[:-2] + (self.all_head_size,)
        context_layer = context_layer.reshape(new_context_layer_shape)
        return context_layer

class BertSelfOutputNumpy:
    def __init__(self, config, weights, layer_idx):
        self.dense_w = weights[f'bert.encoder.layer.{layer_idx}.attention.output.dense.weight'].T
        self.dense_b = weights[f'bert.encoder.layer.{layer_idx}.attention.output.dense.bias']
        self.gamma = weights[f'bert.encoder.layer.{layer_idx}.attention.output.LayerNorm.weight']
        self.beta = weights[f'bert.encoder.layer.{layer_idx}.attention.output.LayerNorm.bias']
        self.layer_norm_eps = config.get('layer_norm_eps', 1e-12)

    def forward(self, hidden_states, input_tensor):
        hidden_states = np.dot(hidden_states, self.dense_w) + self.dense_b
        hidden_states = layer_norm(hidden_states + input_tensor, self.gamma, self.beta, epsilon=self.layer_norm_eps)
        return hidden_states

class BertAttentionNumpy:
    def __init__(self, config, weights, layer_idx):
        self.self_attention = BertSelfAttentionNumpy(config, weights, layer_idx)
        self.output = BertSelfOutputNumpy(config, weights, layer_idx)

    def forward(self, hidden_states, attention_mask=None):
        self_outputs = self.self_attention.forward(hidden_states, attention_mask)
        attention_output = self.output.forward(self_outputs, hidden_states)
        return attention_output

class BertIntermediateNumpy:
    def __init__(self, config, weights, layer_idx):
        self.dense_w = weights[f'bert.encoder.layer.{layer_idx}.intermediate.dense.weight'].T
        self.dense_b = weights[f'bert.encoder.layer.{layer_idx}.intermediate.dense.bias']
        self.hidden_act = config.get('hidden_act', 'gelu')
        if self.hidden_act == 'gelu':
            self.activation_fn = gelu
        elif self.hidden_act == 'relu':
            self.activation_fn = lambda x: np.maximum(0, x)
        else:
            raise ValueError(f"Unsupported activation function: {self.hidden_act}")

    def forward(self, hidden_states):
        hidden_states = np.dot(hidden_states, self.dense_w) + self.dense_b
        hidden_states = self.activation_fn(hidden_states)
        return hidden_states

class BertOutputNumpy:
    def __init__(self, config, weights, layer_idx):
        self.dense_w = weights[f'bert.encoder.layer.{layer_idx}.output.dense.weight'].T
        self.dense_b = weights[f'bert.encoder.layer.{layer_idx}.output.dense.bias']
        self.gamma = weights[f'bert.encoder.layer.{layer_idx}.output.LayerNorm.weight']
        self.beta = weights[f'bert.encoder.layer.{layer_idx}.output.LayerNorm.bias']
        self.layer_norm_eps = config.get('layer_norm_eps', 1e-12)

    def forward(self, hidden_states, input_tensor):
        hidden_states = np.dot(hidden_states, self.dense_w) + self.dense_b
        hidden_states = layer_norm(hidden_states + input_tensor, self.gamma, self.beta, epsilon=self.layer_norm_eps)
        return hidden_states

class BertLayerNumpy:
    def __init__(self, config, weights, layer_idx):
        self.attention = BertAttentionNumpy(config, weights, layer_idx)
        self.intermediate = BertIntermediateNumpy(config, weights, layer_idx)
        self.output = BertOutputNumpy(config, weights, layer_idx)

    def forward(self, hidden_states, attention_mask=None):
        attention_output = self.attention.forward(hidden_states, attention_mask)
        intermediate_output = self.intermediate.forward(attention_output)
        layer_output = self.output.forward(intermediate_output, attention_output)
        return layer_output

class BertEncoderNumpy:
    def __init__(self, config, weights):
        self.num_hidden_layers = config['num_hidden_layers']
        self.layers = [BertLayerNumpy(config, weights, i) for i in range(self.num_hidden_layers)]

    def forward(self, hidden_states, attention_mask=None):
        all_hidden_states = []
        for layer_module in self.layers:
            all_hidden_states.append(hidden_states)
            hidden_states = layer_module.forward(hidden_states, attention_mask)
        all_hidden_states.append(hidden_states)
        return hidden_states, all_hidden_states

class BertPoolerNumpy:
    def __init__(self, config, weights):
        self.dense_w = weights['bert.pooler.dense.weight'].T
        self.dense_b = weights['bert.pooler.dense.bias']

    def forward(self, hidden_states):
        first_token_tensor = hidden_states[:, 0]
        pooled_output = np.dot(first_token_tensor, self.dense_w) + self.dense_b
        pooled_output = np.tanh(pooled_output)
        return pooled_output

class BertForSequenceClassificationNumpy:
    def __init__(self, model_config_path, model_weights_path, num_threads=0): # Added num_threads
        with open(model_config_path, 'r', encoding='utf-8') as f:
            self.config = json.load(f)

        self.weights = np.load(model_weights_path)

        self.embeddings = BertEmbeddingsNumpy(self.config, self.weights)
        self.encoder = BertEncoderNumpy(self.config, self.weights)
        self.pooler = BertPoolerNumpy(self.config, self.weights)

        self.classifier_w = self.weights['classifier.weight'].T
        self.classifier_b = self.weights['classifier.bias']

        if 'num_labels' in self.config:
            self.num_labels = self.config['num_labels']
        elif 'id2label' in self.config:
            self.num_labels = len(self.config['id2label'])
        else:
            self.num_labels = self.classifier_w.shape[1]

        self.num_threads = num_threads
        if self.num_threads > 0:
            self.executor = concurrent.futures.ThreadPoolExecutor(max_workers=self.num_threads)
            print(f"Initialized ThreadPoolExecutor with {self.num_threads} workers.")
        else:
            self.executor = None
            print("Running in single-threaded mode.")

    def _create_attention_mask(self, input_mask):
        extended_attention_mask = input_mask[:, np.newaxis, np.newaxis, :]
        extended_attention_mask = (1.0 - extended_attention_mask) * -10000.0
        return extended_attention_mask

    def _forward_single_sample(self, input_ids_sample, attention_mask_sample, token_type_ids_sample):
        """
        Processes a single sample. Inputs are expected to be 1D arrays (seq_len,).
        They will be expanded to (1, seq_len) for compatibility with existing components.
        """
        input_ids_s = input_ids_sample[np.newaxis, :]
        attention_mask_s = attention_mask_sample[np.newaxis, :] if attention_mask_sample is not None else None
        token_type_ids_s = token_type_ids_sample[np.newaxis, :] if token_type_ids_sample is not None else None

        if attention_mask_s is None:
            attention_mask_s = np.ones_like(input_ids_s)
        if token_type_ids_s is None:
            token_type_ids_s = np.zeros_like(input_ids_s)

        embedding_output = self.embeddings.forward(input_ids_s, token_type_ids_s)
        extended_attention_mask = self._create_attention_mask(attention_mask_s)
        sequence_output, _ = self.encoder.forward(embedding_output, extended_attention_mask)
        pooled_output = self.pooler.forward(sequence_output)
        logits = np.dot(pooled_output, self.classifier_w) + self.classifier_b
        return logits 

    def forward(self, input_ids, attention_mask=None, token_type_ids=None):
        batch_size = input_ids.shape[0]

        if attention_mask is None:
            attention_mask = np.ones_like(input_ids, dtype=np.int64)
        if token_type_ids is None:
            token_type_ids = np.zeros_like(input_ids, dtype=np.int64)

        if self.executor and batch_size > 1: 
            futures = []
            for i in range(batch_size):
                future = self.executor.submit(
                    self._forward_single_sample,
                    input_ids[i],
                    attention_mask[i],
                    token_type_ids[i]
                )
                futures.append(future)

            results_list = [future.result() for future in futures] 
            
            all_logits = np.vstack(results_list)
            return all_logits
        else:
            embedding_output = self.embeddings.forward(input_ids, token_type_ids)
            extended_attention_mask = self._create_attention_mask(attention_mask)
            sequence_output, _ = self.encoder.forward(embedding_output, extended_attention_mask)
            pooled_output = self.pooler.forward(sequence_output)
            logits = np.dot(pooled_output, self.classifier_w) + self.classifier_b
            return logits

    def __del__(self):
        if self.executor:
            print("Shutting down ThreadPoolExecutor...")
            self.executor.shutdown(wait=True)

if __name__ == '__main__':
    dummy_config = {
        "vocab_size": 100, "hidden_size": 12, "num_hidden_layers": 1,
        "num_attention_heads": 2, "intermediate_size": 24, "hidden_act": "gelu",
        "max_position_embeddings": 20, "type_vocab_size": 2,
        "initializer_range": 0.02, "layer_norm_eps": 1e-12, "num_labels": 3
    }
    dummy_weights_data = {}
    hs, vs, mpe, tvs, nah, inter_s, nl = (dummy_config['hidden_size'], dummy_config['vocab_size'],
                                       dummy_config['max_position_embeddings'], dummy_config['type_vocab_size'],
                                       dummy_config['num_attention_heads'], dummy_config['intermediate_size'],
                                       dummy_config['num_labels'])
    ahs = hs // nah

    dummy_weights_data['bert.embeddings.word_embeddings.weight'] = np.random.rand(vs, hs).astype(np.float32)
    dummy_weights_data['bert.embeddings.position_embeddings.weight'] = np.random.rand(mpe, hs).astype(np.float32)
    dummy_weights_data['bert.embeddings.token_type_embeddings.weight'] = np.random.rand(tvs, hs).astype(np.float32)
    dummy_weights_data['bert.embeddings.LayerNorm.weight'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data['bert.embeddings.LayerNorm.bias'] = np.random.rand(hs).astype(np.float32)
    layer_idx = 0
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.self.query.weight'] = np.random.rand(hs, hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.self.query.bias'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.self.key.weight'] = np.random.rand(hs, hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.self.key.bias'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.self.value.weight'] = np.random.rand(hs, hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.self.value.bias'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.output.dense.weight'] = np.random.rand(hs, hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.output.dense.bias'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.output.LayerNorm.weight'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.attention.output.LayerNorm.bias'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.intermediate.dense.weight'] = np.random.rand(inter_s, hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.intermediate.dense.bias'] = np.random.rand(inter_s).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.output.dense.weight'] = np.random.rand(hs, inter_s).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.output.dense.bias'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.output.LayerNorm.weight'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data[f'bert.encoder.layer.{layer_idx}.output.LayerNorm.bias'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data['bert.pooler.dense.weight'] = np.random.rand(hs, hs).astype(np.float32)
    dummy_weights_data['bert.pooler.dense.bias'] = np.random.rand(hs).astype(np.float32)
    dummy_weights_data['classifier.weight'] = np.random.rand(nl, hs).astype(np.float32)
    dummy_weights_data['classifier.bias'] = np.random.rand(nl).astype(np.float32)

    dummy_dir = "./dummy_numpy_bert_model"
    os.makedirs(dummy_dir, exist_ok=True)
    dummy_config_path = os.path.join(dummy_dir, "model_config.json")
    dummy_weights_path = os.path.join(dummy_dir, "model_weights.npz")

    with open(dummy_config_path, 'w') as f:
        json.dump(dummy_config, f)
    np.savez_compressed(dummy_weights_path, **dummy_weights_data)
    print(f"Dummy config and weights saved to {dummy_config_path} and {dummy_weights_path}")

    try:
        print("\n--- Testing Single-Threaded (num_threads=0) ---")
        numpy_model_st = BertForSequenceClassificationNumpy(dummy_config_path, dummy_weights_path, num_threads=0)
        batch_size = 4
        seq_len = 10
        dummy_input_ids = np.random.randint(0, dummy_config['vocab_size'], size=(batch_size, seq_len))
        dummy_attention_mask = np.ones((batch_size, seq_len), dtype=np.int64)
        dummy_attention_mask[1, 7:] = 0
        dummy_attention_mask[3, 5:] = 0
        dummy_token_type_ids = np.zeros((batch_size, seq_len), dtype=np.int64)

        print(f"\nPerforming ST forward pass with input_ids shape {dummy_input_ids.shape}")
        logits_st = numpy_model_st.forward(dummy_input_ids, dummy_attention_mask, dummy_token_type_ids)
        print(f"ST Logits shape: {logits_st.shape}")
        print(f"ST Logits (first 2): {logits_st[:2]}")
        del numpy_model_st

        print("\n--- Testing Multi-Threaded (num_threads=2) ---")
        numpy_model_mt = BertForSequenceClassificationNumpy(dummy_config_path, dummy_weights_path, num_threads=2)
        
        print(f"\nPerforming MT forward pass with input_ids shape {dummy_input_ids.shape}")
        logits_mt = numpy_model_mt.forward(dummy_input_ids, dummy_attention_mask, dummy_token_type_ids)
        print(f"MT Logits shape: {logits_mt.shape}")
        print(f"MT Logits (first 2): {logits_mt[:2]}")

        # Check if results are identical
        assert np.allclose(logits_st, logits_mt, atol=1e-6), "Single-threaded and multi-threaded results differ!"
        print("\nSUCCESS: Single-threaded and multi-threaded results are close enough.")

        del numpy_model_mt # Explicitly delete

        print("\n--- Testing Multi-Threaded (num_threads=4) on batch_size=2 ---")
        batch_size_small = 2
        dummy_input_ids_small = dummy_input_ids[:batch_size_small]
        dummy_attention_mask_small = dummy_attention_mask[:batch_size_small]
        dummy_token_type_ids_small = dummy_token_type_ids[:batch_size_small]

        numpy_model_mt2 = BertForSequenceClassificationNumpy(dummy_config_path, dummy_weights_path, num_threads=4)
        print(f"\nPerforming MT forward pass with input_ids shape {dummy_input_ids_small.shape}")
        logits_mt2 = numpy_model_mt2.forward(dummy_input_ids_small, dummy_attention_mask_small, dummy_token_type_ids_small)
        print(f"MT2 Logits shape: {logits_mt2.shape}")
        print(f"MT2 Logits: {logits_mt2}")
        assert np.allclose(logits_st[:batch_size_small], logits_mt2, atol=1e-6), "ST and MT2 results differ!"
        print("\nSUCCESS: ST (subset) and MT2 (smaller batch, more threads) results are close enough.")

        del numpy_model_mt2


    except Exception as e:
        print(f"Error during dummy model test: {e}")
        import traceback
        traceback.print_exc()
    finally:
        print("\n(Dummy files kept for inspection: ./dummy_numpy_bert_model/)")
```

### 文件: `NumpyBertTokenizer.py`

```python
# NumpyBertTokenizer.py
import json
import re
import unicodedata
import numpy as np
import os

def load_vocab(vocab_file):
    """Loads a vocabulary file into a dictionary."""
    vocab = {}
    ids_to_tokens = {}
    with open(vocab_file, "r", encoding="utf-8") as reader:
        tokens_map = json.load(reader) # Expecting a JSON dict: {"token": id}
    for token, index in tokens_map.items():
        vocab[token] = index
        ids_to_tokens[index] = token
    return vocab, ids_to_tokens

class BasicTokenizer:
    """Runs basic tokenization (punctuation splitting, lower casing, etc.)."""

    def __init__(self, do_lower_case=True, never_split=None):
        self.do_lower_case = do_lower_case
        self.never_split = set(never_split) if never_split else set()

    def tokenize(self, text):
        """Basic Tokenization of a piece of text."""
        text = self._clean_text(text)
        # This was added on November 1st, 2018 for a bug fix
        # Previously, CJK characters were added spaces around them.
        text = self._tokenize_chinese_chars(text)

        orig_tokens = whitespace_tokenize(text)
        split_tokens = []
        for token in orig_tokens:
            if token in self.never_split:
                split_tokens.append(token)
                continue
            if self.do_lower_case:
                token = token.lower()
                token = self._run_strip_accents(token)
            
            # Split on punctuation
            split_tokens.extend(self._run_split_on_punc(token))

        output_tokens = whitespace_tokenize(" ".join(split_tokens))
        return output_tokens

    def _run_strip_accents(self, text):
        """Strips accents from a piece of text."""
        text = unicodedata.normalize("NFD", text)
        output = []
        for char in text:
            cat = unicodedata.category(char)
            if cat == "Mn": # Mark, Nonspacing
                continue
            output.append(char)
        return "".join(output)

    def _run_split_on_punc(self, text):
        """Splits punctuation on a piece of text."""
        if text in self.never_split:
            return [text]
        chars = list(text)
        i = 0
        start_new_word = True
        output = []
        while i < len(chars):
            char = chars[i]
            if _is_punctuation(char):
                output.append([char])
                start_new_word = True
            else:
                if start_new_word:
                    output.append([])
                start_new_word = False
                output[-1].append(char)
            i += 1
        return ["".join(x) for x in output]

    def _tokenize_chinese_chars(self, text):
        """Adds whitespace around any CJK character."""
        output = []
        for char in text:
            cp = ord(char)
            if self._is_chinese_char(cp):
                output.append(" ")
                output.append(char)
                output.append(" ")
            else:
                output.append(char)
        return "".join(output)

    def _is_chinese_char(self, cp):
        """Checks whether CP is the codepoint of a CJK character."""
        if (
            (cp >= 0x4E00 and cp <= 0x9FFF) # CJK Unified Ideographs
            or (cp >= 0x3400 and cp <= 0x4DBF) # CJK Unified Ideographs Extension A
            or (cp >= 0x20000 and cp <= 0x2A6DF) # CJK Unified Ideographs Extension B
            or (cp >= 0x2A700 and cp <= 0x2B73F) # CJK Unified Ideographs Extension C
            # ... and so on for other CJK blocks
        ):
            return True
        return False

    def _clean_text(self, text):
        """Performs invalid character removal and whitespace cleanup on text."""
        output = []
        for char in text:
            cp = ord(char)
            if cp == 0 or cp == 0xFFFD or _is_control(char):
                continue
            if _is_whitespace(char):
                output.append(" ")
            else:
                output.append(char)
        return "".join(output)


class WordpieceTokenizer:
    """Runs WordPiece tokenization."""

    def __init__(self, vocab, unk_token, max_input_chars_per_word=200):
        self.vocab = vocab
        self.unk_token = unk_token
        self.max_input_chars_per_word = max_input_chars_per_word

    def tokenize(self, text):
        """
        Tokenizes a piece of text into its word pieces.
        This uses a greedy longest-match-first algorithm to perform tokenization
        """
        output_tokens = []
        for token in whitespace_tokenize(text): # Should already be processed by BasicTokenizer
            chars = list(token)
            if len(chars) > self.max_input_chars_per_word:
                output_tokens.append(self.unk_token)
                continue

            is_bad = False
            start = 0
            sub_tokens = []
            while start < len(chars):
                end = len(chars)
                cur_substr = None
                while start < end:
                    substr = "".join(chars[start:end])
                    if start > 0: # If not the beginning of the word, add "##"
                        substr = "##" + substr
                    if substr in self.vocab:
                        cur_substr = substr
                        break
                    end -= 1
                
                if cur_substr is None: # No subtoken found
                    is_bad = True
                    break
                sub_tokens.append(cur_substr)
                # Move start to after the matched subtoken (remove "##" for length calculation)
                start = end # This was the original HuggingFace logic, `end` is already correct.

            if is_bad:
                output_tokens.append(self.unk_token)
            else:
                output_tokens.extend(sub_tokens)
        return output_tokens


def _is_whitespace(char):
    """Checks whether `char` is a whitespace character."""
    if char == " " or char == "\t" or char == "\n" or char == "\r":
        return True
    cat = unicodedata.category(char)
    if cat == "Zs": # Space separator
        return True
    return False

def _is_control(char):
    """Checks whether `char` is a control character."""
    if char == "\t" or char == "\n" or char == "\r": # These are often treated as whitespace.
        return False
    cat = unicodedata.category(char)
    if cat.startswith("C"): # "Cc" (Control), "Cf" (Format), "Cs" (Surrogate), "Co" (Private Use)
        return True
    return False

def _is_punctuation(char):
    """Checks whether `char` is a punctuation character."""
    cp = ord(char)
    if (cp >= 33 and cp <= 47) or (cp >= 58 and cp <= 64) or \
       (cp >= 91 and cp <= 96) or (cp >= 123 and cp <= 126):
        return True
    cat = unicodedata.category(char)
    if cat.startswith("P"): # Punctuation
        return True
    return False

def whitespace_tokenize(text):
    """Runs basic whitespace cleaning and splitting on a piece of text."""
    text = text.strip()
    if not text:
        return []
    tokens = text.split()
    return tokens


class NumpyBertTokenizer:
    def __init__(self, vocab_file, tokenizer_config_file):
        if not os.path.exists(vocab_file):
            raise FileNotFoundError(f"Vocab file not found: {vocab_file}")
        if not os.path.exists(tokenizer_config_file):
            raise FileNotFoundError(f"Tokenizer config file not found: {tokenizer_config_file}")

        self.vocab, self.ids_to_tokens = load_vocab(vocab_file)
        
        with open(tokenizer_config_file, "r", encoding="utf-8") as f:
            config = json.load(f)

        self.do_lower_case = config.get("do_lower_case", True)
        self.unk_token = config.get("unk_token", "[UNK]")
        self.sep_token = config.get("sep_token", "[SEP]")
        self.pad_token = config.get("pad_token", "[PAD]")
        self.cls_token = config.get("cls_token", "[CLS]")
        self.mask_token = config.get("mask_token", "[MASK]")
        self.model_max_length = config.get("max_len", 512) # from conversion.py

        # Ensure special tokens are in vocab, add them if they are not (though they should be)
        special_tokens = [self.unk_token, self.sep_token, self.pad_token, self.cls_token, self.mask_token]
        for st in special_tokens:
            if st not in self.vocab:
                # This is a fallback, ideally vocab.json from conversion.py is complete
                new_id = len(self.vocab)
                self.vocab[st] = new_id
                self.ids_to_tokens[new_id] = st
                print(f"Warning: Special token '{st}' not in vocab, added with ID {new_id}")


        self.unk_token_id = self.vocab[self.unk_token]
        self.sep_token_id = self.vocab[self.sep_token]
        self.pad_token_id = self.vocab[self.pad_token]
        self.cls_token_id = self.vocab[self.cls_token]
        self.mask_token_id = self.vocab[self.mask_token]

        never_split_tokens = [self.unk_token, self.sep_token, self.pad_token, self.cls_token, self.mask_token]
        # Add any other tokens from config that should never be split (e.g., from added_tokens_decoder)
        # For simplicity, we'll just use the main special tokens for now.

        self.basic_tokenizer = BasicTokenizer(do_lower_case=self.do_lower_case, never_split=never_split_tokens)
        self.wordpiece_tokenizer = WordpieceTokenizer(vocab=self.vocab, unk_token=self.unk_token)

    def tokenize(self, text):
        """Converts a string into a sequence of tokens."""
        split_tokens = []
        # First, use basic_tokenizer to handle lowercasing, accent stripping, and CJK char handling
        for token in self.basic_tokenizer.tokenize(text):
            # Then, for each token from basic_tokenizer, run wordpiece
            for sub_token in self.wordpiece_tokenizer.tokenize(token):
                split_tokens.append(sub_token)
        return split_tokens

    def convert_tokens_to_ids(self, tokens):
        """Converts a sequence of tokens into ids using the vocab."""
        return [self.vocab.get(token, self.unk_token_id) for token in tokens]

    def convert_ids_to_tokens(self, ids):
        """Converts a sequence of ids into tokens using the vocab."""
        return [self.ids_to_tokens.get(id_, self.unk_token) for id_ in ids]

    def _prepare_for_model(self,
                           ids,
                           pair_ids=None,
                           max_length=None,
                           padding_strategy="max_length", # "longest", "max_length", "do_not_pad"
                           truncation_strategy="longest_first", # "longest_first", "only_first", "only_second", "do_not_truncate"
                           add_special_tokens=True,
                           return_token_type_ids=True,
                           return_attention_mask=True):
        
        if max_length is None:
            max_length = self.model_max_length
        
        num_special_tokens_to_add = 0
        if add_special_tokens:
            num_special_tokens_to_add = 2 if pair_ids is None else 3

        # Truncation
        if truncation_strategy != "do_not_truncate":
            total_len = len(ids) + (len(pair_ids) if pair_ids else 0)
            if total_len > max_length - num_special_tokens_to_add:
                if pair_ids is None:
                    ids = ids[:max_length - num_special_tokens_to_add]
                else: # pair_ids is not None
                    if truncation_strategy == "longest_first":
                        # Truncate the longer sequence first
                        while len(ids) + len(pair_ids) > max_length - num_special_tokens_to_add:
                            if len(ids) > len(pair_ids):
                                ids.pop()
                            else:
                                pair_ids.pop()
                    elif truncation_strategy == "only_first":
                        ids = ids[:max(0, max_length - num_special_tokens_to_add - len(pair_ids))]
                    elif truncation_strategy == "only_second":
                        pair_ids = pair_ids[:max(0, max_length - num_special_tokens_to_add - len(ids))]
                    # else: do_not_truncate (already handled)
        
        # Add special tokens
        sequence = []
        token_type_ids = []

        if add_special_tokens:
            sequence.append(self.cls_token_id)
            token_type_ids.append(0)
        
        sequence.extend(ids)
        token_type_ids.extend([0] * len(ids))

        if pair_ids is not None:
            if add_special_tokens:
                sequence.append(self.sep_token_id)
                token_type_ids.append(0) # SEP for first sequence
            
            sequence.extend(pair_ids)
            token_type_ids.extend([1] * len(pair_ids)) # Segment B

        if add_special_tokens:
            sequence.append(self.sep_token_id)
            token_type_ids.append(1 if pair_ids is not None else 0) # SEP for last sequence

        input_ids = sequence
        
        # Padding
        attention_mask = []
        if padding_strategy != "do_not_pad":
            pad_len = 0
            if padding_strategy == "max_length":
                pad_len = max_length - len(input_ids)
            elif padding_strategy == "longest": # Not fully implemented, needs batch context
                pass # For single sequence, same as do_not_pad or max_length if len < max_len

            if pad_len > 0:
                input_ids.extend([self.pad_token_id] * pad_len)
                token_type_ids.extend([0] * pad_len) # Pad token_type_ids with 0
            
            # Ensure it does not exceed max_length if padding_strategy was "longest" and sequence was already long
            input_ids = input_ids[:max_length]
            token_type_ids = token_type_ids[:max_length]

        if return_attention_mask:
            attention_mask = [1 if id_ != self.pad_token_id else 0 for id_ in input_ids]

        result = {"input_ids": np.array(input_ids, dtype=np.int64)}
        if return_attention_mask:
            result["attention_mask"] = np.array(attention_mask, dtype=np.int64)
        if return_token_type_ids:
            result["token_type_ids"] = np.array(token_type_ids, dtype=np.int64)
            
        return result


    def __call__(self, 
                 text, 
                 text_pair=None, 
                 add_special_tokens=True, 
                 padding="max_length", # "longest", "max_length", "do_not_pad" or bool
                 truncation="longest_first", # "longest_first", "only_first", "only_second", "do_not_truncate" or bool
                 max_length=None,
                 return_tensors=None, # "np" for numpy, None for list
                 return_token_type_ids=True,
                 return_attention_mask=True):
        
        if isinstance(padding, bool):
            padding = "max_length" if padding else "do_not_pad"
        if isinstance(truncation, bool):
            truncation = "longest_first" if truncation else "do_not_truncate"

        if max_length is None:
            max_length = self.model_max_length

        tokens = self.tokenize(text)
        ids = self.convert_tokens_to_ids(tokens)

        pair_ids = None
        if text_pair is not None:
            pair_tokens = self.tokenize(text_pair)
            pair_ids = self.convert_tokens_to_ids(pair_tokens)

        output = self._prepare_for_model(ids,
                                         pair_ids=pair_ids,
                                         max_length=max_length,
                                         padding_strategy=padding,
                                         truncation_strategy=truncation,
                                         add_special_tokens=add_special_tokens,
                                         return_token_type_ids=return_token_type_ids,
                                         return_attention_mask=return_attention_mask)
        
        if return_tensors == "np":
            # Already numpy arrays from _prepare_for_model
            # Ensure they are 2D for batch processing (batch_size=1)
            for key in output:
                output[key] = np.expand_dims(output[key], axis=0)
            return output
        elif return_tensors is None:
            # Convert numpy arrays back to lists if no tensor type specified
            for key in output:
                output[key] = output[key].tolist()
            return output
        else:
            raise ValueError(f"Unsupported return_tensors type: {return_tensors}")


# --- Example Usage (for testing) ---
if __name__ == '__main__':
    # Create dummy vocab and config for testing
    dummy_vocab_data = {
        "[PAD]": 0, "[UNK]": 1, "[CLS]": 2, "[SEP]": 3, "[MASK]": 4,
        "hel": 5, "##lo": 6, "world": 7, "this": 8, "is": 9, "a": 10, "test": 11,
        "你好": 12, "世界": 13, "这": 14, "是": 15, "测": 16, "试": 17,
        "un":18, "##aff":19, "##able":20
    }
    dummy_tokenizer_config_data = {
        "do_lower_case": True,
        "unk_token": "[UNK]",
        "sep_token": "[SEP]",
        "pad_token": "[PAD]",
        "cls_token": "[CLS]",
        "mask_token": "[MASK]",
        "model_max_length": 30, # Renamed from max_len for consistency with HF
        "max_len": 30 # Keep this as conversion.py might produce it
    }

    # Create dummy directory and files
    dummy_dir = "./dummy_numpy_tokenizer_files"
    os.makedirs(dummy_dir, exist_ok=True)
    vocab_path = os.path.join(dummy_dir, "vocab.json")
    config_path = os.path.join(dummy_dir, "tokenizer_config.json")

    with open(vocab_path, "w", encoding="utf-8") as f:
        json.dump(dummy_vocab_data, f)
    with open(config_path, "w", encoding="utf-8") as f:
        json.dump(dummy_tokenizer_config_data, f)

    print(f"Dummy vocab and config saved to {dummy_dir}")

    try:
        print("\nLoading NumpyBertTokenizer with dummy data...")
        tokenizer = NumpyBertTokenizer(vocab_path, config_path)
        print("Tokenizer loaded.")

        print("\n--- Test Case 1: Simple English ---")
        text1 = "Hello world this is a test"
        tokens1 = tokenizer.tokenize(text1)
        print(f"Text: '{text1}'")
        print(f"Tokens: {tokens1}")
        ids1 = tokenizer.convert_tokens_to_ids(tokens1)
        print(f"IDs: {ids1}")
        encoded1 = tokenizer(text1, return_tensors="np", max_length=12)
        print(f"Encoded (max_length=12): {encoded1}")
        print(f"Decoded: {tokenizer.convert_ids_to_tokens(encoded1['input_ids'][0])}")


        print("\n--- Test Case 2: Chinese ---")
        text2 = "你好 世界 这是测试"
        tokens2 = tokenizer.tokenize(text2)
        print(f"Text: '{text2}'")
        print(f"Tokens: {tokens2}") # Expected: ['你', '好', '世', '界', '这', '是', '测', '试']
        ids2 = tokenizer.convert_tokens_to_ids(tokens2)
        print(f"IDs: {ids2}")
        encoded2 = tokenizer(text2, return_tensors="np", max_length=10)
        print(f"Encoded (max_length=10): {encoded2}")
        print(f"Decoded: {tokenizer.convert_ids_to_tokens(encoded2['input_ids'][0])}")

        print("\n--- Test Case 3: OOV and WordPiece ---")
        text3 = "unaffable Xenomorph" # Xenomorph is OOV
        tokens3 = tokenizer.tokenize(text3)
        print(f"Text: '{text3}'")
        print(f"Tokens: {tokens3}") # Expected: ['un', '##aff', '##able', '[UNK]']
        ids3 = tokenizer.convert_tokens_to_ids(tokens3)
        print(f"IDs: {ids3}")
        encoded3 = tokenizer(text3, return_tensors="np", max_length=8)
        print(f"Encoded (max_length=8): {encoded3}")
        print(f"Decoded: {tokenizer.convert_ids_to_tokens(encoded3['input_ids'][0])}")
        
        print("\n--- Test Case 4: Pair of sequences with truncation ---")
        text_a = "Hello world this is a long sentence for the first part."
        text_b = "And this is a shorter second part."
        # max_length for tokenizer config is 30
        # CLS + SEP + SEP = 3 special tokens
        # Available length = 30 - 3 = 27
        # tokens_a (lower): ['hel', '##lo', 'world', 'this', 'is', 'a', 'long', 'sentence', 'for', 'the', 'first', 'part', '.'] (13 tokens)
        # tokens_b (lower): ['and', 'this', 'is', 'a', 'shorter', 'second', 'part', '.'] (8 tokens)
        # Total = 13 + 8 = 21. Fits within 27.
        
        encoded_pair = tokenizer(text_a, text_pair=text_b, return_tensors="np", max_length=20) # Force truncation
        # Max length 20. Special tokens = 3. Available = 17.
        # Original: len(text_a_tokens)=13, len(text_b_tokens)=8. Total 21. Need to truncate 4.
        # Longest first: text_a loses 4 tokens. -> len(text_a_tokens)=9, len(text_b_tokens)=8. Total 17.
        print(f"Encoded pair (max_length=20): {encoded_pair}")
        print(f"Input IDs: {encoded_pair['input_ids'][0]}")
        print(f"Token Type IDs: {encoded_pair['token_type_ids'][0]}")
        print(f"Attention Mask: {encoded_pair['attention_mask'][0]}")
        print(f"Decoded: {tokenizer.convert_ids_to_tokens(encoded_pair['input_ids'][0])}")
        
        # Test without explicit max_length, should use from config (30)
        encoded_pair_default_max = tokenizer(text_a, text_pair=text_b, return_tensors="np")
        print(f"Encoded pair (default max_length={tokenizer.model_max_length}):")
        print(f"Input IDs shape: {encoded_pair_default_max['input_ids'].shape}")
        print(f"Decoded: {tokenizer.convert_ids_to_tokens(encoded_pair_default_max['input_ids'][0])}")


    except Exception as e:
        print(f"Error during dummy tokenizer test: {e}")
        import traceback
        traceback.print_exc()
    finally:
        # Clean up dummy files
        # if os.path.exists(vocab_path): os.remove(vocab_path)
        # if os.path.exists(config_path): os.remove(config_path)
        # if os.path.exists(dummy_dir): os.rmdir(dummy_dir)
        print(f"\n(Dummy files kept for inspection: {dummy_dir})")
```

### 文件: `addData.py`

```python
# emotion_data_entry.py
import pandas as pd
import os

def add_to_csv():
    """交互式添加情绪数据到CSV文件"""
    # 检查文件是否存在
    file_path = "emotion_data_manual.csv"
    if os.path.exists(file_path):
        df = pd.read_csv(file_path)
        print(f"\n当前已有数据: {len(df)}条")
    else:
        df = pd.DataFrame(columns=["text", "label"])
        print("新建数据文件...")

    # 可用情绪列表
    emotions = [
        "不屑", "不知所措", "担心", "尴尬", "紧张", "高兴",
        "自信", "害怕", "很羞涩", "害羞", "认真", "生气",
        "说话", "无语", "厌恶", "反感", "疑惑", "正常"
    ]

    while True:
        print("\n" + "="*30)
        print("当前可用情绪标签:")
        print(", ".join(emotions))
        print("="*30)

        # 输入文本
        text = input("请输入文本内容（输入q退出）: ").strip()
        if text.lower() == 'q':
            break

        # 输入标签
        while True:
            label = input("请输入情绪标签: ").strip()
            if label in emotions:
                break
            print(f"无效标签！请从以下选择: {', '.join(emotions)}")

        # 添加到DataFrame
        new_row = {"text": text, "label": label}
        df = pd.concat([df, pd.DataFrame([new_row])], ignore_index=True)

        # 保存到CSV
        df.to_csv(file_path, index=False, encoding='utf-8-sig')
        print(f"已添加: 「{text}」→ {label} (当前总数: {len(df)})")

if __name__ == "__main__":
    print("=== 情绪数据录入工具 ===")
    print("说明: 逐条添加文本和对应情绪标签")
    add_to_csv()
    print("\n数据已保存到 emotion_data_manual.csv")
```

### 文件: `conversion.py`

```python
import torch
import numpy as np
import json
import os
from transformers import BertForSequenceClassification, BertTokenizer, BertConfig

def convert_huggingface_model_to_numpy(model_path="./emotion_model_18emo", output_dir="./numpy_bert_model"):
    """
    加载 Hugging Face BERT 模型和分词器，并将其权重和配置转换为 NumPy 格式和 JSON 文件。

    Args:
        model_path (str): 原始 Hugging Face 模型保存的路径。
        output_dir (str): 转换后的 NumPy 权重和配置文件的保存路径。
    """
    os.makedirs(output_dir, exist_ok=True)
    print(f"创建输出目录: {output_dir}")

    # --- 1. 转换模型权重 ---
    print("\n--- 正在转换模型权重 ---")
    try:
        # 加载模型
        model = BertForSequenceClassification.from_pretrained(model_path)
        print(f"成功加载 Hugging Face 模型: {model_path}")

        # 获取模型状态字典（包含所有权重和偏置）
        state_dict = model.state_dict()
        numpy_weights = {}

        # 将 PyTorch Tensor 转换为 NumPy 数组
        for key, value in state_dict.items():
            numpy_weights[key] = value.cpu().numpy()
            # print(f"转换权重: {key}, 形状: {numpy_weights[key].shape}")

        # 保存为 .npz 文件 (NumPy 压缩文件，可以保存多个数组)
        weights_output_path = os.path.join(output_dir, "model_weights.npz")
        np.savez_compressed(weights_output_path, **numpy_weights)
        print(f"模型权重已保存到: {weights_output_path}")

    except Exception as e:
        print(f"加载或转换模型权重失败: {e}")
        return

    # --- 2. 转换模型配置 ---
    print("\n--- 正在转换模型配置 ---")
    try:
        # 加载模型配置
        config = BertConfig.from_pretrained(model_path)
        model_config = config.to_dict()

        # 移除一些不必要的或 PyTorch 特有的配置项
        keys_to_remove = ["_name_or_path", "torch_dtype", "architectures", "problem_type"]
        for key in keys_to_remove:
            model_config.pop(key, None)

        # 确保id2label和label2id是字符串key，方便JSON保存和后续使用
        if "id2label" in model_config:
            model_config["id2label"] = {str(k): v for k, v in model_config["id2label"].items()}
        if "label2id" in model_config:
            model_config["label2id"] = {k: v for k, v in model_config["label2id"].items()}

        # 确保包含num_labels字段
        if "num_labels" not in model_config and "id2label" in model_config:
            model_config["num_labels"] = len(model_config["id2label"])
            print(f"添加num_labels字段: {model_config['num_labels']}")

        config_output_path = os.path.join(output_dir, "model_config.json")
        with open(config_output_path, "w", encoding="utf-8") as f:
            json.dump(model_config, f, ensure_ascii=False, indent=4)
        print(f"模型配置已保存到: {config_output_path}")

    except Exception as e:
        print(f"加载或转换模型配置失败: {e}")
        return

    # --- 3. 转换分词器配置和词汇表 ---
    print("\n--- 正在转换分词器配置和词汇表 ---")
    try:
        # 加载分词器
        tokenizer = BertTokenizer.from_pretrained(model_path)
        print(f"成功加载 Hugging Face 分词器: {model_path}")

        # 保存词汇表 (vocab.txt 已经是文本格式，但我们可以将其转换为 JSON 格式的字典)
        vocab_output_path = os.path.join(output_dir, "vocab.json")
        with open(vocab_output_path, "w", encoding="utf-8") as f:
            json.dump(tokenizer.vocab, f, ensure_ascii=False, indent=4)
        print(f"词汇表已保存到: {vocab_output_path}")

        # 保存分词器配置
        tokenizer_config_path = os.path.join(model_path, "tokenizer_config.json")
        special_tokens_map_path = os.path.join(model_path, "special_tokens_map.json")

        tokenizer_config = {}
        if os.path.exists(tokenizer_config_path):
            with open(tokenizer_config_path, "r", encoding="utf-8") as f:
                tokenizer_config.update(json.load(f))
        if os.path.exists(special_tokens_map_path):
            with open(special_tokens_map_path, "r", encoding="utf-8") as f:
                tokenizer_config.update(json.load(f))
        
        tokenizer_config["unk_token"] = tokenizer.unk_token
        tokenizer_config["sep_token"] = tokenizer.sep_token
        tokenizer_config["pad_token"] = tokenizer.pad_token
        tokenizer_config["cls_token"] = tokenizer.cls_token
        tokenizer_config["mask_token"] = tokenizer.mask_token
        tokenizer_config["do_lower_case"] = tokenizer.do_lower_case
        tokenizer_config["max_len"] = tokenizer.model_max_length

        tokenizer_config_output_path = os.path.join(output_dir, "tokenizer_config.json")
        with open(tokenizer_config_output_path, "w", encoding="utf-8") as f:
            json.dump(tokenizer_config, f, ensure_ascii=False, indent=4)
        print(f"分词器配置已保存到: {tokenizer_config_output_path}")

    except Exception as e:
        print(f"加载或转换分词器配置失败: {e}")
        return

    print("\n--- 所有转换完成！ ---")
    print(f"转换后的文件位于目录: {output_dir}")
    print("下一步：您可以开始在 BERT.py 中使用这些 NumPy 权重和配置来实现 BERT 模型。")

if __name__ == "__main__":
    model_source_path = "./emotion_model_18emo"
    numpy_output_directory = "./numpy_bert_model"

    if not os.path.exists(model_source_path):
        print(f"错误: 找不到模型源目录 '{model_source_path}'。请先运行 'startTrain.py' 训练模型。")
    else:
        convert_huggingface_model_to_numpy(model_source_path, numpy_output_directory)
```

### 文件: `csvCleaner.py`

```python
import re
import pandas as pd
from unicodedata import normalize

def deep_clean_text(text):
    """
    深度清洗文本的核武器函数
    处理：不可见字符、异常空格、BOM头、零宽字符、HTML标签等
    """
    if not isinstance(text, str):
        text = str(text)
    
    # 1. 标准化Unicode字符（如全角转半角）
    text = normalize('NFKC', text)
    
    # 2. 移除不可打印字符（保留中文、英文、数字、常用标点）
    text = ''.join(
        char for char in text 
        if char.isprintable() or 
           '\u4e00' <= char <= '\u9fff'  # 保留所有汉字
    )
    
    # 3. 处理特殊空白字符
    text = re.sub(r'[\u200b-\u200d\u2028-\u202f\ufeff]', '', text)  # 零宽字符
    
    # 4. 替换异常空格（包含全角空格）
    text = re.sub(r'[\s\u3000]+', ' ', text).strip()
    
    # 5. 移除控制字符（ASCII 0-31）
    text = re.sub(r'[\x00-\x1f\x7f]', '', text)
    
    # 6. 处理HTML/XML标签（如果有）
    text = re.sub(r'<[^>]+>', '', text)
    
    # 7. 处理URL和特殊符号
    text = re.sub(r'https?://\S+|www\.\S+', '', text)
    text = re.sub(r'[【】■◆▶☞➤]+', '', text)  # 去除装饰性符号
    
    return text

def clean_csv_file(input_path, output_path):
    """
    清洗整个CSV文件
    :param input_path: 输入文件路径
    :param output_path: 输出文件路径
    """
    # 读取数据（强制UTF-8编码，处理BOM头）
    try:
        df = pd.read_csv(input_path, encoding='utf-8-sig')
    except UnicodeDecodeError:
        df = pd.read_csv(input_path, encoding='gbk')
    
    # 检查必要列是否存在
    assert 'text' in df.columns, "CSV文件必须包含'text'列"
    
    # 深度清洗
    print("正在清洗数据...")
    df['text'] = df['text'].astype(str).apply(deep_clean_text)
    
    # 移除空文本
    df = df[df['text'].str.strip().astype(bool)]
    
    # 保存清洗后的数据
    df.to_csv(output_path, index=False, encoding='utf-8-sig')
    print(f"清洗完成！已保存到 {output_path}")
    
    # 打印清洗报告
    sample_report = df.sample(min(5, len(df)))
    print("\n清洗后样本示例:")
    print(sample_report[['text']].to_markdown(index=False))

if __name__ == "__main__":
    input_file = "emotion_data_manual.csv"  # 原始数据路径
    output_file = "emotion_data_manual.csv"  # 清洗后保存路径
    
    clean_csv_file(input_file, output_file)
    
    # 验证清洗效果
    print("\n验证清洗结果:")
    test_samples = [
        "  Hello\u200bWorld！  ",  # 含零宽空格
        "NUL\x00字符测试",         # 含控制字符
        "全角　空格\u3000测试",     # 全角空格
        "<p>HTML标签</p>",         # HTML标签
        "https://example.com 网址"  # URL
    ]
    
    print("\n清洗测试案例:")
    for sample in test_samples:
        cleaned = deep_clean_text(sample)
        print(f"原始: {repr(sample)} → 清洗后: {repr(cleaned)}")
```

### 文件: `inference.py`

```python
import numpy as np
import json
import os

try:
    from BERT import BertForSequenceClassificationNumpy, softmax
    from NumpyBertTokenizer import NumpyBertTokenizer
except ImportError:
    print("错误：无法导入 BERT.py 或 NumpyBertTokenizer.py。请确保这些文件与 inference.py 在同一目录下，或者在 Python 路径中。")
    exit(1)

class NumpyEmotionClassifier:
    def __init__(self, model_dir="./numpy_bert_model"):
        """
        加载基于NumPy的BERT模型和分词器。
        """
        print(f"正在从 '{model_dir}' 加载Numpy模型和分词器...")

        model_config_path = os.path.join(model_dir, "model_config.json")
        model_weights_path = os.path.join(model_dir, "model_weights.npz")
        tokenizer_vocab_path = os.path.join(model_dir, "vocab.json")
        tokenizer_config_path = os.path.join(model_dir, "tokenizer_config.json")

        # 检查所有必要文件是否存在
        required_files = [model_config_path, model_weights_path, tokenizer_vocab_path, tokenizer_config_path]
        for f_path in required_files:
            if not os.path.exists(f_path):
                raise FileNotFoundError(f"错误：必需文件未找到 - {f_path}。请先运行 conversion.py。")

        # 加载模型配置 (包含 id2label)
        with open(model_config_path, "r", encoding="utf-8") as f:
            self.model_config = json.load(f)
        
        self.id2label = self.model_config.get("id2label")
        if not self.id2label:
            original_model_label_map_path = "./emotion_model_18emo/label_mapping.json" # 假设 conversion.py 基于此
            if os.path.exists(original_model_label_map_path):
                print(f"警告：model_config.json 中未找到 id2label，尝试从 {original_model_label_map_path} 加载。")
                with open(original_model_label_map_path, "r", encoding="utf-8") as f:
                    label_map_data = json.load(f)
                    self.id2label = label_map_data.get("id2label")
            if not self.id2label:
                 raise ValueError("错误：在模型配置或备用路径中均未找到 'id2label' 映射。")

        print("\n加载的标签映射关系:")
        for id_key, label in self.id2label.items():
            print(f"{id_key}: {label}")

        # 初始化分词器
        self.tokenizer = NumpyBertTokenizer(
            vocab_file=tokenizer_vocab_path,
            tokenizer_config_file=tokenizer_config_path
        )
        # 获取分词器的最大长度配置
        self.max_length = self.tokenizer.model_max_length
        print(f"分词器最大长度设置为: {self.max_length}")


        # 初始化模型
        self.model = BertForSequenceClassificationNumpy(
            model_config_path=model_config_path,
            model_weights_path=model_weights_path
        )
        print("Numpy模型和分词器加载成功。")


    def predict(self, text, confidence_threshold=0.2):
        """
        预测文本情绪（带置信度阈值过滤）
        """
        # 1. 编码输入
        # NumpyBertTokenizer.__call__ 应该返回一个字典，包含 'input_ids', 'attention_mask', 'token_type_ids'
        # 并且这些值应该是 NumPy 数组，形状为 (batch_size, seq_len)
        encoded_input = self.tokenizer(
            text,
            padding="max_length", # 或 "longest"
            truncation="longest_first", # 或 True
            max_length=self.max_length, # 使用从tokenizer配置中获取的max_length
            return_tensors="np" # 确保返回 NumPy 数组
        )

        input_ids = encoded_input['input_ids']
        attention_mask = encoded_input['attention_mask']
        token_type_ids = encoded_input.get('token_type_ids') # token_type_ids 是可选的

        # 2. 模型推理
        # BertForSequenceClassificationNumpy.forward 接收这些 NumPy 数组
        logits = self.model.forward(
            input_ids=input_ids,
            attention_mask=attention_mask,
            token_type_ids=token_type_ids
        ) # logits 形状: (batch_size, num_labels)

        # 3. 计算概率 (Softmax)
        # BERT.py 中定义的 softmax 函数
        probs = softmax(logits, axis=-1) # probs 形状: (batch_size, num_labels)

        # 假设 batch_size = 1
        probs_single = probs[0] # (num_labels,)

        # 4. 处理结果
        pred_id = np.argmax(probs_single)
        pred_prob = probs_single[pred_id]

        # 获取Top3结果
        top3 = self._get_top_k(probs_single, k=3)
        
        # 低置信度处理
        # 注意：self.id2label 的键可能是字符串 "0", "1", ...
        predicted_label_str = self.id2label.get(str(pred_id), f"未知标签ID_{pred_id}")

        if pred_prob < confidence_threshold:
            return {
                "label": "不确定",
                "confidence": float(pred_prob),
                "top3": top3,
                "warning": f"置信度 ({pred_prob:.2%}) 低于阈值 ({confidence_threshold:.0%})，最可能的标签是 '{predicted_label_str}'"
            }
        
        return {
            "label": predicted_label_str,
            "confidence": float(pred_prob),
            "top3": top3
        }

    def _get_top_k(self, probs_array, k=3):
        """
        获取概率最高的k个结果。
        probs_array: 1D NumPy 数组，包含单个样本的各类别概率。
        """
        # argsort 返回的是排序后的索引，从小到大
        # 所以取最后k个，并反转顺序得到从大到小的索引
        top_k_ids = np.argsort(probs_array)[-k:][::-1]
        
        results = []
        for i in range(min(k, len(top_k_ids))):
            pred_id = top_k_ids[i]
            probability = probs_array[pred_id]
            label = self.id2label.get(str(pred_id), f"未知标签ID_{pred_id}") # 使用 str(pred_id)
            results.append({
                "label": label,
                "probability": float(probability)
            })
        return results

def main():
    # 从 startTrain.py 获取 TARGET_EMOTIONS 作为参考，或直接从加载的 id2label 生成
    # 这里我们依赖加载的 id2label
    
    print("【Numpy实现的18类情绪分类器】") # 假设是18类，具体取决于模型
    print("="*40)
    
    # 初始化分类器
    model_directory = "./numpy_bert_model"
    try:
        classifier = NumpyEmotionClassifier(model_dir=model_directory)
        num_labels_loaded = len(classifier.id2label)
        print(f"情绪类别数量 (根据加载的id2label): {num_labels_loaded}")
        print("="*40)
        print("\n模型加载成功！输入文本进行分析，输入 ':q' 退出。")
    except FileNotFoundError as e:
        print(f"\n模型加载失败: {str(e)}")
        print("请确保：")
        print(f"1. 模型目录 '{model_directory}' 存在。")
        print(f"2. 该目录包含 'model_config.json', 'model_weights.npz', 'tokenizer_config.json', 'vocab.json'。")
        print("3. 您已经成功运行了 'conversion.py' 脚本将HuggingFace模型转换为Numpy格式。")
        return
    except Exception as e:
        print(f"\n模型加载时发生未知错误: {str(e)}")
        import traceback
        traceback.print_exc()
        return
    
    while True:
        try:
            text = input("\n请输入要分析的文本: ").strip()
            
            if text.lower() in [':q', ':quit', 'exit', '退出']:
                print("\n正在退出程序...")
                break
                
            if not text:
                print("输入不能为空！请输入有效文本。")
                continue
                
            result = classifier.predict(text)
            
            print("\n" + "="*30 + " 分析结果 " + "="*30)
            print(f"📝 文本: \"{text}\"")
            
            if "warning" in result:
                print(f"⚠️  {result['warning']}")
            
            print(f"🎯 主要情绪: {result['label']} (置信度: {result['confidence']:.2%})")
            
            if result['top3'] and (result['label'] != "不确定" or "warning" in result) :
                print("\n详细概率分布 (Top 3):")
                for i, item in enumerate(result["top3"], 1):
                    # 如果主情绪是“不确定”，但有警告（意味着有一个最可能的标签），则第一个top3就是那个最可能的
                    # 否则，如果主情绪不是“不确定”，则第一个top3就是主情绪
                    is_primary = (item['label'] == result['label']) or \
                                 (result['label'] == "不确定" and "warning" in result and i == 1)

                    print(f"  {i}. {item['label']}: {item['probability']:.2%}{' (主要)' if is_primary and result['label'] != '不确定' else ''}")
            print("="* (30 + 10 + 30)) # 匹配上面的长度
            
        except KeyboardInterrupt:
            print("\n检测到中断，正在退出程序...")
            break
        except Exception as e:
            print(f"\n❌ 预测过程中发生错误: {str(e)}")
            import traceback
            traceback.print_exc() # 打印详细错误信息以供调试

if __name__ == "__main__":
    main()
```

### 文件: `startTrain.py`

```python
# emotion_classifier_18emo.py # <-- 文件名建议也更新
import torch
import pandas as pd
import numpy as np
from sklearn.model_selection import train_test_split
from sklearn.preprocessing import LabelEncoder
from sklearn.metrics import classification_report
from transformers import (
    BertTokenizer,
    BertForSequenceClassification,
    Trainer,
    TrainingArguments
)
import os
import json

# 固定随机种子保证可复现
SEED = 42
np.random.seed(SEED)
torch.manual_seed(SEED)

# 明确定义18类情绪及其顺序（关键！）
TARGET_EMOTIONS = ["高兴", "厌恶", "害羞", "害怕",
                   "生气", "认真", "紧张", "慌张",
                   "疑惑", "兴奋", "无奈", "担心",
                   "惊讶", "哭泣", "心动", "难为情", "自信", "调皮"]
NUM_LABELS = len(TARGET_EMOTIONS) # 获取标签数量

def load_data(data_path="emotion_data_manual.csv"):
    """加载并预处理数据，强制使用定义的情绪标签"""
    # 加载数据并筛选目标情绪
    try:
        data = pd.read_csv(data_path)
    except FileNotFoundError:
        print(f"错误：数据文件 '{data_path}' 未找到。请确保文件存在于正确路径。")
        exit() # 或者引发异常
    
    # 筛选有效标签，并转换为字符串以防万一
    data["label"] = data["label"].astype(str)
    data = data[data["label"].isin(TARGET_EMOTIONS)].copy() # 使用 .copy() 避免 SettingWithCopyWarning
    data["text"] = data["text"].astype(str)

    if data.empty:
        print(f"错误：在 '{data_path}' 中没有找到属于 TARGET_EMOTIONS 的数据。")
        exit()

    # 数据统计
    print("\n=== 数据统计 ===")
    print(f"目标情绪类别数量: {NUM_LABELS}")
    print("筛选后总样本数:", len(data))
    print("类别分布:\n", data["label"].value_counts())

    # 使用固定顺序的标签编码器
    label_encoder = LabelEncoder()
    label_encoder.fit(TARGET_EMOTIONS)  # 强制按定义顺序编码

    # 划分数据集（保证测试集至少包含每个类别一个样本，如果可能）
    # 计算最小测试集比例以包含所有类
    min_samples_per_class = 1
    required_test_samples = NUM_LABELS * min_samples_per_class
    min_test_size_for_all_classes = required_test_samples / len(data)

    # 设置测试集比例，通常在0.1到0.3之间，但要确保能覆盖所有类
    test_size = max(0.2, min(min_test_size_for_all_classes, 0.3))
    # 如果总样本太少，可能无法满足 stratify 要求，这里简化处理
    if len(data) < NUM_LABELS * 2: # 至少保证训练集和测试集每个类都有样本（理论上）
         print("警告：数据量过少，可能无法有效分层或训练。")
         test_size = max(0.1, min_test_size_for_all_classes) # 尝试减少测试集比例

    print(f"实际使用的测试集比例: {test_size:.2f}")

    try:
        train_texts, test_texts, train_labels, test_labels = train_test_split(
            data["text"].tolist(),
            data["label"].tolist(),
            test_size=test_size,
            stratify=data["label"], # 尝试分层抽样
            random_state=SEED
        )
    except ValueError as e:
        print(f"分层抽样失败: {e}. 可能某些类别样本过少。尝试非分层抽样...")
        # 如果分层失败（通常因为某类样本太少），退回到普通随机抽样
        train_texts, test_texts, train_labels, test_labels = train_test_split(
            data["text"].tolist(),
            data["label"].tolist(),
            test_size=test_size,
            random_state=SEED
        )


    # 编码标签
    train_labels_encoded = label_encoder.transform(train_labels)
    test_labels_encoded = label_encoder.transform(test_labels)

    print(f"\n划分结果: 训练集={len(train_texts)}, 测试集={len(test_texts)}")
    print("测试集类别分布:\n", pd.Series(test_labels).value_counts().sort_index())
    # 检查测试集是否包含所有类别
    test_unique_labels = set(test_labels)
    if len(test_unique_labels) < NUM_LABELS:
        print(f"警告：测试集仅包含 {len(test_unique_labels)}/{NUM_LABELS} 个类别。缺失的类别：{set(TARGET_EMOTIONS) - test_unique_labels}")

    print("标签映射:", dict(zip(label_encoder.classes_, label_encoder.transform(label_encoder.classes_))))

    return train_texts, test_texts, train_labels_encoded, test_labels_encoded, label_encoder

class EmotionDataset(torch.utils.data.Dataset):
    """自定义数据集类"""
    # *** FIX: Renamed 'init' to '__init__' ***
    def __init__(self, encodings, labels):
        self.encodings = encodings
        self.labels = labels

    def __getitem__(self, idx):
        # Ensure encodings contain torch tensors or convert numpy arrays
        item = {key: torch.as_tensor(val[idx]) for key, val in self.encodings.items()}
        item["labels"] = torch.as_tensor(self.labels[idx])
        return item

    def __len__(self):
        return len(self.labels)

def train_and_evaluate():
    """训练和评估18类情绪分类模型""" # <-- 更新注释
    # 1. 加载数据
    train_texts, test_texts, train_labels, test_labels, label_encoder = load_data()

    # 2. 初始化模型和分词器
    model_name = "bert-base-chinese"
    tokenizer = BertTokenizer.from_pretrained(model_name, use_fast=False)
    model = BertForSequenceClassification.from_pretrained(
        model_name,
        num_labels=NUM_LABELS,  # 使用变量 NUM_LABELS
        id2label={i: label for i, label in enumerate(label_encoder.classes_)},
        label2id={label: i for i, label in enumerate(label_encoder.classes_)}
    )

    # 3. 数据编码
    print("\nTokenizing 数据...")
    # 使用 tolist() 确保输入是 list of strings
    train_encodings = tokenizer(
        train_texts,
        truncation=True,
        padding="max_length",
        max_length=128,
        # return_tensors="pt" # Trainer 会处理 tensor 转换, 这里可以不指定或者指定 None
    )
    test_encodings = tokenizer(
        test_texts,
        truncation=True,
        padding="max_length",
        max_length=128,
        # return_tensors="pt"
    )

    # 4. 创建数据集
    train_dataset = EmotionDataset(train_encodings, train_labels)
    test_dataset = EmotionDataset(test_encodings, test_labels)

    # 5. 训练配置（优化后的超参数）
    # *** FIX: Updated output_dir name ***
    output_dir_base = "./results_18emo"
    training_args = TrainingArguments(
        output_dir=output_dir_base,        # 输出目录
        num_train_epochs=8,                # 根据需要调整 epoch
        per_device_train_batch_size=16,    # 根据显存调整 batch size
        per_device_eval_batch_size=32,
        learning_rate=2e-5,               # 适合 fine-tuning 的学习率
        weight_decay=0.01,                 # L2 正则化
        warmup_ratio=0.1,                  # 预热比例
        evaluation_strategy="epoch",
        save_strategy="epoch",
        load_best_model_at_end=True,
        metric_for_best_model="f1_weighted", # 按加权 F1 选择最佳模型
        logging_dir=f'{output_dir_base}/logs', # 指定日志目录
        logging_steps=50,
        seed=SEED,
        fp16=torch.cuda.is_available(),    # 如果可用，自动启用混合精度
        report_to="none"                   # 禁用 wandb 等外部报告
    )

    # 6. 自定义评估指标
    def compute_metrics(pred):
        labels = pred.label_ids
        preds = pred.predictions.argmax(-1)
        # 使用 label_encoder.classes_ 获取正确的标签名称顺序
        report = classification_report(
            labels, preds,
            target_names=label_encoder.classes_,
            output_dict=True,
            zero_division=0 # 处理某个类别在预测或真实标签中都没有出现的情况
        )
        # 返回 Trainer 需要的指标
        return {
            "accuracy": report["accuracy"],
            "f1_weighted": report["weighted avg"]["f1-score"],
            "precision_weighted": report["weighted avg"]["precision"],
            "recall_weighted": report["weighted avg"]["recall"],
        }

    # 7. 训练
    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=test_dataset,
        compute_metrics=compute_metrics,
        tokenizer=tokenizer # 传递 tokenizer 方便 Trainer 处理 padding
    )

    print("\n开始训练...")
    trainer.train()

    # 8. 最终评估 (加载最好的模型进行评估)
    print("\n=== 测试集最终性能 (使用最佳模型) ===")
    eval_results = trainer.evaluate(test_dataset)
    print(f"评估结果: {eval_results}")

    # 获取详细的分类报告
    print("\n详细分类报告:")
    predictions = trainer.predict(test_dataset)
    y_pred = np.argmax(predictions.predictions, axis=1)
    print(classification_report(
        test_labels, # 使用原始编码的 test_labels
        y_pred,
        target_names=label_encoder.classes_, # 使用正确的标签名称
        digits=4
    ))

    # 9. 保存模型和配置
    # *** FIX: Updated output_dir name ***
    final_model_dir = "./emotion_model_18emo"
    os.makedirs(final_model_dir, exist_ok=True)
    print(f"\n保存最佳模型到 {final_model_dir}...")
    trainer.save_model(final_model_dir) # 保存最佳模型、tokenizer配置、训练状态等
    tokenizer.save_pretrained(final_model_dir) # 确保 tokenizer 也保存

    # 保存标签映射
    label_mapping_path = os.path.join(final_model_dir, "label_mapping.json")
    print(f"保存标签映射到 {label_mapping_path}...")
    # *** FIX: Added encoding='utf-8' to open() ***
    with open(label_mapping_path, "w", encoding="utf-8") as f:
        json.dump({
            # 确保 key 是字符串，因为 JSON 的 key 必须是 string
            "id2label": {str(i): label for i, label in enumerate(label_encoder.classes_)},
            "label2id": {label: i for i, label in enumerate(label_encoder.classes_)}
        }, f, ensure_ascii=False, indent=2)

    print(f"\n模型和配置已保存到 {final_model_dir}")

# *** FIX: Corrected if __name__ == "__main__": ***
if __name__ == "__main__":
    train_and_evaluate()
```

### 文件: `testModel.py`

```python
# emotion_interactive_8emo.py
from transformers import BertTokenizer, BertForSequenceClassification
import torch
import os
import json

class EmotionClassifier:
    def __init__(self, model_path="./emotion_model_18emo"):
        """加载8类情绪分类模型"""
        # 加载模型和分词器
        self.tokenizer = BertTokenizer.from_pretrained(model_path)
        self.model = BertForSequenceClassification.from_pretrained(model_path)
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        self.model.to(self.device)
        
        # 从保存的配置加载标签映射
        config_path = os.path.join(model_path, "label_mapping.json")
        with open(config_path, "r", encoding='utf-8') as f: 
            label_config = json.load(f)
        self.id2label = label_config["id2label"]
        self.label2id = label_config["label2id"]
        
        # 打印加载的标签映射
        print("\n加载的标签映射关系:")
        for id, label in self.id2label.items():
            print(f"{id}: {label}")

    def predict(self, text, confidence_threshold=0.2):
        """预测文本情绪（带置信度阈值过滤）"""
        # 编码输入
        inputs = self.tokenizer(
            text, 
            truncation=True, 
            max_length=128, 
            return_tensors="pt"
        ).to(self.device)
        
        # 推理
        with torch.no_grad():
            outputs = self.model(**inputs)
            probs = torch.softmax(outputs.logits, dim=1)
        
        # 处理结果
        pred_prob, pred_id = torch.max(probs, dim=1)
        pred_prob = pred_prob.item()
        pred_id = pred_id.item()
        
        # 获取Top3结果
        top3 = self._get_top3(probs)
        
        # 低置信度处理
        if pred_prob < confidence_threshold:
            return {
                "label": "不确定",
                "confidence": pred_prob,
                "top3": top3,
                "warning": f"置信度低于阈值({confidence_threshold:.0%})"
            }
        
        return {
            "label": self.id2label[str(pred_id)],
            "confidence": pred_prob,
            "top3": top3
        }

    def _get_top3(self, probs):
        """获取概率最高的3个结果"""
        top3_probs, top3_ids = torch.topk(probs, 3)
        return [
            {
                "label": self.id2label[str(idx.item())],
                "probability": prob.item()
            }
            for prob, idx in zip(top3_probs[0], top3_ids[0])
        ]

def main():
    print("【12类情绪分类器】")
    print("="*40)
    print("情绪类别: ...")
    print("="*40)
    
    # 初始化分类器
    try:
        classifier = EmotionClassifier()
        print("\n模型加载成功！输入文本进行分析，输入 ':q' 退出")
    except Exception as e:
        print(f"\n模型加载失败: {str(e)}")
        print("请检查：")
        print("1. 模型路径 ./emotion_model_12emo 是否存在")
        print("2. 目录是否包含 label_mapping.json 文件")
        return
    
    while True:
        try:
            text = input("\n请输入要分析的文本: ").strip()
            
            # 退出命令
            if text.lower() in [':q', ':quit', 'exit']:
                print("\n退出程序")
                break
                
            # 空输入处理
            if not text:
                print("输入不能为空！")
                continue
                
            # 预测并打印结果
            result = classifier.predict(text)
            print("\n" + "="*30)
            print(f"📝 文本: {text}")
            
            if "warning" in result:
                print(f"⚠️ {result['warning']}")
            
            print(f"🎯 主情绪: {result['label']} (置信度: {result['confidence']:.2%})")
            
            if result['label'] != "不确定":
                print("\n其他可能情绪:")
                for i, item in enumerate(result["top3"][1:], 1):
                    print(f"{i}. {item['label']}: {item['probability']:.2%}")
            
            print("="*30)
            
        except KeyboardInterrupt:
            print("\n检测到中断，退出程序...")
            break
        except Exception as e:
            print(f"\n❌ 预测时发生错误: {str(e)}")

if __name__ == "__main__":
    main()
```
