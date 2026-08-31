use sea_orm::Statement;
use sea_orm_migration::prelude::*;

/// 为 line 增加 spoken_content / spoken_language，保存按有效 voice_lang 选定的
/// TTS 输入文本与语言。
///
/// 旧版 tts_content 是历史命名的“第二语言”字段，可能包含日语，也可能在运行时
/// 被翻译成英/韩/西/阿语，无法仅凭 UI locale 判断实际朗读内容。新增可空列后，
/// 新行可以精确恢复显示与补生成语音；旧行保持 NULL，统一显示 canonical content。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT name FROM pragma_table_info('line') WHERE name = 'spoken_content'"
                    .to_string(),
            ))
            .await?;
        if rows.is_empty() {
            manager
                .alter_table(
                    Table::alter()
                        .table(Line::Table)
                        .add_column(ColumnDef::new(Line::SpokenContent).text().null())
                        .to_owned(),
                )
                .await?;
        }

        let language_rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT name FROM pragma_table_info('line') WHERE name = 'spoken_language'"
                    .to_string(),
            ))
            .await?;
        if language_rows.is_empty() {
            manager
                .alter_table(
                    Table::alter()
                        .table(Line::Table)
                        .add_column(ColumnDef::new(Line::SpokenLanguage).string_len(32).null())
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Line::Table)
                    .drop_column(Line::SpokenLanguage)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Line::Table)
                    .drop_column(Line::SpokenContent)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Line {
    Table,
    SpokenContent,
    SpokenLanguage,
}
