pub mod channels;
pub mod commands;
pub mod events;
pub(crate) mod fs_sync;
pub mod lifecycle;
pub mod persistent_state;
pub(crate) mod reset_transaction;
pub mod responses;
pub mod schema;
pub mod script_popups;
pub mod validate;
pub fn install() {
    use ling_chat_main::ai_service::game_system::script_engine::extension::{self, Hooks};
    extension::install(Hooks {
        validate_choice: channels::validate_choice,
        check_config: lifecycle::check_config,
        initialize: |script, ctx| Box::pin(lifecycle::initialize(script, ctx)),
        restore_title: events::window_title_event::restore_window_title,
        configure_schema: schema::configure,
        event_fields: responses::event_fields,
        effect_names: schema::effect_names,
        validate_config: validate::config,
        validate_event: validate::event,
        register_events: || {
            events::character_file_event::register();
            events::console_window_event::register();
            events::force_choice_event::register();
            events::glitch_window_event::register();
            events::horror_log_event::register();
            events::jumpscare_event::register();
            events::menu_effect_event::register();
            events::poem_game_event::register();
            events::random_var_event::register();
            events::voice_shift_event::register();
            events::wait_event::register();
            events::watch_file_event::register();
            events::window_title_event::register();
        },
        begin: |app| {
            script_popups::begin_run();
            events::glitch_window_event::begin_glitch_window_run(app)
        },
        close: |app| {
            events::glitch_window_event::close_all_glitch_windows(app);
            script_popups::close_all();
        },
        prepare: |script, data, preview| {
            if preview {
                persistent_state::prepare_preview(script)
            } else {
                persistent_state::prepare_playthrough(script, data)
            }
        },
        persist: persistent_state::save_playthrough,
        entry_error: persistent_state::entry_error,
        recover: reset_transaction::recover_pending_resets,
    });
}
