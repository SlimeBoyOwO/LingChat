pub mod catalog;
pub mod discovery;
pub mod runtime;
pub use ling_chat_dlc_contract::*;
#[cfg(test)]
mod contract_tests;
pub fn install() {
    ling_chat_dlc_contract::runtime::install(ling_chat_dlc_contract::runtime::Hooks {
        script_directories: ling_chat_plugins::script_package_dirs,
        close_plugin: |app, id| {
            Box::pin(async move { runtime::request_close_plugin(&app, &id).await })
        },
        close_all: |app| Box::pin(async move { runtime::request_close_all(&app).await }),
        force_close_plugin: runtime::close_plugin,
        force_close_all: runtime::close_all,
    });
}
