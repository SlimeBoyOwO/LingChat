use std::sync::Arc;

fn main() {
    // Always generate a fresh token at process start; callers must discover it
    // from the loopback-only ready record rather than configure a predictable secret.
    let token = uuid::Uuid::new_v4().to_string();
    let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
    if let Err(error) =
        runtime.block_on(ling_chat_lib::memory_test_api::api::serve(Arc::from(token)))
    {
        eprintln!("memory-test-api stopped: {error}");
        std::process::exit(1);
    }
}
