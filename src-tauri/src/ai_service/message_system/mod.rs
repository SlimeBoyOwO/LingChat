pub mod events;
pub mod generator;
pub mod processor;
pub mod producer;
pub mod responses;

/// issue #784 的顺序契约测试：驱动真实的 producer 与有序 publisher。
#[cfg(test)]
mod ordering_tests;
