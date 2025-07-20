# webuiTest - main 项目概览

## 项目结构
```text
webuiTest - main/
├── backend/
│   ├── api/
│   │   ├── __init__.py
│   │   └── chat_history.py
│   ├── core/
│   │   ├── VitsTTS.py
│   │   ├── __init__.py
│   │   ├── ai_service.py
│   │   ├── deepseek.py
│   │   ├── frontend_manager.py
│   │   ├── langDetect.py
│   │   ├── logger.py
│   │   ├── predictor.py
│   │   ├── service_manager.py
│   │   └── websocket_server.py
│   ├── database/
│   │   ├── __init__.py
│   │   ├── chat_system.db
│   │   ├── conversation_model.py
│   │   ├── database.py
│   │   ├── test.py
│   │   ├── test_user.py
│   │   └── user_model.py
│   ├── emotion_model_12emo/
│   │   ├── config.json
│   │   ├── label_mapping.json
│   │   ├── model.safetensors
│   │   ├── special_tokens_map.json
│   │   ├── tokenizer_config.json
│   │   └── vocab.txt
│   ├── emotion_model_18emo/
│   │   ├── config.json
│   │   ├── label_mapping.json
│   │   ├── model.safetensors
│   │   ├── special_tokens_map.json
│   │   ├── tokenizer_config.json
│   │   ├── training_args.bin
│   │   └── vocab.txt
│   ├── go-impl/
│   │   ├── api/
│   │   │   ├── routes/
│   │   │   │   ├── common/
│   │   │   │   │   └── context.go
│   │   │   │   ├── middleware/
│   │   │   │   │   └── jwt.go
│   │   │   │   ├── v1/
│   │   │   │   │   ├── request/
│   │   │   │   │   │   ├── chat.go
│   │   │   │   │   │   └── user.go
│   │   │   │   │   ├── response/
│   │   │   │   │   │   └── chat.go
│   │   │   │   │   ├── chat.go
│   │   │   │   │   └── user.go
│   │   │   │   └── routes.go
│   │   │   ├── websocket.go
│   │   │   └── websocket_test.go
│   │   ├── cmd/
│   │   │   └── app/
│   │   │       ├── main.go
│   │   │       └── main_test.go
│   │   ├── docs/
│   │   │   └── GETTING_STARTED.md
│   │   ├── internal/
│   │   │   ├── clients/
│   │   │   │   ├── VitsTTS/
│   │   │   │   │   ├── VitsTTS.go
│   │   │   │   │   └── VitsTTS_test.go
│   │   │   │   ├── emotionPredictor/
│   │   │   │   │   ├── data.go
│   │   │   │   │   ├── emotionPredictor.go
│   │   │   │   │   └── emotionPredictor_test.go
│   │   │   │   └── llm/
│   │   │   │       ├── llm.go
│   │   │   │       └── llm_test.go
│   │   │   ├── config/
│   │   │   │   └── config.go
│   │   │   ├── data/
│   │   │   │   ├── ent/
│   │   │   │   │   ├── ent/
│   │   │   │   │   │   ├── conversation/
│   │   │   │   │   │   │   ├── conversation.go
│   │   │   │   │   │   │   └── where.go
│   │   │   │   │   │   ├── conversationmessage/
│   │   │   │   │   │   │   ├── conversationmessage.go
│   │   │   │   │   │   │   └── where.go
│   │   │   │   │   │   ├── enttest/
│   │   │   │   │   │   │   └── enttest.go
│   │   │   │   │   │   ├── hook/
│   │   │   │   │   │   │   └── hook.go
│   │   │   │   │   │   ├── migrate/
│   │   │   │   │   │   │   ├── migrate.go
│   │   │   │   │   │   │   └── schema.go
│   │   │   │   │   │   ├── predicate/
│   │   │   │   │   │   │   └── predicate.go
│   │   │   │   │   │   ├── runtime/
│   │   │   │   │   │   │   └── runtime.go
│   │   │   │   │   │   ├── shadow/
│   │   │   │   │   │   │   ├── shadow.go
│   │   │   │   │   │   │   └── where.go
│   │   │   │   │   │   ├── user/
│   │   │   │   │   │   │   ├── user.go
│   │   │   │   │   │   │   └── where.go
│   │   │   │   │   │   ├── client.go
│   │   │   │   │   │   ├── conversation.go
│   │   │   │   │   │   ├── conversation_create.go
│   │   │   │   │   │   ├── conversation_delete.go
│   │   │   │   │   │   ├── conversation_query.go
│   │   │   │   │   │   ├── conversation_update.go
│   │   │   │   │   │   ├── conversationmessage.go
│   │   │   │   │   │   ├── conversationmessage_create.go
│   │   │   │   │   │   ├── conversationmessage_delete.go
│   │   │   │   │   │   ├── conversationmessage_query.go
│   │   │   │   │   │   ├── conversationmessage_update.go
│   │   │   │   │   │   ├── ent.go
│   │   │   │   │   │   ├── mutation.go
│   │   │   │   │   │   ├── runtime.go
│   │   │   │   │   │   ├── shadow.go
│   │   │   │   │   │   ├── shadow_create.go
│   │   │   │   │   │   ├── shadow_delete.go
│   │   │   │   │   │   ├── shadow_query.go
│   │   │   │   │   │   ├── shadow_update.go
│   │   │   │   │   │   ├── tx.go
│   │   │   │   │   │   ├── user.go
│   │   │   │   │   │   ├── user_create.go
│   │   │   │   │   │   ├── user_delete.go
│   │   │   │   │   │   ├── user_query.go
│   │   │   │   │   │   └── user_update.go
│   │   │   │   │   ├── schema/
│   │   │   │   │   │   ├── conversation.go
│   │   │   │   │   │   ├── conversation_message.go
│   │   │   │   │   │   ├── shadow.go
│   │   │   │   │   │   ├── timestamp.go
│   │   │   │   │   │   └── user.go
│   │   │   │   │   └── generate.go
│   │   │   │   ├── conversation.go
│   │   │   │   ├── data.go
│   │   │   │   ├── legacy_temp_chat_context.go
│   │   │   │   └── user.go
│   │   │   └── service/
│   │   │       ├── conversation.go
│   │   │       ├── langDetect.go
│   │   │       ├── langDetect_test.go
│   │   │       ├── lingChat.go
│   │   │       ├── lingChat_test.go
│   │   │       ├── llm_msg_parser.go
│   │   │       └── user.go
│   │   ├── pkg/
│   │   │   └── jwt/
│   │   │       └── jwt.go
│   │   ├── .env
│   │   ├── .env.example
│   │   ├── Dockerfile
│   │   ├── README.md
│   │   ├── go.mod
│   │   └── go.sum
│   ├── README.md
│   ├── predictor_server.py
│   ├── requirements.txt
│   ├── webChat.docker.py
│   └── windows_main.py
├── frontend/
│   └── public/
│       ├── audio/
│       │   ├── part_1.wav
│       │   ├── part_2.wav
│       │   ├── part_3.wav
│       │   ├── part_4.wav
│       │   ├── part_5.wav
│       │   ├── part_6.wav
│       │   ├── part_7.wav
│       │   └── part_8.wav
│       ├── audio_effects/
│       │   ├── 伤心.wav
│       │   ├── 厌恶.wav
│       │   ├── 叹气.wav
│       │   ├── 喜悦.wav
│       │   ├── 喜爱.wav
│       │   ├── 困扰.wav
│       │   ├── 害羞.wav
│       │   ├── 察觉.wav
│       │   ├── 尴尬.wav
│       │   ├── 思考.wav
│       │   ├── 惊讶.wav
│       │   ├── 愉快.wav
│       │   ├── 无语.wav
│       │   ├── 流汗.wav
│       │   ├── 生气.wav
│       │   ├── 疑问.wav
│       │   ├── 聊天.wav
│       │   ├── 转场.wav
│       │   └── 震惊.wav
│       ├── css/
│       │   ├── animation.css
│       │   ├── browser-compat.css
│       │   ├── disable-animations.css
│       │   ├── galgame.css
│       │   ├── login.css
│       │   ├── menu.css
│       │   └── styles.css
│       ├── js/
│       │   ├── core/
│       │   │   ├── connection.js
│       │   │   ├── event-bus.js
│       │   │   └── safe-storage.js
│       │   ├── features/
│       │   │   ├── background/
│       │   │   │   └── star-field.js
│       │   │   ├── chat/
│       │   │   │   └── manager.js
│       │   │   ├── emotion/
│       │   │   │   ├── config.js
│       │   │   │   └── controller.js
│       │   │   ├── history/
│       │   │   │   └── manager.js
│       │   │   ├── image/
│       │   │   │   └── controller.js
│       │   │   ├── menu/
│       │   │   │   └── controller.js
│       │   │   ├── save/
│       │   │   │   ├── controller.js
│       │   │   │   └── conversation-loader.js
│       │   │   └── sound/
│       │   │       └── controller.js
│       │   ├── login/
│       │   │   ├── auth-check.js
│       │   │   ├── clear-user-data.js
│       │   │   └── login.js
│       │   ├── ui/
│       │   │   ├── dom.js
│       │   │   ├── type-writer.js
│       │   │   └── ui-controller.js
│       │   ├── utils/
│       │   │   ├── dom-utils.js
│       │   │   └── image-utils.js
│       │   └── app.js
│       ├── pages/
│       │   ├── index.html
│       │   └── login.html
│       └── pictures/
│           ├── ai logo/
│           │   ├── claude.webp
│           │   ├── deepseek.webp
│           │   ├── gemini.webp
│           │   ├── grok.webp
│           │   ├── openai.webp
│           │   └── qwen.webp
│           ├── animation/
│           │   ├── AI思考.webp
│           │   ├── 叹气.webp
│           │   ├── 害羞.webp
│           │   ├── 察觉.webp
│           │   ├── 心动.webp
│           │   ├── 惊讶.webp
│           │   ├── 慌乱.webp
│           │   ├── 流汗.webp
│           │   ├── 流泪.webp
│           │   ├── 生气.webp
│           │   ├── 生气2.webp
│           │   ├── 疑问.webp
│           │   ├── 紧张.webp
│           │   ├── 聊天.webp
│           │   ├── 转场.webp
│           │   ├── 转场卡顿.webp
│           │   ├── 难为情.webp
│           │   └── 高兴.webp
│           ├── backgrounds/
│           │   ├── login/
│           │   │   ├── ... (1 more .png files)
│           │   │   ├── 01.png
│           │   │   ├── 8e7a06969b6ecad17eef1914434859693493265644980448.png
│           │   │   └── X0_6rTZl.png
│           │   ├── 266851.jpg
│           │   ├── 386057.jpg
│           │   ├── homepage_bg.jpeg
│           │   ├── homepage_bg2.jpg
│           │   ├── 纯白背景.png
│           │   └── 菜单背景.png
│           ├── icons/
│           │   ├── favicon.svg
│           │   ├── 小猫.svg
│           │   └── 小猫Ling.svg
│           ├── qinling/
│           │   ├── ... (15 more .png files)
│           │   ├── 不屑.png
│           │   ├── 伤心.png
│           │   ├── 兴奋.png
│           │   ├── 新版调皮.sai2
│           │   ├── 正常 - 副本.sai2
│           │   └── 调皮.sai2
│           └── Python安装必看.png
├── log/
│   ├── ... (112 more .log files)
│   ├── 0.log
│   ├── 1.log
│   └── 10.log
├── logs/
│   ├── ... (211 more .log files)
│   ├── 0.log
│   ├── 1.log
│   ├── 10.log
│   └── log转json.py
├── node_modules/
│   ├── body-parser/
│   │   ├── lib/
│   │   │   ├── types/
│   │   │   │   ├── json.js
│   │   │   │   ├── raw.js
│   │   │   │   ├── text.js
│   │   │   │   └── urlencoded.js
│   │   │   ├── read.js
│   │   │   └── utils.js
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── bytes/
│   │   ├── History.md
│   │   ├── LICENSE
│   │   ├── Readme.md
│   │   ├── index.js
│   │   └── package.json
│   ├── call-bind-apply-helpers/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── actualApply.d.ts
│   │   ├── actualApply.js
│   │   ├── applyBind.d.ts
│   │   ├── applyBind.js
│   │   ├── functionApply.d.ts
│   │   ├── functionApply.js
│   │   ├── functionCall.d.ts
│   │   ├── functionCall.js
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   ├── reflectApply.d.ts
│   │   ├── reflectApply.js
│   │   └── tsconfig.json
│   ├── call-bound/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── content-type/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── debug/
│   │   ├── src/
│   │   │   ├── browser.js
│   │   │   ├── common.js
│   │   │   ├── index.js
│   │   │   └── node.js
│   │   ├── LICENSE
│   │   ├── README.md
│   │   └── package.json
│   ├── depd/
│   │   ├── lib/
│   │   │   └── browser/
│   │   │       └── index.js
│   │   ├── History.md
│   │   ├── LICENSE
│   │   ├── Readme.md
│   │   ├── index.js
│   │   └── package.json
│   ├── dotenv/
│   │   ├── lib/
│   │   │   ├── cli-options.js
│   │   │   ├── env-options.js
│   │   │   ├── main.d.ts
│   │   │   └── main.js
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README-es.md
│   │   ├── README.md
│   │   ├── config.d.ts
│   │   ├── config.js
│   │   └── package.json
│   ├── dunder-proto/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   ├── get.js
│   │   │   ├── index.js
│   │   │   └── set.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── get.d.ts
│   │   ├── get.js
│   │   ├── package.json
│   │   ├── set.d.ts
│   │   ├── set.js
│   │   └── tsconfig.json
│   ├── ee-first/
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── es-define-property/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── es-errors/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── eval.d.ts
│   │   ├── eval.js
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   ├── range.d.ts
│   │   ├── range.js
│   │   ├── ref.d.ts
│   │   ├── ref.js
│   │   ├── syntax.d.ts
│   │   ├── syntax.js
│   │   ├── tsconfig.json
│   │   ├── type.d.ts
│   │   ├── type.js
│   │   ├── uri.d.ts
│   │   └── uri.js
│   ├── es-object-atoms/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── RequireObjectCoercible.d.ts
│   │   ├── RequireObjectCoercible.js
│   │   ├── ToObject.d.ts
│   │   ├── ToObject.js
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── isObject.d.ts
│   │   ├── isObject.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── es6-promise/
│   │   ├── dist/
│   │   │   ├── es6-promise.auto.js
│   │   │   ├── es6-promise.auto.map
│   │   │   ├── es6-promise.auto.min.js
│   │   │   ├── es6-promise.auto.min.map
│   │   │   ├── es6-promise.js
│   │   │   ├── es6-promise.map
│   │   │   ├── es6-promise.min.js
│   │   │   └── es6-promise.min.map
│   │   ├── lib/
│   │   │   ├── es6-promise/
│   │   │   │   ├── promise/
│   │   │   │   │   ├── all.js
│   │   │   │   │   ├── race.js
│   │   │   │   │   ├── reject.js
│   │   │   │   │   └── resolve.js
│   │   │   │   ├── -internal.js
│   │   │   │   ├── asap.js
│   │   │   │   ├── enumerator.js
│   │   │   │   ├── polyfill.js
│   │   │   │   ├── promise.js
│   │   │   │   ├── then.js
│   │   │   │   └── utils.js
│   │   │   ├── es6-promise.auto.js
│   │   │   └── es6-promise.js
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── auto.js
│   │   ├── es6-promise.d.ts
│   │   └── package.json
│   ├── express-http-proxy/
│   │   ├── .github/
│   │   │   └── workflows/
│   │   │       └── node.js.yml
│   │   ├── app/
│   │   │   └── steps/
│   │   │       ├── buildProxyReq.js
│   │   │       ├── copyProxyResHeadersToUserRes.js
│   │   │       ├── decorateProxyReqBody.js
│   │   │       ├── decorateProxyReqOpts.js
│   │   │       ├── decorateUserRes.js
│   │   │       ├── decorateUserResHeaders.js
│   │   │       ├── filterUserRequest.js
│   │   │       ├── handleProxyErrors.js
│   │   │       ├── maybeSkipToNextHandler.js
│   │   │       ├── prepareProxyReq.js
│   │   │       ├── resolveProxyHost.js
│   │   │       ├── resolveProxyReqPath.js
│   │   │       ├── sendProxyRequest.js
│   │   │       └── sendUserRes.js
│   │   ├── lib/
│   │   │   ├── as.js
│   │   │   ├── chunkLength.js
│   │   │   ├── getHeaders.js
│   │   │   ├── isUnset.js
│   │   │   ├── mockHTTP.js
│   │   │   ├── requestOptions.js
│   │   │   ├── resolveOptions.js
│   │   │   └── scopeContainer.js
│   │   ├── node_modules/
│   │   │   ├── debug/
│   │   │   │   ├── src/
│   │   │   │   │   ├── browser.js
│   │   │   │   │   ├── common.js
│   │   │   │   │   ├── index.js
│   │   │   │   │   └── node.js
│   │   │   │   ├── CHANGELOG.md
│   │   │   │   ├── LICENSE
│   │   │   │   ├── README.md
│   │   │   │   ├── node.js
│   │   │   │   └── package.json
│   │   │   ├── iconv-lite/
│   │   │   │   ├── encodings/
│   │   │   │   │   ├── tables/
│   │   │   │   │   │   ├── big5-added.json
│   │   │   │   │   │   ├── cp936.json
│   │   │   │   │   │   ├── cp949.json
│   │   │   │   │   │   ├── cp950.json
│   │   │   │   │   │   ├── eucjp.json
│   │   │   │   │   │   ├── gb18030-ranges.json
│   │   │   │   │   │   ├── gbk-added.json
│   │   │   │   │   │   └── shiftjis.json
│   │   │   │   │   ├── dbcs-codec.js
│   │   │   │   │   ├── dbcs-data.js
│   │   │   │   │   ├── index.js
│   │   │   │   │   ├── internal.js
│   │   │   │   │   ├── sbcs-codec.js
│   │   │   │   │   ├── sbcs-data-generated.js
│   │   │   │   │   ├── sbcs-data.js
│   │   │   │   │   ├── utf16.js
│   │   │   │   │   └── utf7.js
│   │   │   │   ├── lib/
│   │   │   │   │   ├── bom-handling.js
│   │   │   │   │   ├── extend-node.js
│   │   │   │   │   ├── index.d.ts
│   │   │   │   │   ├── index.js
│   │   │   │   │   └── streams.js
│   │   │   │   ├── Changelog.md
│   │   │   │   ├── LICENSE
│   │   │   │   ├── README.md
│   │   │   │   └── package.json
│   │   │   └── raw-body/
│   │   │       ├── HISTORY.md
│   │   │       ├── LICENSE
│   │   │       ├── README.md
│   │   │       ├── SECURITY.md
│   │   │       ├── index.d.ts
│   │   │       ├── index.js
│   │   │       └── package.json
│   │   ├── test/
│   │   │   ├── support/
│   │   │   │   └── proxyTarget.js
│   │   │   ├── bodyEncoding.js
│   │   │   ├── catchingErrors.js
│   │   │   ├── cookies.js
│   │   │   ├── decorateUserResHeaders.js
│   │   │   ├── defineMultipleProxyHandlers.js
│   │   │   ├── filter.js
│   │   │   ├── getBody.js
│   │   │   ├── handleProxyError.js
│   │   │   ├── headers.js
│   │   │   ├── host.js
│   │   │   ├── https.js
│   │   │   ├── maybeSkipToNextHandler.js
│   │   │   ├── middlewareCompatibility.js
│   │   │   ├── params.js
│   │   │   ├── path.js
│   │   │   ├── port.js
│   │   │   ├── postBody.js
│   │   │   ├── preserveHostHdr.js
│   │   │   ├── proxyReqOptDecorator.js
│   │   │   ├── proxyReqPathResolver.js
│   │   │   ├── resolveProxyReqPath.js
│   │   │   ├── session.js
│   │   │   ├── status.js
│   │   │   ├── streaming.js
│   │   │   ├── timeout.js
│   │   │   ├── traceDebugging.js
│   │   │   ├── urlParsing.js
│   │   │   ├── userResDecorator.js
│   │   │   └── verbs.js
│   │   ├── .eslintignore
│   │   ├── .eslintrc
│   │   ├── .travis.yml
│   │   ├── LICENSE-MIT
│   │   ├── README.md
│   │   ├── index.js
│   │   ├── package.json
│   │   ├── request1
│   │   └── request2
│   ├── function-bind/
│   │   ├── .github/
│   │   │   ├── FUNDING.yml
│   │   │   └── SECURITY.md
│   │   ├── test/
│   │   │   ├── .eslintrc
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── implementation.js
│   │   ├── index.js
│   │   └── package.json
│   ├── get-intrinsic/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── GetIntrinsic.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── get-proto/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── Object.getPrototypeOf.d.ts
│   │   ├── Object.getPrototypeOf.js
│   │   ├── README.md
│   │   ├── Reflect.getPrototypeOf.d.ts
│   │   ├── Reflect.getPrototypeOf.js
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── gopd/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── gOPD.d.ts
│   │   ├── gOPD.js
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── has-symbols/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   ├── shams/
│   │   │   │   ├── core-js.js
│   │   │   │   └── get-own-property-symbols.js
│   │   │   ├── index.js
│   │   │   └── tests.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   ├── shams.d.ts
│   │   ├── shams.js
│   │   └── tsconfig.json
│   ├── hasown/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── http-errors/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── iconv-lite/
│   │   ├── .github/
│   │   │   └── dependabot.yml
│   │   ├── encodings/
│   │   │   ├── tables/
│   │   │   │   ├── big5-added.json
│   │   │   │   ├── cp936.json
│   │   │   │   ├── cp949.json
│   │   │   │   ├── cp950.json
│   │   │   │   ├── eucjp.json
│   │   │   │   ├── gb18030-ranges.json
│   │   │   │   ├── gbk-added.json
│   │   │   │   └── shiftjis.json
│   │   │   ├── dbcs-codec.js
│   │   │   ├── dbcs-data.js
│   │   │   ├── index.js
│   │   │   ├── internal.js
│   │   │   ├── sbcs-codec.js
│   │   │   ├── sbcs-data-generated.js
│   │   │   ├── sbcs-data.js
│   │   │   ├── utf16.js
│   │   │   ├── utf32.js
│   │   │   └── utf7.js
│   │   ├── lib/
│   │   │   ├── bom-handling.js
│   │   │   ├── index.d.ts
│   │   │   ├── index.js
│   │   │   └── streams.js
│   │   ├── Changelog.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   └── package.json
│   ├── inherits/
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── inherits.js
│   │   ├── inherits_browser.js
│   │   └── package.json
│   ├── math-intrinsics/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── constants/
│   │   │   ├── maxArrayLength.d.ts
│   │   │   ├── maxArrayLength.js
│   │   │   ├── maxSafeInteger.d.ts
│   │   │   ├── maxSafeInteger.js
│   │   │   ├── maxValue.d.ts
│   │   │   └── maxValue.js
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .eslintrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── abs.d.ts
│   │   ├── abs.js
│   │   ├── floor.d.ts
│   │   ├── floor.js
│   │   ├── isFinite.d.ts
│   │   ├── isFinite.js
│   │   ├── isInteger.d.ts
│   │   ├── isInteger.js
│   │   ├── isNaN.d.ts
│   │   ├── isNaN.js
│   │   ├── isNegativeZero.d.ts
│   │   ├── isNegativeZero.js
│   │   ├── max.d.ts
│   │   ├── max.js
│   │   ├── min.d.ts
│   │   ├── min.js
│   │   ├── mod.d.ts
│   │   ├── mod.js
│   │   ├── package.json
│   │   ├── pow.d.ts
│   │   ├── pow.js
│   │   ├── round.d.ts
│   │   ├── round.js
│   │   ├── sign.d.ts
│   │   ├── sign.js
│   │   └── tsconfig.json
│   ├── media-typer/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── mime-db/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── db.json
│   │   ├── index.js
│   │   └── package.json
│   ├── mime-types/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   ├── mimeScore.js
│   │   └── package.json
│   ├── ms/
│   │   ├── index.js
│   │   ├── license.md
│   │   ├── package.json
│   │   └── readme.md
│   ├── object-inspect/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── example/
│   │   │   ├── all.js
│   │   │   ├── circular.js
│   │   │   ├── fn.js
│   │   │   └── inspect.js
│   │   ├── test/
│   │   │   ├── browser/
│   │   │   │   └── dom.js
│   │   │   ├── bigint.js
│   │   │   ├── circular.js
│   │   │   ├── deep.js
│   │   │   ├── element.js
│   │   │   ├── err.js
│   │   │   ├── fakes.js
│   │   │   ├── fn.js
│   │   │   ├── global.js
│   │   │   ├── has.js
│   │   │   ├── holes.js
│   │   │   ├── indent-option.js
│   │   │   ├── inspect.js
│   │   │   ├── lowbyte.js
│   │   │   ├── number.js
│   │   │   ├── quoteStyle.js
│   │   │   ├── toStringTag.js
│   │   │   ├── undef.js
│   │   │   └── values.js
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── index.js
│   │   ├── package-support.json
│   │   ├── package.json
│   │   ├── readme.markdown
│   │   ├── test-core-js.js
│   │   └── util.inspect.js
│   ├── on-finished/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── qs/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── dist/
│   │   │   └── qs.js
│   │   ├── lib/
│   │   │   ├── formats.js
│   │   │   ├── index.js
│   │   │   ├── parse.js
│   │   │   ├── stringify.js
│   │   │   └── utils.js
│   │   ├── test/
│   │   │   ├── empty-keys-cases.js
│   │   │   ├── parse.js
│   │   │   ├── stringify.js
│   │   │   └── utils.js
│   │   ├── .editorconfig
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE.md
│   │   ├── README.md
│   │   └── package.json
│   ├── raw-body/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── SECURITY.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   └── package.json
│   ├── safer-buffer/
│   │   ├── LICENSE
│   │   ├── Porting-Buffer.md
│   │   ├── Readme.md
│   │   ├── dangerous.js
│   │   ├── package.json
│   │   ├── safer.js
│   │   └── tests.js
│   ├── setprototypeof/
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   └── package.json
│   ├── side-channel/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .editorconfig
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── side-channel-list/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .editorconfig
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── list.d.ts
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── side-channel-map/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .editorconfig
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── side-channel-weakmap/
│   │   ├── .github/
│   │   │   └── FUNDING.yml
│   │   ├── test/
│   │   │   └── index.js
│   │   ├── .editorconfig
│   │   ├── .eslintrc
│   │   ├── .nycrc
│   │   ├── CHANGELOG.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.d.ts
│   │   ├── index.js
│   │   ├── package.json
│   │   └── tsconfig.json
│   ├── statuses/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── codes.json
│   │   ├── index.js
│   │   └── package.json
│   ├── toidentifier/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── type-is/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── unpipe/
│   │   ├── HISTORY.md
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── index.js
│   │   └── package.json
│   ├── ws/
│   │   ├── lib/
│   │   │   ├── buffer-util.js
│   │   │   ├── constants.js
│   │   │   ├── event-target.js
│   │   │   ├── extension.js
│   │   │   ├── limiter.js
│   │   │   ├── permessage-deflate.js
│   │   │   ├── receiver.js
│   │   │   ├── sender.js
│   │   │   ├── stream.js
│   │   │   ├── subprotocol.js
│   │   │   ├── validation.js
│   │   │   ├── websocket-server.js
│   │   │   └── websocket.js
│   │   ├── LICENSE
│   │   ├── README.md
│   │   ├── browser.js
│   │   ├── index.js
│   │   ├── package.json
│   │   └── wrapper.mjs
│   └── .package-lock.json
├── scripts/
│   └── start.py
├── temp_voice/
├── third_party/
│   ├── install.bat
│   └── logger_new.py
├── 记忆测试/
│   └── RAG/
│       ├── 2025年04月/
│       │   └── 05日/
│       │       └── session_20250405_195515.json
│       ├── Dialogue_history/
│       │   ├── 2025年04月/
│       │   │   └── 05日/
│       │   │       └── session_20250405_195515.json
│       │   └── 2025年05月/
│       │       └── 16日/
│       │           ├── session_20250516_004910.json
│       │           └── session_20250516_121232.json
│       ├── chroma_db_store/
│       │   ├── 72e4d717-aa9d-49c8-9fff-46c190f36486/
│       │   │   ├── ... (1 more .bin files)
│       │   │   ├── data_level0.bin
│       │   │   ├── header.bin
│       │   │   └── length.bin
│       │   └── chroma.sqlite3
│       ├── logs/
│       │   └── 32.log
│       ├── run_logs/
│       │   ├── ... (4 more .log files)
│       │   ├── 2025-05-16_00-42-00.log
│       │   ├── 2025-05-16_00-44-01.log
│       │   └── 2025-05-16_00-49-04.log
│       └── 32.log
├── .env
├── .env.example
├── .env.example.docker
├── .gitignore
├── Dockerfile-node
├── Dockerfile-predictor
├── Dockerfile-python
├── LICENSE
├── README.md
├── chat_system.db
├── docker-compose.golang.yml
├── docker-compose.yml
├── lingchat.ico
├── lingchat.png
├── package-lock.json
├── package.json
├── progress.md
├── requirements.txt
├── start.docker.bat
└── start.windows.bat
```

## 文件内容
### 文件: `README.md`

```markdown
# [0.1.0-正式版] - 待发布(develop)
已更新：

- 将12类情感分类模型扩展，新增18类情感分类模型

计划：

1，多用户支持
- 为服务器端部署增加用户注册与登录功能

2，聊天记录切换
- 支持用户在浏览器端创建、切换不同的聊天会话（上下文）。

3，人设切换与分享
- 支持用户在浏览器端创建、切换不同的人设+背景剧情Prompt。
- 支持用户导出/导入人设+背景剧情Prompt的json

4，双部署模式
- 项目按照服务器部署模式开发
- 为个人本地部署模式提供一建包：实质是第一次启动自动注册，后续启动时以本地用户账号登录。同时在README中提供本地账户默认的用户名和密码

当前项目结构
```
LingChat/
├── Dockerfile-node # Node.js环境Docker配置
├── Dockerfile-predictor # 情感预测服务Docker配置
├── Dockerfile-python # Python环境Docker配置
├── backend/ # 后端服务目录，包含核心业务逻辑和AI交互实现。
│   ├── VitsTTS.py # 基于VITS的文本到语音转换模块
│   ├── deepseek.py # DeepSeek AI模型接口
│   ├── emotion_model_12emo/ # 12分类情感模型目录
│   ├── emotion_model_18emo/ # 18分类情感模型目录
│   ├── go-impl/ # Go语言实现的组件
│   ├── langDetect.py # 语言检测模块，用于区分中日文本
│   ├── logger.py # 日志记录模块
│   ├── predictor.py # 情感分类模型，使用BERT分析文本情感
│   ├── predictor_server.py # 情感预测服务的独立服务器
│   ├── run.bat # Windows环境下的启动脚本
│   ├── webChat.docker.py # Docker环境下的主程序入口
│   ├── webChat.windows.py # Windows环境下的主程序入口，处理WebSocket连接、AI回复处理和情感分析
│   └── webChat2.py # 主程序的替代版本
├── docker-compose.yml # Docker容器配置
├── frontend/ # 前端应用目录，基于Node.js和WebSocket实现实时通信。
│   ├── package.json # 项目依赖配置
│   ├── public/ # 前端静态资源目录
│   │   ├── audio/ # 语音文件存储目录
│   │   ├── audio_effects/ # 音效文件目录
│   │   ├── css/ # 样式表文件
│   │   │   ├── animation.css # 动画效果
│   │   │   ├── browser-compat.css # 浏览器兼容样式
│   │   │   ├── galgame.css # 主要样式
│   │   │   └── menu.css # 菜单样式
│   │   ├── js/ # JavaScript文件
│   │   ├── pages/ # HTML页面文件
│   │   │   └── index.html # 主页面，包含对话界面和设置菜单
│   │   └── pictures/ # 图片资源目录，包含背景和角色图片
│   ├── run_server.bat # 前端服务器启动脚本
│   └── server.js # 前端服务器，处理静态资源和WebSocket消息转发
├── logs/ # 日志文件目录
├── requirements.txt # Python依赖列表
├── scripts/ # 脚本工具目录
│   └── start.py # 项目启动脚本，负责启动虚拟环境、检查依赖、启动各组件服务
├── start.docker.bat # Docker环境启动脚本
├── start.windows.bat # Windows环境启动脚本
├── temp_voice/ # 临时语音文件存储目录
└── third_party/ # 第三方依赖目录
    └── install.bat # 第三方依赖安装脚本
```

0.2.0计划：

使用RAG向量库等实现长期记忆
增加类似安科/跑团式的长线预设剧情支持
```

### 文件: `readme.md`

```markdown
# [0.1.0-正式版] - 待发布(develop)
已更新：

- 将12类情感分类模型扩展，新增18类情感分类模型

计划：

1，多用户支持
- 为服务器端部署增加用户注册与登录功能

2，聊天记录切换
- 支持用户在浏览器端创建、切换不同的聊天会话（上下文）。

3，人设切换与分享
- 支持用户在浏览器端创建、切换不同的人设+背景剧情Prompt。
- 支持用户导出/导入人设+背景剧情Prompt的json

4，双部署模式
- 项目按照服务器部署模式开发
- 为个人本地部署模式提供一建包：实质是第一次启动自动注册，后续启动时以本地用户账号登录。同时在README中提供本地账户默认的用户名和密码

当前项目结构
```
LingChat/
├── Dockerfile-node # Node.js环境Docker配置
├── Dockerfile-predictor # 情感预测服务Docker配置
├── Dockerfile-python # Python环境Docker配置
├── backend/ # 后端服务目录，包含核心业务逻辑和AI交互实现。
│   ├── VitsTTS.py # 基于VITS的文本到语音转换模块
│   ├── deepseek.py # DeepSeek AI模型接口
│   ├── emotion_model_12emo/ # 12分类情感模型目录
│   ├── emotion_model_18emo/ # 18分类情感模型目录
│   ├── go-impl/ # Go语言实现的组件
│   ├── langDetect.py # 语言检测模块，用于区分中日文本
│   ├── logger.py # 日志记录模块
│   ├── predictor.py # 情感分类模型，使用BERT分析文本情感
│   ├── predictor_server.py # 情感预测服务的独立服务器
│   ├── run.bat # Windows环境下的启动脚本
│   ├── webChat.docker.py # Docker环境下的主程序入口
│   ├── webChat.windows.py # Windows环境下的主程序入口，处理WebSocket连接、AI回复处理和情感分析
│   └── webChat2.py # 主程序的替代版本
├── docker-compose.yml # Docker容器配置
├── frontend/ # 前端应用目录，基于Node.js和WebSocket实现实时通信。
│   ├── package.json # 项目依赖配置
│   ├── public/ # 前端静态资源目录
│   │   ├── audio/ # 语音文件存储目录
│   │   ├── audio_effects/ # 音效文件目录
│   │   ├── css/ # 样式表文件
│   │   │   ├── animation.css # 动画效果
│   │   │   ├── browser-compat.css # 浏览器兼容样式
│   │   │   ├── galgame.css # 主要样式
│   │   │   └── menu.css # 菜单样式
│   │   ├── js/ # JavaScript文件
│   │   ├── pages/ # HTML页面文件
│   │   │   └── index.html # 主页面，包含对话界面和设置菜单
│   │   └── pictures/ # 图片资源目录，包含背景和角色图片
│   ├── run_server.bat # 前端服务器启动脚本
│   └── server.js # 前端服务器，处理静态资源和WebSocket消息转发
├── logs/ # 日志文件目录
├── requirements.txt # Python依赖列表
├── scripts/ # 脚本工具目录
│   └── start.py # 项目启动脚本，负责启动虚拟环境、检查依赖、启动各组件服务
├── start.docker.bat # Docker环境启动脚本
├── start.windows.bat # Windows环境启动脚本
├── temp_voice/ # 临时语音文件存储目录
└── third_party/ # 第三方依赖目录
    └── install.bat # 第三方依赖安装脚本
```

0.2.0计划：

使用RAG向量库等实现长期记忆
增加类似安科/跑团式的长线预设剧情支持
```

### 文件: `backend/README.md`

```markdown

```

### 文件: `backend/api/__init__.py`

```python

```

### 文件: `backend/api/chat_history.py`

```python
# api/chat_history.py

from fastapi import APIRouter, Query, HTTPException, Request
from typing import List
from datetime import datetime
from database.user_model import UserConversationModel
from database.conversation_model import ConversationModel
from core.service_manager import service_manager

router = APIRouter(prefix="/api/v1/chat/history", tags=["Chat History"])

@router.get("/list")
async def list_user_conversations(user_id: int, page: int = 1, page_size: int = 10):
    try:
        result = UserConversationModel.get_user_conversations(user_id, page, page_size)
        return {
            "code": 200,
            "data": result
        }
    except Exception as e:
        return {
            "code": 500,
            "msg": "Failed to fetch user conversations",
            "error": str(e)
        }
    
@router.get("/load")
async def load_user_conversations(user_id: int, conversation_id: int):
    try:
        result = ConversationModel.get_conversation_messages(conversation_id=conversation_id)
        if(result != None):
            service_manager.ai_service.load_memory(result)
            print("成功调用记忆存储")
            return {
                "code": 200,
                "data": "success"
            }
        else:
            return {
                "code": 500,
                "msg": "Failed to load user conversations",
                "error": "加载的数据是空的"
            }
        
    except Exception as e:
        print("创建conversation的时候出错")
        print(str(e))
        return {
            "code": 500,
            "msg": "Failed to load user conversations",
            "error": str(e)
        }
    
@router.post("/create")  # 改为POST方法
async def create_user_conversations(request: Request):
    try:
        # 从请求体获取JSON数据
        payload = await request.json()
        user_id = payload.get("user_id")
        title = payload.get("title")
        
        # 参数验证
        if not user_id or not title:
            raise HTTPException(status_code=400, detail="缺少必要参数")
        
        # 获取消息记忆
        messages = service_manager.ai_service.get_memory()
        if not messages:  # 处理空消息情况
            print("消息记录是空的，请检查错误！！！！！！！！")
        
        # 创建对话
        conversation_id = ConversationModel.create_conversation(
            user_id=user_id,
            messages=messages,
            title=title
        )
        
        return {
            "code": 200,
            "data": {
                "conversation_id": conversation_id,
                "message": "对话创建成功"
            }
        }
        
    except HTTPException as he:
        raise he  # 重新抛出已处理的HTTP异常
    except Exception as e:
        print("创建conversation的时候出错")
        print(str(e))
        raise HTTPException(
            status_code=500,
            detail=f"创建对话失败: {str(e)}"
        )

@router.post("/save")
async def save_user_conversation(request: Request):
    """
    保存/更新现有对话
    请求体格式:
    {
        "user_id": int,
        "conversation_id": int,
        "title": str (可选)
    }
    """
    try:
        # 从请求体获取JSON数据
        payload = await request.json()
        user_id = payload.get("user_id")
        conversation_id = payload.get("conversation_id")
        title = payload.get("title")
        
        # 参数验证
        if not user_id or not conversation_id:
            raise HTTPException(status_code=400, detail="缺少必要参数(user_id或conversation_id)")
        
        # 获取当前消息记忆
        messages = service_manager.ai_service.get_memory()
        if not messages:
            print("警告: 消息记录是空的，将清空对话内容")
        
        # 更新对话
        ConversationModel.change_conversation_messages(
            conversation_id=conversation_id,
            messages=messages
        )
        
        # 如果需要更新标题
        if title:
            ConversationModel.update_conversation_title(conversation_id, title)
        
        return {
            "code": 200,
            "data": {
                "conversation_id": conversation_id,
                "message": "对话保存成功",
                "message_count": len(messages)
            }
        }
        
    except HTTPException as he:
        raise he
    except ValueError as ve:
        raise HTTPException(status_code=404, detail=str(ve))
    except Exception as e:
        raise HTTPException(
            status_code=500,
            detail=f"保存对话失败: {str(e)}"
        )

@router.post("/delete")
async def delete_user_conversation(request: Request):
    """
    删除用户对话
    请求体格式:
    {
        "user_id": int,
        "conversation_id": int
    }
    """
    try:
        # 从请求体获取JSON数据
        payload = await request.json()
        user_id = payload.get("user_id")
        conversation_id = payload.get("conversation_id")
        
        # 参数验证
        if not user_id or not conversation_id:
            raise HTTPException(status_code=400, detail="缺少必要参数(user_id或conversation_id)")
        
        # 执行删除
        deleted = ConversationModel.delete_conversation(conversation_id)
        
        if not deleted:
            raise HTTPException(status_code=404, detail="对话不存在或已被删除")
        
        return {
            "code": 200,
            "data": {
                "conversation_id": conversation_id,
                "message": "对话删除成功"
            }
        }
        
    except HTTPException as he:
        raise he
    except Exception as e:
        raise HTTPException(
            status_code=500,
            detail=f"删除对话失败: {str(e)}"
        )
    
@router.post("/process-log")
async def process_log_file(request: Request):
    try:
        data = await request.json()
        content = data.get('content')
        user_id = data.get('user_id')
        
        if not content or not user_id:
            raise HTTPException(status_code=400, detail="缺少必要参数")
        
        # 解析日志文件
        messages = parse_chat_log(content)
        if not messages:
            raise HTTPException(status_code=400, detail="日志文件未包含有效对话")
        
        # 添加到记忆系统
        service_manager.ai_service.load_memory(messages)
        
        # 同时创建新对话记录（可选）
        title = f"导入对话 {datetime.now().strftime('%Y-%m-%d %H:%M')}"
        conversation_id = ConversationModel.create_conversation(
            user_id=user_id,
            messages=messages,
            title=title
        )
        
        return {
            "success": True,
            "processed_count": len(messages),
            "conversation_id": conversation_id
        }
        
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"处理日志文件失败: {str(e)}")
```

### 文件: `backend/core/VitsTTS.py`

```python
import aiohttp
import asyncio
import os
from pathlib import Path
from .logger import Logger

class VitsTTS:
    def __init__(self, api_url=None, speaker_id=4, audio_format="wav", lang="ja", enable=True, logger=None):
        """
        初始化VITS语音合成器
        :param api_url: API端点地址
        :param speaker_id: 说话人ID (默认4)
        :param audio_format: 音频格式 (默认wav)
        :param lang: 语言代码 (默认ja-日语)
        :param logger: 日志记录器
        """
        self.logger = logger or Logger()
        self.api_url = api_url or os.environ.get("VITS_API_URL", "http://127.0.0.1:23456/voice/vits")
        self.speaker_id = speaker_id or int(os.environ.get("VITS_SPEAKER_ID", 4))
        self.format = audio_format
        self.lang = lang
        self.temp_dir = Path("temp_voice")
        self.temp_dir.mkdir(exist_ok=True)
        self.enable = enable
        
        # 检查语音服务可用性
        asyncio.run(self._check_service())

    async def _check_service(self):
        """检查语音服务是否可用"""
        error_message = None
        service_host_port = self.api_url.split('/')[2] 
        base_service_url = f"http://{service_host_port}" 

        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(self.api_url, timeout=2) as response:
                    if response.status < 500: 
                        self.logger.tts_status(True, f"vits-simple-api 服务可达 ({service_host_port}, 状态: {response.status})")
                        self.enable = True
                        return True
                    else:
                        error_message = f"vits-simple-api 主端点响应错误 (状态码: {response.status})"

        except aiohttp.ClientConnectorError as e:
            error_message = f"vits-simple-api 连接失败 (地址: {self.api_url})"
        except asyncio.TimeoutError:
            error_message = f"vits-simple-api 连接超时 (地址: {self.api_url})"
        except Exception as e: 
            error_detail = str(e)
            if not error_detail or error_detail == "()":
                error_message = "vits-simple-api 遇到未知连接错误"
            else:
                error_message = f"vits-simple-api 连接时遇到错误: {error_detail}"
        
        self.enable = False
        final_detail_message = "vits-simple-api 不可用或响应异常，语音功能将被禁用"
        if error_message:
            final_detail_message += f" ({error_message})"
        self.logger.tts_status(False, final_detail_message)
        return False

    async def generate_voice(self, text, file_name, save_file=False):
        """
        异步生成语音文件
        :param text: 要合成的文本
        :param save_file: 是否保留生成的音频文件
        :return: 音频文件路径 (失败返回None)
        """
        if not self.enable:
            self.logger.warning("TTS服务未启用，跳过语音生成")
            return None
            
        if not text or not text.strip():
            self.logger.debug("提供的文本为空，跳过语音生成")
            return None

        params = {
            "text": text,
            "id": self.speaker_id,
            "format": self.format,
            "lang": self.lang
        }

        output_file = str(file_name) 
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(self.api_url, params=params, timeout=10) as response:
                    response.raise_for_status()  
                    with open(output_file, "wb") as f:
                        f.write(await response.read())
                    self.logger.debug(f"语音生成成功: {os.path.basename(output_file)} (文本: \"{text}\")")
            return output_file
        except aiohttp.ClientResponseError as e: 
            self.logger.error(f"语音生成HTTP请求失败 (URL: {e.request_info.url}, 状态: {e.status}, 消息: {e.message}) 文本: \"{text}\"")
        except aiohttp.ClientError as e:  
            self.logger.error(f"语音生成网络或客户端错误 (类型: {type(e).__name__}, 消息: {str(e)}) 文本: \"{text}\"")
        except asyncio.TimeoutError: 
            self.logger.error(f"语音生成请求超时 (URL: {self.api_url}) 文本: \"{text}\"")
        except Exception as e: 
            error_type = type(e).__name__
            error_str = str(e)
            error_repr = repr(e)
            log_msg = f"语音生成时发生未知错误 (类型: {error_type}"
            if error_str and error_str.strip():
                log_msg += f", 消息: {error_str}"
            if error_repr and error_repr.strip() and error_repr != error_str:
                 log_msg += f", 详细: {error_repr}"
            log_msg += f") 文本: \"{text}\""
            self.logger.error(log_msg)
        return None

    def play_voice(self, text, auto_cleanup=True):
        """
        生成并立即播放语音
        :param text: 要合成的文本
        :param auto_cleanup: 播放后是否自动删除文件
        :return: 是否成功
        """
        loop = asyncio.get_event_loop()
        audio_file = loop.run_until_complete(self.async_generate_voice(text, "temp"))
        if not audio_file:
            return False

        try:
            return True
        except Exception as e:
            print(f"[VitsTTS] 播放失败: {str(e)}")
        finally:
            if auto_cleanup and audio_file and os.path.exists(audio_file):
                os.remove(audio_file)
        return False

    def cleanup(self):
        """清理所有临时文件"""
        for file in self.temp_dir.glob(f"*.{self.format}"):
            try:
                file.unlink()
            except Exception as e:
                self.logger.warning(f"清理临时文件失败 {file.name}: {str(e)}")

    def __del__(self):
        """析构时自动清理"""
        self.cleanup()

# 使用示例
if __name__ == "__main__":
    logger = Logger()
    tts = VitsTTS(speaker_id=4, logger=logger)
    
    # 异步生成
    async def test_voice():
        audio = await tts.generate_voice("こんにちは", "greeting.wav")
        if audio:
            print(f"生成成功: {audio}")
    
    asyncio.run(test_voice())
```

### 文件: `backend/core/__init__.py`

```python
from .ai_service import AIService
from .frontend_manager import FrontendManager
from .websocket_server import WebSocketServer

__version__ = "1.0.0"

__doc__ = """
核心功能模块包
包含以下主要组件：
- AIService: AI对话服务
- FrontendManager: 前端进程管理
- WebSocketServer: WebSocket服务
"""
```

### 文件: `backend/core/ai_service.py`

```python
import os
import glob
import asyncio
import re
from typing import List, Dict, Optional

from .deepseek import DeepSeek
from .predictor import EmotionClassifier  # 导入情绪分类器
from .VitsTTS import VitsTTS              # 导入语音生成
from .logger import Logger
from .langDetect import LangDetect

# 常量定义
TEMP_VOICE_DIR = "../public/audio"
WS_HOST = "localhost"
WS_PORT = 8765

# ANSI 颜色代码
COLOR = {
    "user": "\033[92m",
    "ai": "\033[96m",
    "emotion": "\033[93m",
    "reset": "\033[0m"
}

class AIService:
    def __init__(self, logger=None):
        """初始化所有服务组件"""
        self.logger = logger or Logger()
        self.deepseek = DeepSeek(logger=self.logger)
        self.emotion_classifier = EmotionClassifier(logger=self.logger)
        self.lang_detector = LangDetect()
        self.tts_engine = self._init_tts_engine()
        self._prepare_directories()

        self.temp_voice_dir = os.environ.get("TEMP_VOICE_DIR", "frontend/public/audio")
        os.makedirs(self.temp_voice_dir, exist_ok=True)
        
    def _init_tts_engine(self) -> VitsTTS:
        """初始化TTS引擎"""
        return VitsTTS(
            api_url="http://127.0.0.1:23456/voice/vits",
            speaker_id=4,
            lang="ja",
            logger=self.logger
        )
    
    def _prepare_directories(self):
        """准备必要的目录"""
        os.makedirs(TEMP_VOICE_DIR, exist_ok=True)
    
    async def process_message(self, user_message: str) -> Optional[List[Dict]]:
        """处理用户消息的完整流程"""
        try:
            # 1. 获取AI回复
            ai_response = self.deepseek.process_message(user_message)
            self._log_conversation("用户", user_message)
            self._log_conversation("钦灵", ai_response)
            
            # 2. 分析情绪和生成语音
            return await self._process_ai_response(ai_response, user_message)
        except Exception as e:
            self.logger.error(f"处理消息时出错: {e}")
            return None
    
    def load_memory(self, memory):
        self.deepseek.load_memory(memory)
        self.logger.info("新的记忆已经被加载")
    
    async def _process_ai_response(self, ai_response: str, user_message: str) -> List[Dict]:
        """处理AI回复的完整流程"""
        self._clean_temp_voice_files()
        
        # 分析情绪片段
        emotion_segments = self._analyze_emotions(ai_response)
        if not emotion_segments:
            self.logger.warning("未检测到有效情绪片段")
            raise ValueError("未检测到有效情绪片段")
        
        # 生成语音和构造响应
        await self._generate_voice_files(emotion_segments)
        responses = self._create_responses(emotion_segments, user_message)
    
        self.logger.debug("--- AI 回复分析结果 ---")
        self._log_analysis_result(emotion_segments)
        self.logger.debug("--- 分析结束 ---")

        return responses
    
    def _analyze_emotions(self, text: str) -> List[Dict]:
        """分析文本中每个【】标记的情绪，并提取日语和中文部分"""
        # 改进后的正则表达式，更灵活地匹配各种情况
        emotion_segments = re.findall(r'(【(.*?)】)([^【】]*)', text)

        results = []
        for i, (full_tag, emotion_tag, following_text) in enumerate(emotion_segments, 1):
            # 统一处理括号（兼容中英文括号）
            following_text = following_text.replace('(', '（').replace(')', '）')

            # 提取日语部分（<...>），改进匹配模式
            japanese_match = re.search(r'<(.*?)>', following_text)
            japanese_text = japanese_match.group(1).strip() if japanese_match else ""

            # 提取动作部分（（...）），改进匹配模式
            motion_match = re.search(r'（(.*?)）', following_text)
            motion_text = motion_match.group(1).strip() if motion_match else ""

            # 清理后的文本（移除日语部分和动作部分）
            cleaned_text = re.sub(r'<.*?>|（.*?）', '', following_text).strip()

            # 清理日语文本中的动作部分
            if japanese_text:
                japanese_text = re.sub(r'（.*?）', '', japanese_text).strip()

            # 跳过完全空的文本
            # 修改：如果cleaned_text和japanese_text都为空，但motion_text不为空，也应保留
            if not cleaned_text and not japanese_text and not motion_text:
                continue # 只有三者都为空时才跳过

            # 改进语言检测和处理
            try:
                if japanese_text and cleaned_text:
                    # 如果两者都有内容，才进行语言检测和交换
                    lang_jp = self.lang_detector.detect_language(japanese_text)
                    lang_clean = self.lang_detector.detect_language(cleaned_text)

                    if (lang_jp in ['Chinese', 'Chinese_ABS'] and lang_clean in ['Japanese', 'Chinese']) and \
                        lang_clean != 'Chinese_ABS':
                            cleaned_text, japanese_text = japanese_text, cleaned_text

            except Exception as e:
                # 语言检测失败时保持原样
                self.logger.warning(f"语言检测错误: {e}")

            # 对情绪标签单独预测，增加错误处理
            try:
                predicted = self.emotion_classifier.predict(emotion_tag)
                prediction_result = {
                    "label": predicted["label"],
                    "confidence": predicted["confidence"]
                }
            except Exception as e:
                self.logger.error(f"情绪预测错误 '{emotion_tag}': {e}")
                prediction_result = {
                    "label": "unknown",
                    "confidence": 0.0
                }

            results.append({
                "index": i,
                "original_tag": emotion_tag,
                "following_text": cleaned_text,
                "motion_text": motion_text,
                "japanese_text": japanese_text,
                "predicted": prediction_result["label"],
                "confidence": prediction_result["confidence"],
                # 使用 os.path.basename 确保只包含文件名
                "voice_file": os.path.join(self.temp_voice_dir, f"part_{i}.{self.tts_engine.format}")
            })

        return results
    
    async def _generate_voice_files(self, segments: List[Dict]):
        """生成语音文件"""
        tasks = [
            self.tts_engine.generate_voice(seg["japanese_text"], seg["voice_file"], True)
            for seg in segments if seg["japanese_text"]
        ]
        await asyncio.gather(*tasks)
    
    def _create_responses(self, segments: List[Dict], user_message: str) -> List[Dict]:
        """构造响应消息"""
        total_parts = len(segments)
        return [{
            "type": "reply",
            "emotion": seg['predicted'],
            "originalTag": seg['original_tag'],
            "message": seg['following_text'],
            "motionText": seg['motion_text'],
            "audioFile": os.path.basename(seg['voice_file']) if os.path.exists(seg['voice_file']) else None,
            "originalMessage": user_message,
            "isMultiPart": total_parts > 1,
            "partIndex": idx,
            "totalParts": total_parts
        } for idx, seg in enumerate(segments)]
    
    def _clean_temp_voice_files(self):
        """清理临时语音文件"""
        for file in glob.glob(os.path.join(TEMP_VOICE_DIR, "*.wav")):
            try:
                os.remove(file)
            except OSError as e:
                self.logger.warning(f"删除文件 {file} 失败: {e}")
    
    def _log_conversation(self, speaker: str, message: str):
        """记录对话日志"""
        log_message = f"{speaker}: {message}"
        self.logger.info_white_text(log_message)
        self.logger.log_conversation(speaker, message)

    def _log_analysis_result(self, segments):
        """记录分析结果"""
        for segment in segments:
            self.logger.debug(f"\n分析结果 (片段 {segment['index']}):")
            self.logger.debug(f"  原始标记: 【{segment['original_tag']}】")
            self.logger.debug(f"  中文文本: {segment['following_text']}")
            if segment['motion_text']:
                self.logger.debug(f"  动作文本: （{segment['motion_text']}）")
            if segment['japanese_text']:
                self.logger.debug(f"  日文文本: <{segment['japanese_text']}>")
            self.logger.debug(f"  预测情绪: {segment['predicted']} (置信度: {segment['confidence']:.2%})")
            if os.path.exists(segment['voice_file']):
                self.logger.debug(f"  对应语音: {os.path.basename(segment['voice_file'])}")
            else:
                if segment['japanese_text']:
                    self.logger.debug(f"  对应语音: (未生成或生成失败)")
    
    def get_memory(self):
        return self.deepseek.get_messsages()
```

### 文件: `backend/core/deepseek.py`

```python
from openai import OpenAI
import os
import json
import copy
from .logger import Logger

class DeepSeek:
    def __init__(self, api_key=None, base_url=None, logger=None):
        self.logger = logger or Logger()
        
        # OpenAI API 初始化    
        api_key = api_key or os.environ.get("CHAT_API_KEY")
        base_url = base_url or os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com")
            
        self.client = OpenAI(api_key=api_key, base_url=base_url)

        self.settings = os.environ.get("SYSTEM_PROMPT")

        self.debug_mode = os.environ.get("DEBUG_MODE", "False").lower() == "true"
        
        # 强化系统指令
        self.messages = [
            {
                "role": "system", 
                "content": self.settings
            }
        ]
        
        self.logger.debug("DeepSeek LLM 服务已初始化")

    def process_message(self, user_input):
        if user_input.lower() in ["退出", "结束"]:
            self.logger.info("用户请求终止程序")
            return "程序终止"
            
        self.messages.append({"role": "user", "content": user_input})

        # 若Debug模式开启，则截取发送到llm的文字信息打印到终端
        if self.debug_mode:
            self.logger.debug("\n------ 开发者模式：以下信息被发送给了llm ------")
            for message in self.messages:
                self.logger.debug(f"Role: {message['role']}\nContent: {message['content']}\n")
            self.logger.debug("------ 结束 ------")

        try:
            self.logger.debug("正在发送请求到DeepSeek LLM...")
            response = self.client.chat.completions.create(
                model=os.environ.get("MODEL_TYPE"),
                messages=self.messages,
                stream=False
            )
            ai_response = response.choices[0].message.content
            self.messages.append({"role": "assistant", "content": ai_response})
            # self.logger.log_text(self.messages)
            self.logger.debug("成功获取LLM响应")

            return ai_response

        except Exception as e:
            self.logger.error(f"LLM请求失败: {str(e)}")
            return "ERROR"

    def load_memory(self, memory):
        if isinstance(memory, str):
            memory = json.loads(memory)  # 将JSON字符串转为Python列表
        self.messages = copy.deepcopy(memory)  # 使用深拷贝
        self.logger.info("记忆存档已经加载")
        self.logger.info(f"内容是：{memory}")
        self.logger.info(f"新的messages是：{self.messages}")

    def get_messsages(self):
        return self.messages
```

### 文件: `backend/core/frontend_manager.py`

```python
# frontend_manager.py
import os
import subprocess
import webbrowser
import time
import atexit
import signal
import sys
from typing import Optional
from .logger import Logger

class FrontendManager:
    def __init__(self, logger: Optional[Logger] = None):
        self.process: Optional[subprocess.Popen] = None
        self._cleanup_called = False
        self.logger = logger or Logger()
        self._register_cleanup()

    def start_frontend(self, frontend_dir: str, port: str = "3000") -> bool:
        """启动前端开发服务器"""
        server_js_path = os.path.join(frontend_dir, "app.js")
        
        if not os.path.isfile(server_js_path):
            self.logger.error(f"前端脚本未找到: {server_js_path}")
            return False

        try:
            self.process = subprocess.Popen(
                ["node", os.path.basename(server_js_path)],
                cwd=frontend_dir,
                creationflags=self._get_creation_flags(),
                start_new_session=sys.platform != "win32"
            )
            self.logger.info(f"前端服务器已启动 (PID: {self.process.pid})")
            
            time.sleep(3)  # 等待服务器启动
            if self.process.poll() is not None:
                self.logger.error(f"前端服务器退出，错误码: {self.process.poll()}")
                return False
            
            self._open_browser(f"http://localhost:{port}")
            return True
            
        except FileNotFoundError:
            self.logger.error("未找到'node'命令. 请安装Node.js")
            return False
        except Exception as e:
            self.logger.error(f"启动前端失败: {e}")
            return False

    def stop_frontend(self):
        """停止前端服务器"""
        if self._cleanup_called or not self.process:
            return

        self._cleanup_called = True
        self.logger.info("正在停止前端服务器...")
        
        try:
            if self.process.poll() is None:
                self.process.terminate()
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=2)
        except Exception as e:
            self.logger.error(f"停止前端出错: {e}")
        finally:
            self.process = None

    def _open_browser(self, url: str):
        """尝试在浏览器中打开URL"""
        try:
            webbrowser.open(url)
            self.logger.info(f"已在浏览器中打开: {url}")
        except Exception as e:
            self.logger.error(f"打开浏览器失败: {e}")

    def _get_creation_flags(self):
        """获取平台特定的进程创建标志"""
        return subprocess.CREATE_NEW_PROCESS_GROUP if sys.platform == "win32" else 0

    def _register_cleanup(self):
        """注册清理函数"""
        atexit.register(self.stop_frontend)
        
        # 注册信号处理器
        signals = (signal.SIGINT, signal.SIGTERM)
        if sys.platform != "win32":
            signals += (signal.SIGHUP,)
        else:
            signals += (signal.SIGBREAK,)

        for sig in signals:
            try:
                signal.signal(sig, self._handle_signal)
            except (OSError, ValueError, AttributeError) as e:
                self.logger.warning(f"无法注册信号 {sig}: {e}")

    def _handle_signal(self, sig, frame):
        """处理操作系统信号"""
        self.logger.info(f"收到信号 {signal.Signals(sig).name}，正在关闭...")
        self.stop_frontend()
        sys.exit(0)
```

### 文件: `backend/core/langDetect.py`

```python
class LangDetect:
    def __init__(self):
        # 定义Unicode范围
        self.chinese_ranges = [
            (0x4E00, 0x9FFF),    # 基本汉字
            (0x3400, 0x4DBF),    # 扩展A
            (0x20000, 0x2A6DF),  # 扩展B
            (0x2A700, 0x2B73F),  # 扩展C
            (0x2B740, 0x2B81F),  # 扩展D
            (0x2B820, 0x2CEAF),  # 扩展E
            (0xF900, 0xFAFF),    # 兼容汉字
            (0x3300, 0x33FF),    # 兼容符号
        ]
        
        self.japanese_ranges = [
            (0x3040, 0x309F),    # 平假名
            (0x30A0, 0x30FF),    # 片假名
            (0x31F0, 0x31FF),    # 片假名音标扩展
            (0xFF65, 0xFF9F),    # 半角片假名
        ]

    def detect_language(self, text):
        """
        判断输入文本是中文还是日文
        
        参数:
            text (str): 要检测的文本
            
        返回:
            str: "Chinese", "Japanese" 或 "Unknown"
        """
        
        # 统计字符类型
        chinese_count = 0
        japanese_count = 0
        
        for char in text:
            code = ord(char)
            
            # 检查中文
            for start, end in self.chinese_ranges:
                if start <= code <= end:
                    chinese_count += 1
                    break
                    
            # 检查日文
            for start, end in self.japanese_ranges:
                if start <= code <= end:
                    japanese_count += 1
                    break
        
        # 判断结果
        if chinese_count > 0 and japanese_count == 0:
            return "Chinese_ABS"
        elif japanese_count < chinese_count:
            return "Chinese"
        else:
            return "Japanese"

# 测试示例
if __name__ == "__main__":
    detecter = LangDetect()

    test_cases = [
        ("你好，世界！", "Chinese"),
        ("こんにちは、世界！", "Japanese"),
        ("約束します...今後はどんな命令も...", "Unknown"),
        ("分かった...今後は...『毛選』を...同人誌カバーで包んで読むから...", "Japanese"),  # 混合情况
    ]
    
    for text, expected in test_cases:
        result = detecter.detect_language(text)
        print(f"输入: {text} | 检测结果: {result} | 预期: {expected} | {'✓' if result == expected else '✗'}")

    input("等你宝宝")
```

### 文件: `backend/core/logger.py`

```python
import os
import sys
import json
from datetime import datetime
from enum import Enum
from typing import Dict, Optional
from dotenv import load_dotenv

load_dotenv()

# 日志级别
class LogLevel(Enum):
    DEBUG = 0
    INFO = 1
    WARNING = 2
    ERROR = 3

class Color:
    BLACK = "\033[30m"
    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    MAGENTA = "\033[35m"
    CYAN = "\033[36m"
    WHITE = "\033[37m"
    BRIGHT_BLACK = "\033[90m"
    BRIGHT_RED = "\033[91m"
    BRIGHT_GREEN = "\033[92m"
    BRIGHT_YELLOW = "\033[93m"
    BRIGHT_BLUE = "\033[94m"
    BRIGHT_MAGENTA = "\033[95m"
    BRIGHT_CYAN = "\033[96m"
    BRIGHT_WHITE = "\033[97m"
    BOLD = "\033[1m"
    RESET = "\033[0m"

class Logger:
    def __init__(self):
        # 应用日志配置
        self.app_log_dir = os.environ.get("APP_LOG_DIR", "log")
        os.makedirs(self.app_log_dir, exist_ok=True)
        self.app_log_file = self._setup_app_logging()

        # 对话日志配置 (保持原有逻辑和环境变量 BACKEND_LOG_DIR)
        self.conversation_log_dir = os.environ.get("BACKEND_LOG_DIR", "logs")
        os.makedirs(self.conversation_log_dir, exist_ok=True)
        self.conversation_log_file = self._setup_conversation_logging()
        
        log_level_str = os.environ.get("LOG_LEVEL", "INFO").upper()
        try:
            self.log_level = LogLevel[log_level_str]
        except KeyError:
            self.log_level = LogLevel.INFO
            print(f"{Color.YELLOW}无效的日志级别: {log_level_str}，使用默认级别 INFO{Color.RESET}")
        
        self.level_config = {
            LogLevel.DEBUG: {"color": Color.BRIGHT_BLACK, "prefix": "DEBUG"},
            LogLevel.INFO: {"color": Color.BRIGHT_GREEN, "prefix": "INFO"},
            LogLevel.WARNING: {"color": Color.BRIGHT_YELLOW, "prefix": "WARN"},
            LogLevel.ERROR: {"color": Color.BRIGHT_RED, "prefix": "ERROR"}
        }
        
        self.status_colors = {
            "success": Color.GREEN,
            "error": Color.RED,
            "warning": Color.YELLOW
        }
    
    def _setup_app_logging(self):
        """配置应用程序日志文件路径 (例如: log/0.log)"""
        try:
            existing_logs = [f for f in os.listdir(self.app_log_dir) if f.endswith('.log') and f[:-4].isdigit()]
            next_num = 0
            if existing_logs:
                next_num = max(int(f[:-4]) for f in existing_logs) + 1
        except FileNotFoundError:
            # 如果目录刚创建，listdir可能会在此刻失败（尽管 makedirs exist_ok=True）
            # 或其他权限问题，安全起见，从0开始
            existing_logs = []
            next_num = 0
            
        app_log_file_path = os.path.join(self.app_log_dir, f"{next_num}.log")
        with open(app_log_file_path, 'a', encoding='utf-8') as f:
            f.write(f"--- Application Log Started: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')} ---\n")
        return app_log_file_path

    def _setup_conversation_logging(self):
        """配置对话日志文件路径 (例如: logs/0.log)，保持原有格式"""
        try:
            existing_logs = [f for f in os.listdir(self.conversation_log_dir) if f.endswith('.log') and f[:-4].isdigit()]
            next_num = 0
            if existing_logs:
                next_num = max(int(f[:-4]) for f in existing_logs) + 1
        except FileNotFoundError:
            existing_logs = []
            next_num = 0

        conv_log_file_path = os.path.join(self.conversation_log_dir, f"{next_num}.log")
        # 对话日志通常每个会话（或每次启动）是一个新文件，或追加到特定文件，这里保持了每次启动新文件并写日期的逻辑
        with open(conv_log_file_path, 'w', encoding='utf-8') as f: 
            f.write(f"对话日期: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        return conv_log_file_path

    def log_conversation(self, role, content):
        """记录对话内容到专门的对话日志文件"""
        with open(self.conversation_log_file, 'a', encoding='utf-8') as f:
            f.write(f"{role}: {content}\n\n")

    def _log(self, level: LogLevel, message: str, file_output: bool = True, force_message_color: Optional[str] = None):
        """基础日志输出函数"""
        if level.value < self.log_level.value:
            return
        
        config = self.level_config[level]
        now = datetime.now().strftime('%H:%M:%S')
        
        colored_log_prefix = f"{config['color']}[{config['prefix']}]{Color.RESET}"
        
        message_body_color = force_message_color if force_message_color else config['color']
        if force_message_color == Color.WHITE: 
            message_body_color = Color.WHITE
        
        console_message = f"{colored_log_prefix} {message_body_color}{message}{Color.RESET}"
        print(console_message)
        
        if file_output:
            plain_log_prefix = f"[{config['prefix']}]" 
            with open(self.app_log_file, 'a', encoding='utf-8') as f:
                f.write(f"{now} {plain_log_prefix} {message}\n")

    def log_text(self, message: str):
        """输出默认颜色（白色）的文本，不添加前缀"""
        print(message)

    def debug(self, message: str):
        """调试级别日志"""
        self._log(LogLevel.DEBUG, message)

    def info(self, message: str):
        """信息级别日志"""
        self._log(LogLevel.INFO, message)

    def info_white_text(self, message: str):
        """INFO级别日志，但消息文本为白色"""
        self._log(LogLevel.INFO, message, force_message_color=Color.WHITE)

    def warning(self, message: str):
        """警告级别日志"""
        self._log(LogLevel.WARNING, message)

    def error(self, message: str):
        """错误级别日志"""
        self._log(LogLevel.ERROR, message)

    def service_status(self, service_name: str, is_running: bool, details: Optional[str] = None, status_type: str = None):
        """输出服务状态信息（成功/失败）"""
        status_type = status_type or ("success" if is_running else "error")
        status_text = f"已{'运行' if is_running else '停止'}"
        color = self.status_colors.get(status_type, Color.RESET)
        
        status_part = f"{color}[{service_name} {status_text}]{Color.RESET}" 
        
        cleaned_details = details
        if details and details.strip() == "()":
            cleaned_details = None 
        elif details:
            cleaned_details = details.strip()

        if cleaned_details:
            message_to_log = f"{status_part} {Color.WHITE}{cleaned_details}{Color.RESET}"
        else:
            message_to_log = status_part
        

        level = LogLevel.INFO if is_running else LogLevel.WARNING
        self._log(level, message_to_log, file_output=False, force_message_color=Color.WHITE if cleaned_details else None)
        
    def emotion_model_status(self, is_success: bool, details: Optional[str] = None):
        """情绪模型加载状态"""
        status = "情绪分类模型加载正常" if is_success else "情绪分类模型加载异常"
        self.service_status(status, is_success, details, "success" if is_success else "error")
    
    def tts_status(self, is_running: bool, details: Optional[str] = None):
        """语音服务状态"""
        status = "语音服务已运行" if is_running else "语音服务未运行"
        cleaned_details = details
        if details:
            details_str = str(details).strip()
            if details_str == "()" or details_str == "('()',)":
                cleaned_details = None
            elif "vits-simple-api未运行" in details_str and "语音功能将被禁用" in details_str:
                import re
                match = re.search(r'\((.*?)\)', details_str)
                if match and not match.group(1).strip(): 
                    cleaned_details = details_str.split('(')[0].strip()
                else:
                    cleaned_details = details_str 

        self.service_status(status, is_running, cleaned_details, "success" if is_running else "error")
    
    def backend_status(self, is_running: bool, details: Optional[str] = None):
        """后端服务状态"""
        status = "后端服务已启动" if is_running else "后端服务未启动"
        self.service_status(status, is_running, details, "success" if is_running else "error")
    
    def client_message(self, message: Dict):
        """记录客户端消息"""
        try:
            message_str = json.dumps(message, ensure_ascii=False)
            self.debug(f"收到原始客户端消息: {message_str}")
        except TypeError:
            self.debug(f"收到原始客户端消息 (无法JSON序列化): {message}")
    
    def client_connect(self):
        """记录客户端连接"""
        self.info(f"新的客户端连接建立")
    
    def client_disconnect(self):
        """记录客户端断开连接"""
        self.info(f"客户端断开连接")
    
    def response_info(self, count: int):
        """记录响应信息"""
        self.info(f"准备发送 {count} 条回复")
```

### 文件: `backend/core/predictor.py`

```python
from transformers import BertTokenizer, BertForSequenceClassification
import torch
import os
import json
from pathlib import Path
from .logger import Logger

class EmotionClassifier:
    def __init__(self, model_path=None, logger=None):
        """加载情绪分类模型"""
        self.logger = logger or Logger()
        
        # 加载模型和分词器
        try:
            model_path = model_path or os.environ.get("EMOTION_MODEL_PATH", "./emotion_model_18emo")
            model_path = Path(model_path).resolve()
            self.tokenizer = BertTokenizer.from_pretrained(model_path, local_files_only=True)
            self.model = BertForSequenceClassification.from_pretrained(model_path)
            self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
            self.model.to(self.device)
            
            # 从保存的配置加载标签映射
            config_path = os.path.join(model_path, "label_mapping.json")
            with open(config_path, "r", encoding='utf-8') as f: 
                label_config = json.load(f)
            self.id2label = label_config["id2label"]
            self.label2id = label_config["label2id"]
            
            self._log_label_mapping()
            self.logger.emotion_model_status(True, f"已成功加载情绪分类模型: {model_path.name}")
        except Exception as e:
            self.logger.emotion_model_status(False, f"加载情绪分类模型失败: {e}")
            self.id2label = {}
            self.label2id = {}

    def _log_label_mapping(self):
        """记录标签映射关系"""
        self.logger.debug("\n加载的标签映射关系:")
        for id, label in self.id2label.items():
            self.logger.debug(f"{id}: {label}")

    def predict(self, text, confidence_threshold=0.08):
        """预测文本情绪（带置信度阈值过滤）"""
        try:
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
                self.logger.debug(f"情绪识别置信度低: {text} -> 不确定 ({pred_prob:.2%})")
                return {
                    "label": "不确定",
                    "confidence": pred_prob,
                    "top3": top3,
                    "warning": f"置信度低于阈值({confidence_threshold:.0%})"
                }
            
            label = self.id2label.get(str(pred_id), "未知")
            self.logger.debug(f"情绪识别: {text} -> {label} ({pred_prob:.2%})")
            return {
                "label": label,
                "confidence": pred_prob,
                "top3": top3
            }
        except Exception as e:
            self.logger.error(f"情绪预测错误: {e}")
            return {
                "label": "未知",
                "confidence": 0.0,
                "top3": [],
                "error": str(e)
            }

    def _get_top3(self, probs):
        """获取概率最高的3个结果"""
        top3_probs, top3_ids = torch.topk(probs, 3)
        return [
            {
                "label": self.id2label.get(str(idx.item()), "未知"),
                "probability": prob.item()
            }
            for prob, idx in zip(top3_probs[0], top3_ids[0])
        ]

def main():
    print("【情绪分类器】")
    print("="*40)
    
    # 初始化分类器
    try:
        logger = Logger()
        classifier = EmotionClassifier(logger=logger)
        print("\n模型加载成功！输入文本进行分析，输入 ':q' 退出")
    except Exception as e:
        print(f"\n模型加载失败: {str(e)}")
        print("请检查：")
        print("1. 模型路径是否存在")
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

### 文件: `backend/core/service_manager.py`

```python
from .ai_service import AIService
import os

class ServiceManager:
    _instance = None
    
    def __init__(self):
        self.ai_service = AIService()
        print(f"🧠🧠🧠 ai_service 初始化，进程 ID: {os.getpid()}")
    
    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

service_manager = ServiceManager.get_instance()
```

### 文件: `backend/core/websocket_server.py`

```python
# websocket_server.py
import websockets
import json
from typing import Callable, Optional
from .logger import Logger

class WebSocketServer:
    def __init__(self, 
                 host: str = "0.0.0.0", 
                 port: int = 8765,
                 message_handler: Optional[Callable] = None,
                 logger: Optional[Logger] = None):
        self.host = host
        self.port = port
        self.server = None
        self.message_handler = message_handler
        self.logger = logger or Logger()

    async def _handle_client(self, websocket):
        """处理客户端连接"""
        self.logger.debug("新的客户端连接")
        self.logger.client_connect()
        
        try:
            async for message in websocket:
                try:
                    data = json.loads(message)
                    if self.message_handler:
                        await self.message_handler(websocket, data)
                except json.JSONDecodeError:
                    self.logger.error("收到无效的JSON数据")
                except Exception as e:
                    self.logger.error(f"处理消息时出错: {e}")

        except websockets.exceptions.ConnectionClosed:
            self.logger.client_disconnect()

    async def start(self):
        """启动WebSocket服务器"""
        self.server = await websockets.serve(
            self._handle_client,
            self.host,
            self.port,
            ping_interval=20,
            max_size=2**25
        )
        self.logger.backend_status(True, f"ws://{self.host}:{self.port}")

    async def stop(self):
        """停止WebSocket服务器"""
        if self.server:
            self.server.close()
            await self.server.wait_closed()
            self.logger.backend_status(False, "服务已停止")
```

### 文件: `backend/database/__init__.py`

```python
from .database import init_db

# 包被导入时初始化数据库
init_db()
```

### 文件: `backend/database/conversation_model.py`

```python
from typing import List, Dict, Optional
from .database import get_db_connection, Role
import json


class ConversationModel:
    @staticmethod
    def create_conversation(user_id: int, messages: List[Dict[str, str]], title: Optional[str] = None) -> int:
        """
        将完整对话插入数据库
        :param user_id: 所属用户ID
        :param messages: 消息列表，每条为{"role": "user/assistant/system", "content": "..."}
        :param title: 可选对话标题
        :return: conversation_id
        """
        if not messages:
            raise ValueError("消息列表不能为空")

        # 自动生成标题
        if not title:
            first_user_msg = next((m for m in messages if m["role"] == Role.USER.value), None)
            title = (first_user_msg["content"][:20] + "...") if first_user_msg else "New Conversation"

        conn = get_db_connection()
        # 优化设置（仅用于批量导入）
        conn.execute("PRAGMA synchronous = OFF")
        conn.execute("PRAGMA journal_mode = MEMORY")
        conn.execute("PRAGMA cache_size = 100000")
        
        cursor = conn.cursor()
        
        try:
            # 1. 插入对话
            cursor.execute(
                "INSERT INTO conversations (title, owned_user) VALUES (?, ?)",
                (title, user_id)
            )
            conversation_id = cursor.lastrowid
            
            # 2. 批量插入消息（分批次防止SQL过长）
            BATCH_SIZE = 500  # 每次插入500条
            msg_ids = []
            
            for i in range(0, len(messages), BATCH_SIZE):
                batch = messages[i:i+BATCH_SIZE]
                placeholders = ",".join(["(?,?,?)"]*len(batch))
                params = []
                for msg in batch:
                    params.extend([msg["role"], msg["content"], conversation_id])
                
                cursor.execute(
                    f"INSERT INTO messages (role, content, owned_conversation) VALUES {placeholders}",
                    params
                )
                
                # 获取这批消息的ID（假设是连续分配的）
                first_id = cursor.lastrowid - len(batch) + 1
                msg_ids.extend(range(first_id, first_id + len(batch)))
            
            # 3. 批量插入关系（同样分批次）
            relations = [(msg_ids[i], msg_ids[i+1]) for i in range(len(msg_ids)-1)]
            for i in range(0, len(relations), BATCH_SIZE):
                batch = relations[i:i+BATCH_SIZE]
                placeholders = ",".join(["(?,?)"]*len(batch))
                params = [item for pair in batch for item in pair]
                
                cursor.execute(
                    f"INSERT INTO message_relations (parent_id, child_id) VALUES {placeholders}",
                    params
                )
            
            # 4. 更新最后消息
            cursor.execute(
                "UPDATE conversations SET last_message_id=?, updated_at=datetime('now') WHERE id=?",
                (msg_ids[-1], conversation_id)
            )
            
            conn.commit()
            return conversation_id
        
        except Exception as e:
            conn.rollback()
            raise e
        finally:
            # 恢复默认设置
            conn.execute("PRAGMA synchronous = NORMAL")
            conn.execute("PRAGMA journal_mode = DELETE")
            conn.close()

    @staticmethod
    def append_messages_to_conversation(conversation_id: int, messages: List[Dict[str, str]]) -> None:
        """
        向已有对话中追加新消息（自动建立父子关系、更新最后消息 ID）
        """
        if not messages:
            raise ValueError("追加的消息列表不能为空")

        conn = get_db_connection()
        cursor = conn.cursor()

        try:
            # 获取当前对话存在性和最后一条消息 ID
            cursor.execute("SELECT last_message_id FROM conversations WHERE id = ?", (conversation_id,))
            row = cursor.fetchone()
            if not row:
                raise ValueError(f"对话 ID {conversation_id} 不存在")
            prev_msg_id = row["last_message_id"]

            last_msg_id = None

            for msg in messages:
                role = msg["role"]
                content = msg["content"]

                # 插入消息
                cursor.execute(
                    "INSERT INTO messages (role, content, owned_conversation) VALUES (?, ?, ?)",
                    (role, content, conversation_id)
                )
                current_msg_id = cursor.lastrowid

                # 建立父子关系（如果不是第一条追加）
                if prev_msg_id:
                    cursor.execute(
                        "INSERT INTO message_relations (parent_id, child_id) VALUES (?, ?)",
                        (prev_msg_id, current_msg_id)
                    )

                prev_msg_id = current_msg_id
                last_msg_id = current_msg_id

            # 更新对话最后消息 ID 和更新时间
            cursor.execute(
                "UPDATE conversations SET last_message_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                (last_msg_id, conversation_id)
            )

            conn.commit()
            print(f"成功向对话 {conversation_id} 追加了 {len(messages)} 条消息。")

        except Exception as e:
            conn.rollback()
            raise e

        finally:
            conn.close()
    
    @staticmethod
    def change_conversation_messages(conversation_id: int, messages: List[Dict[str, str]]) -> None:
        """
        完全替换对话中的消息（删除原有消息，插入新消息）
        :param conversation_id: 要修改的对话ID
        :param messages: 新的消息列表，每条为{"role": "user/assistant/system", "content": "..."}
        """
        if not messages:
            raise ValueError("消息列表不能为空")

        conn = get_db_connection()
        # 优化设置（仅用于批量操作）
        conn.execute("PRAGMA synchronous = OFF")
        conn.execute("PRAGMA journal_mode = MEMORY")
        conn.execute("PRAGMA cache_size = 100000")
        
        cursor = conn.cursor()
        
        try:
            # 1. 验证对话存在性
            cursor.execute("SELECT id FROM conversations WHERE id = ?", (conversation_id,))
            if not cursor.fetchone():
                raise ValueError(f"对话 ID {conversation_id} 不存在")

            # 2. 删除原有消息和关系（外键约束应该会自动删除关系）
            cursor.execute("DELETE FROM messages WHERE owned_conversation = ?", (conversation_id,))
            
            # 3. 批量插入新消息（分批次防止SQL过长）
            BATCH_SIZE = 500  # 每次插入500条
            msg_ids = []
            
            for i in range(0, len(messages), BATCH_SIZE):
                batch = messages[i:i+BATCH_SIZE]
                placeholders = ",".join(["(?,?,?)"]*len(batch))
                params = []
                for msg in batch:
                    params.extend([msg["role"], msg["content"], conversation_id])
                
                cursor.execute(
                    f"INSERT INTO messages (role, content, owned_conversation) VALUES {placeholders}",
                    params
                )
                
                # 获取这批消息的ID（假设是连续分配的）
                first_id = cursor.lastrowid - len(batch) + 1
                msg_ids.extend(range(first_id, first_id + len(batch)))
            
            # 4. 批量插入新关系
            relations = [(msg_ids[i], msg_ids[i+1]) for i in range(len(msg_ids)-1)]
            for i in range(0, len(relations), BATCH_SIZE):
                batch = relations[i:i+BATCH_SIZE]
                placeholders = ",".join(["(?,?)"]*len(batch))
                params = [item for pair in batch for item in pair]
                
                cursor.execute(
                    f"INSERT INTO message_relations (parent_id, child_id) VALUES {placeholders}",
                    params
                )
            
            # 5. 更新最后消息和修改时间
            cursor.execute(
                "UPDATE conversations SET last_message_id = ?, updated_at = datetime('now') WHERE id = ?",
                (msg_ids[-1], conversation_id)
            )
            
            conn.commit()
            print(f"成功替换对话 {conversation_id} 的消息，共 {len(messages)} 条")

        except Exception as e:
            conn.rollback()
            raise e
        finally:
            # 恢复默认设置
            conn.execute("PRAGMA synchronous = NORMAL")
            conn.execute("PRAGMA journal_mode = DELETE")
            conn.close()

    
    @staticmethod
    def get_conversation_messages(conversation_id: int) -> str:
        """
        高效获取对话的完整消息链：从最后一条消息向上追溯所有父节点，构建顺序列表。
        一次性查询所有涉及的消息与关系，避免多次数据库调用。
        """
        conn = get_db_connection()
        cursor = conn.cursor()

        # 获取最后一条消息 ID
        cursor.execute(
            "SELECT last_message_id FROM conversations WHERE id = ?",
            (conversation_id,)
        )
        result = cursor.fetchone()
        if not result or not result["last_message_id"]:
            conn.close()
            print("该对话没有消息。")
            return "[]"
        
        last_msg_id = result["last_message_id"]

        # 获取当前对话中所有消息
        cursor.execute(
            "SELECT id, role, content FROM messages WHERE owned_conversation = ?",
            (conversation_id,)
        )
        all_messages = {row["id"]: {"role": row["role"], "content": row["content"]} for row in cursor.fetchall()}

        # 获取所有 message_relations 映射 child -> parent
        cursor.execute(
            "SELECT parent_id, child_id FROM message_relations "
            "WHERE child_id IN (SELECT id FROM messages WHERE owned_conversation = ?)",
            (conversation_id,)
        )
        child_to_parent = {row["child_id"]: row["parent_id"] for row in cursor.fetchall()}

        conn.close()

        # 从最后一条消息向上回溯链条
        message_chain = []
        current_id = last_msg_id
        while current_id:
            msg = all_messages.get(current_id)
            if msg:
                message_chain.append(msg)
            current_id = child_to_parent.get(current_id)

        message_chain.reverse()  # 转换为从头到尾顺序

        json_output = json.dumps(message_chain, ensure_ascii=False, indent=2)

        return json_output

    @staticmethod
    def update_conversation_title(conversation_id: int, title: str) -> None:
        """更新对话标题"""
        conn = get_db_connection()
        try:
            conn.execute(
                "UPDATE conversations SET title = ?, updated_at = datetime('now') WHERE id = ?",
                (title, conversation_id)
            )
            conn.commit()
        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()
    
    @staticmethod
    def delete_conversation(conversation_id: int) -> bool:
        """
        删除对话及其所有关联消息
        :param conversation_id: 要删除的对话ID
        :return: 是否删除成功
        """
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute("PRAGMA foreign_keys = ON")  # 必须对每个连接都开启
        
        try:
            # 由于外键约束，删除对话会自动删除关联的消息和关系
            cursor.execute(
                "DELETE FROM conversations WHERE id = ?",
                (conversation_id,)
            )
            
            # 检查是否真的删除了记录
            deleted = cursor.rowcount > 0
            
            conn.commit()
            return deleted
            
        except Exception as e:
            conn.rollback()
            raise e
        finally:
            conn.close()
```

### 文件: `backend/database/database.py`

```python
import sqlite3
from datetime import datetime
from enum import Enum

DB_NAME = "chat_system.db"


class Role(Enum):
    SYSTEM = "system"
    USER = "user"
    ASSISTANT = "assistant"


def init_db():
    conn = sqlite3.connect(DB_NAME)
    cursor = conn.cursor()

    # 启用外键支持
    cursor.execute("PRAGMA foreign_keys = ON")

    # 创建用户表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        username TEXT UNIQUE NOT NULL,
        password TEXT NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
    """)

    # 创建对话表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS conversations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL DEFAULT 'New Conversation',
        last_message_id INTEGER,
        owned_user INTEGER NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (owned_user) REFERENCES users(id) ON DELETE CASCADE,
        FOREIGN KEY (last_message_id) REFERENCES messages(id) ON DELETE SET NULL
    )
    """)

    # 创建消息表
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant')),
        content TEXT NOT NULL,
        owned_conversation INTEGER NOT NULL,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (owned_conversation) REFERENCES conversations(id) ON DELETE CASCADE
    )
    """)

    # 创建消息关系表（替代原来的parent_message_ids）
    cursor.execute("""
    CREATE TABLE IF NOT EXISTS message_relations (
        parent_id INTEGER NOT NULL,
        child_id INTEGER NOT NULL,
        PRIMARY KEY (parent_id, child_id),
        FOREIGN KEY (parent_id) REFERENCES messages(id) ON DELETE CASCADE,
        FOREIGN KEY (child_id) REFERENCES messages(id) ON DELETE CASCADE
    )
    """)

    # 创建索引提高查询性能
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_conversation_user ON conversations(owned_user)")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_message_conversation ON messages(owned_conversation)")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_message_relations_parent ON message_relations(parent_id)")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_message_relations_child ON message_relations(child_id)")

    conn.commit()
    conn.close()


def get_db_connection():
    conn = sqlite3.connect(DB_NAME)
    conn.row_factory = sqlite3.Row  # 允许以字典方式访问结果
    return conn


def main():
    # 初始化数据库
    init_db()
    
    # 测试数据库连接和表结构
    conn = get_db_connection()
    cursor = conn.cursor()
    
    # 检查表是否存在
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
    tables = cursor.fetchall()
    print("数据库中的表:")
    for table in tables:
        print(table['name'])
    
    # 显示每个表的结构
    for table in tables:
        print(f"\n{table['name']}表结构:")
        cursor.execute(f"PRAGMA table_info({table['name']})")
        columns = cursor.fetchall()
        for col in columns:
            print(f"{col['name']}: {col['type']}")
    
    conn.close()
    print("\n数据库初始化完成，表结构验证通过。")


if __name__ == "__main__":
    main()
```

### 文件: `backend/database/test.py`

```python
from database.conversation_model import ConversationModel

# example_messages = [
#     {"role": "system", "content": "你是一个狼娘"},
#     {"role": "user", "content": "你好呀灵灵"},
#     {"role": "assistant", "content": "【高兴】啊，莱姆你来啦！..."},
#     {"role": "user", "content": "想看你变成黄油的样子哦"},
#     {"role": "assistant", "content": "【羞耻】呜...莱姆你在说什么呀！..."},
# ]

# user_id = 1  # 假设已有用户
# ConversationModel.load_conversation(user_id=user_id, messages=example_messages)

conversation_id = 1  # 你想查询的 conversation ID
json_output = ConversationModel.get_conversation_messages(conversation_id)

print("历史记录是")
print(json_output)

ConversationModel.append_messages_to_conversation(
    conversation_id=1,
    messages=[
        {"role": "user", "content": "我可以草草你吗？"},
        {"role": "assistant", "content": "【情动】今天破例一次哦~"}
    ]
)

json_output = ConversationModel.get_conversation_messages(conversation_id)
print("历史记录是")
print(json_output)
```

### 文件: `backend/database/test_user.py`

```python
from database.user_model import UserModel, UserConversationModel

# 创建用户
# try:
#     user_id = UserModel.create_user("alice", "securepassword123")
#     print(f"创建用户成功，ID: {user_id}")
# except ValueError as ve:
#     print("用户创建失败：", ve)

# 获取用户的所有对话

user_id = 1
conversations = UserConversationModel.get_user_conversations(user_id)
for conv in conversations:
    print(f"对话ID: {conv['id']}, 标题: {conv['title']}, 更新时间: {conv['updated_at']}")
```

### 文件: `backend/database/user_model.py`

```python
from .database import get_db_connection
from typing import Optional, List, Dict
import hashlib


class UserModel:
    @staticmethod
    def create_user(username: str, password: str) -> Optional[int]:
        """
        创建新用户，用户名必须唯一。
        """
        conn = get_db_connection()
        cursor = conn.cursor()

        # 检查用户名是否存在
        cursor.execute("SELECT id FROM users WHERE username = ?", (username,))
        if cursor.fetchone():
            conn.close()
            raise ValueError(f"用户名 '{username}' 已存在")

        # 暂时懒得加盐了，测试一下看看对不对
        hashed_password = password

        cursor.execute(
            "INSERT INTO users (username, password) VALUES (?, ?)",
            (username, hashed_password)
        )
        user_id = cursor.lastrowid
        conn.commit()
        conn.close()
        return user_id

    @staticmethod
    def get_user_by_id(user_id: int) -> Optional[Dict]:
        conn = get_db_connection()
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM users WHERE id = ?", (user_id,))
        row = cursor.fetchone()
        conn.close()
        return dict(row) if row else None


class UserConversationModel:
    @staticmethod
    def get_user_conversations(user_id: int, page: int = 1, page_size: int = 10) -> Dict:
        """
        分页获取用户的所有对话，按更新时间倒序排列
        返回 dict：包含 conversations 和 total 总数
        """
        offset = (page - 1) * page_size
        conn = get_db_connection()
        cursor = conn.cursor()

        # 获取总数
        cursor.execute("""
            SELECT COUNT(*) FROM conversations WHERE owned_user = ?
        """, (user_id,))
        total = cursor.fetchone()[0]

        # 获取分页数据
        cursor.execute("""
            SELECT id, title, updated_at, last_message_id, created_at
            FROM conversations
            WHERE owned_user = ?
            ORDER BY updated_at DESC
            LIMIT ? OFFSET ?
        """, (user_id, page_size, offset))
        conversations = cursor.fetchall()
        conn.close()

        return {
            "conversations": [dict(row) for row in conversations],
            "total": total
        }
```

### 文件: `backend/go-impl/README.md`

```markdown
# LingChat Backend Golang Implementation

LingChat 的后端服务Golang实现

## 环境要求

- Go 1.21 或更高版本
- Docker 和 Docker Compose（可选，用于容器化部署）

## 本地开发环境搭建

### 1. 安装 Go

如果你还没有安装 Go，请按照以下步骤安装：

#### macOS (使用 Homebrew)
```bash
brew install go
```

#### Linux
```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install golang-go

# CentOS/RHEL
sudo yum install golang
```

#### Windows

Windows请参考`go.dev`自行下载安装包，或使用`Winget`,`Scoop`等工具

### 2. 配置 Go 环境

确保设置以下环境变量（通常会自动配置，可通过`go env`检查）：

```bash
export GOPATH=$HOME/go
export PATH=$PATH:$GOPATH/bin
```

### 3. 克隆项目并安装依赖

```bash
# 进入项目目录
cd backend/go-impl

# 下载依赖
go mod download
```

### 4. 配置环境变量

复制 `.env.example` 文件（如果存在）或创建 `.env` 文件，并配置必要的环境变量：

```bash
cp .env.example .env  # 如果存在 .env.example
```

然后编辑 `.env` 文件，设置必要的配置项。

## 运行应用

### 本地运行

#### 方式一：直接运行源码
```bash
# 启动应用
go run cmd/app/main.go
```

#### 方式二：编译后运行可执行文件
```bash
# 编译应用（在当前目录生成可执行文件）
go build -o lingchat ./cmd/app

# 运行编译后的可执行文件
./lingchat
```

应用将在 `http://localhost:8765` 启动。

### 使用 Docker 运行

1. 构建 Docker 镜像：

```bash
docker build -t lingchat-backend .
```

2. 运行容器：

```bash
docker run -p 8765:8765 --env-file .env lingchat-backend
```

## 开发指南

### 项目结构

```
.
├── api/            # API 定义和接口
├── cmd/            # 主程序入口
├── internal/       # 内部实现
│   ├── clients/        # 各依赖服务的客户端实现
│   ├── config/         # 配置相关
│   └── service/        # 核心业务逻辑
├── go.mod          # Go 模块定义
├── go.sum          # 依赖校验
└── .env            # 环境配置
```

### 更新依赖

```bash
go mod tidy
```

## 测试

运行测试：

```bash
go test ./...
```

## 常见问题

1. 如果遇到端口被占用：
   - 检查是否有其他进程在使用 8765 端口
   - 可以在 `.env` 文件中修改端口配置

2. 如果遇到依赖问题：
   - 运行 `go mod tidy` 清理依赖
   - 确保所有依赖都正确安装
```

### 文件: `backend/predictor_server.py`

```python
import os
from contextlib import asynccontextmanager
from typing import List, Optional

import dotenv
import uvicorn
from fastapi import FastAPI, HTTPException
from core.predictor import EmotionClassifier
from pydantic import BaseModel


classifier = None  # 初始化分类器


@asynccontextmanager
async def lifespan(app: FastAPI):
    global classifier
    try:
        model_path = os.environ.get("EMOTION_MODEL_PATH", "./emotion_model_18emo")
        classifier = EmotionClassifier(model_path)
    except Exception as e:
        raise Exception(f"Failed to initialize classifier: {str(e)}")
    yield


app = FastAPI(
    title="Emotion Classification API",
    description="API for emotion classification using BERT model",
    version="1.0.0",
    lifespan=lifespan,
)


class PredictionRequest(BaseModel):
    text: str
    confidence_threshold: Optional[float] = 0.08


class EmotionResult(BaseModel):
    label: str
    probability: float


class PredictionResponse(BaseModel):
    label: str
    confidence: float
    top3: List[EmotionResult]
    warning: Optional[str] = None


@app.get("/health")
async def health_check():
    return {"status": "healthy"}


@app.post("/predict", response_model=PredictionResponse)
async def predict_emotion(request: PredictionRequest):
    if classifier is None:
        raise HTTPException(status_code=500, detail="Classifier not initialized")
    try:
        result = classifier.predict(request.text, request.confidence_threshold)
        return result
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


if __name__ == "__main__":
    dotenv.load_dotenv()
    host = os.environ.get("EMOTION_BIND_ADDR", "0.0.0.0")
    port = os.environ.get("EMOTION_PORT", 8000)

    uvicorn.run(
        "predictor_server:app", host=host, port=port, workers=1, log_level="info"
    )
```

### 文件: `backend/webChat.docker.py`

```python
import os
import json
import asyncio
import websockets
import glob
from core.deepseek import DeepSeek
import re
from core.predictor import EmotionClassifier  # 导入情绪分类器
from core.VitsTTS import VitsTTS              # 导入语音生成
from core.logger import Logger
from core.langDetect import LangDetect
import dotenv

dotenv.load_dotenv()
logger = Logger()
deepseek = DeepSeek()
emotion_classifier = EmotionClassifier()
langDetect = LangDetect()
tts_engine = VitsTTS(
    #enbale=False      #如果你没有配置simple-voice-api，请去掉这一行最开始的#号
)
temp_voice_dir = "./frontend/public/audio"
os.makedirs(temp_voice_dir, exist_ok=True)

# ANSI 颜色代码
COLOR_USER = "\033[92m"  # 绿色
COLOR_AI = "\033[96m"    # 青蓝色
COLOR_EMOTION = "\033[93m" # 黄色（用于情绪显示）
COLOR_RESET = "\033[0m"  # 重置颜色

def analyze_emotions(text):
    """分析文本中每个【】标记的情绪，并提取日语和中文部分"""
    # 改进后的正则表达式，更灵活地匹配各种情况
    emotion_segments = re.findall(r'(【(.*?)】)([^【】]*)', text)
    
    results = []
    for i, (full_tag, emotion_tag, following_text) in enumerate(emotion_segments, 1):
        # 统一处理括号（兼容中英文括号）
        following_text = following_text.replace('(', '（').replace(')', '）')
        
        # 提取日语部分（<...>），改进匹配模式
        japanese_match = re.search(r'<(.*?)>', following_text)
        japanese_text = japanese_match.group(1).strip() if japanese_match else ""
        
        # 提取动作部分（（...）），改进匹配模式
        motion_match = re.search(r'（(.*?)）', following_text)
        motion_text = motion_match.group(1).strip() if motion_match else ""
        
        # 清理后的文本（移除日语部分和动作部分）
        cleaned_text = re.sub(r'<.*?>|（.*?）', '', following_text).strip()
        
        # 清理日语文本中的动作部分
        if japanese_text:
            japanese_text = re.sub(r'（.*?）', '', japanese_text).strip()
        
        # 跳过完全空的文本
        if not any([following_text, japanese_text, motion_text]):
            continue
        
        # 改进语言检测和处理
        try:
            if japanese_text and cleaned_text:
                # 如果两者都有内容，才进行语言检测和交换
                lang_jp = langDetect.detect_language(japanese_text)
                lang_clean = langDetect.detect_language(cleaned_text)
                
                if (lang_jp in ['Chinese', 'Chinese_ABS'] and lang_clean in ['Japanese', 'Chinese']) and \
                    lang_clean != 'Chinese_ABS':
                        cleaned_text, japanese_text = japanese_text, cleaned_text


        except Exception as e:
            # 语言检测失败时保持原样
            print(f"Language detection error: {e}")
        
        # 对情绪标签单独预测，增加错误处理
        try:
            predicted = emotion_classifier.predict(emotion_tag)
            prediction_result = {
                "label": predicted["label"],
                "confidence": predicted["confidence"]
            }
        except Exception as e:
            print(f"Emotion prediction error for '{emotion_tag}': {e}")
            prediction_result = {
                "label": "unknown",
                "confidence": 0.0
            }
        
        results.append({
            "index": i,
            "original_tag": emotion_tag,
            "following_text": cleaned_text,
            "motion_text": motion_text,
            "japanese_text": japanese_text,
            "predicted": prediction_result["label"],
            "confidence": prediction_result["confidence"],
            "voice_file": os.path.join(temp_voice_dir, f"part_{i}.{tts_engine.format}")
        })
    
    return results

async def generate_voice_files(text_segments):
    """异步生成所有语音文件"""
    tasks = []
    for segment in text_segments:
        if segment["japanese_text"]:  # 确保有实际内容
            task = text_to_speech(
                segment["japanese_text"], 
                segment["voice_file"]
            )
            tasks.append(task)
    await asyncio.gather(*tasks)

def play_voice_files(text_segments):
    """顺序播放生成的语音文件"""
    for segment in text_segments:
        if os.path.exists(segment["voice_file"]):
            print(f"\n播放: 【{segment['original_tag']}】{segment['following_text']}")
            print(f"\n日文翻译: {segment['japanese_text']}")
            print(f"预测情绪: {segment['predicted']} (置信度: {segment['confidence']:.2%})")
            # 使用 pydub 播放音频，避免文件占用问题
            

# 语音合成（TTS）函数
async def text_to_speech(text, output_file):
    """生成单个语音文件"""
    await tts_engine.generate_voice(text, output_file, True)

def create_responses(emotion_segments, user_message):
    # 构造多条回复
    responses = []
    for segment in emotion_segments:
        # 提取文件名（去掉目录部分）
        audio_file = os.path.basename(segment['voice_file'])
        
        response = {
            "type": "reply",
            "emotion": segment['predicted'],
            "originalTag": segment['original_tag'],
            "message": segment['following_text'],
            "motionText": segment['motion_text'],
            "audioFile": audio_file,
            "originalMessage": user_message,
            "isMultiPart": True,  # 标记是多部分消息
            "partIndex": len(responses),  # 当前部分索引
            "totalParts": len(emotion_segments)  # 总部分数
        }
        responses.append(response)
    
    return responses

async def process_ai_response(ai_response, user_message):
    """处理AI回复并分析情绪"""
    # 0. 清理临时语音文件夹中的所有.wav文件
    if os.path.exists(temp_voice_dir):
        wav_files = glob.glob(os.path.join(temp_voice_dir, "*.wav"))
        for file in wav_files:
            try:
                os.remove(file)
            except Exception as e:
                print(f"删除文件 {file} 时出错: {e}")

    # 1. 打印原始回复
    print(f"\n{COLOR_AI}钦灵:{COLOR_RESET} {ai_response}")
    
    # 2. 分析情绪片段
    emotion_segments = analyze_emotions(ai_response)
    if not emotion_segments:
        print("未检测到有效情绪片段，请检查deepseek.py中的apikey是否正确填写")
        return
    
    # 3. 生成语音文件
    print("\n生成语音文件中...")
    await generate_voice_files(emotion_segments)

    # 4. 构造消息包
    responses = create_responses(emotion_segments, user_message)

    # 5. 播放并显示分析结果
    print("\n语音分析结果:")
    play_voice_files(emotion_segments)

    return responses
    
    

async def handle_client(websocket):
    print("Python 服务: 新的连接建立")
    
    try:
        async for message in websocket:
            data = json.loads(message)
            print(f"Python 收到消息: {data}")
            
            if data.get('type') == 'message':
                user_message = data.get('content', '')
                logger.log_conversation("用户", user_message)
                ai_response = deepseek.process_message(user_message)
                logger.log_conversation("钦灵", ai_response)
                
                try:
                    responses = await process_ai_response(ai_response, user_message)
                    if responses:  # 确保responses不是None
                        for response in responses:
                            await websocket.send(json.dumps(response))
                            await asyncio.sleep(0.1)
                except Exception as e:
                    print(f"\n处理AI响应时出错: {str(e)}")
                    await websocket.send(json.dumps({"error": str(e)}))
                
    except websockets.exceptions.ConnectionClosedOK:
        print("Python 服务: 连接正常关闭")
    except Exception as e:
        print(f"Python 服务: 发生错误 - {e}")

async def main():
    # 显式创建事件循环（兼容 Docker 和 Windows）
    print("main函数加载中")
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    
    # 修改主机地址为 0.0.0.0（允许 Docker 外部访问）
    # 修改 WebSocket 服务器配置
    bind_addr = os.environ.get("BACKEND_BIND_ADDR", "0.0.0.0")
    bind_port = os.environ.get("BACKEND_PORT", 8765)
    server = await websockets.serve(
        handle_client,
        bind_addr,
        bind_port,
        ping_interval=None,
        # 添加以下配置
        ping_timeout=None,
        close_timeout=None,
        max_size=2**25,  # 32MB
        # 放宽跨域限制
        origins=None  # 允许所有来源
    )
    
    print(f"Python WebSocket 服务运行在 ws://{bind_addr}:{bind_port}")
    await server.wait_closed()

if __name__ == "__main__":
    print("程序启动！")
    asyncio.run(main())
```

### 文件: `backend/windows_main.py`

```python
import os
import asyncio
import json
import uvicorn
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
from fastapi.middleware.cors import CORSMiddleware
from dotenv import load_dotenv
from core.service_manager import service_manager
from core.frontend_manager import FrontendManager
from core.logger import Logger
from api.chat_history import router as chat_history_router

load_dotenv()

# ============= 初始化核心组件 =============
logger = Logger()
app = FastAPI()
logo = [
    "", 
    "", 
    "█╗       ██╗ ███╗   ██╗  ██████╗      █████╗ ██╗  ██╗  █████╗  ████████╗",
    "██║      ██║ ████╗  ██║ ██╔════╝     ██╔═══╝ ██║  ██║ ██╔══██╗ ╚══██╔══╝",
    "██║      ██║ ██╔██╗ ██║ ██║  ███╗    ██║     ███████║ ███████║    ██║   ",
    "██║      ██║ ██║╚██╗██║ ██║   ██║    ██║     ██╔══██║ ██╔══██║    ██║   ",
    "███████╗ ██║ ██║ ╚████║ ╚██████╔╝     █████╗ ██║  ██║ ██║  ██║    ██║   ",
    "╚══════╝ ╚═╝ ╚═╝  ╚═══╝  ╚═════╝      ╚════╝ ╚═╝  ╚═╝ ╚═╝  ╚═╝    ╚═╝   "
    ]

# ============= 保留你的原始 WebSocket 处理逻辑 =============
async def handle_websocket_message(websocket, data):
    """完全复用你原有的消息处理逻辑"""
    if data.get('type') == 'message':
        # logger.client_message(data)
        responses = await service_manager.ai_service.process_message(data.get('content', ''))
        for response in responses:
            await websocket.send_json(response)

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    try:
        while True:
            message = await websocket.receive()
            # 首先检查是否是断开消息
            if message.get('type') == 'websocket.disconnect':
                logger.info(f"客户端断开连接，代码: {message.get('code')}")
            else:
                print(message)
                data = json.loads(message["text"])

                if data.get('type') == 'ping':
                    await websocket.send_json({"type": "pong"})
                elif data.get('type') == 'message':
                    responses = await service_manager.ai_service.process_message(data.get('content', ''))
                    for response in responses:
                        await websocket.send_json(response)
            
                    
    except WebSocketDisconnect:
        print("客户端断开连接")

# ============= 新增 HTTP 路由 =============
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000"],  # 你的前端地址
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.include_router(chat_history_router)

# ============= 保留前端服务 =============
frontend_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'frontend', 'public')

@app.get("/")
async def index():
    return FileResponse(os.path.join(frontend_dir, "pages", "index.html"))

@app.get("/about")
async def about():
    return FileResponse(os.path.join(frontend_dir, "pages", "about.html"))

# 静态文件服务（处理其他未匹配的请求）
app.mount("/", StaticFiles(directory=frontend_dir, html=True), name="static")

# ============= 启动逻辑 =============
async def main():
    for line in logo:
        logger.log_text(line)
    logger.log_text("\n")

    # 启动前端
    # frontend = FrontendManager(logger)
    # if not frontend.start_frontend(
    #     frontend_dir=frontend_dir,
    #     port=os.getenv('FRONTEND_PORT', '3000')
    # ):
    #     logger.error("前端启动失败")
    #     return

    # 启动 FastAPI（同时支持 HTTP 和 WebSocket）
    config = uvicorn.Config(
        app,
        host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
        port=int(os.getenv('BACKEND_PORT', '8765')),
        log_level="info"
    )
    server = uvicorn.Server(config)
    await server.serve()

if __name__ == "__main__":
    asyncio.run(main())
```

### 文件: `logs/log转json.py`

```python
import os
import json
from datetime import datetime
import glob

def parse_log_to_json(log_filepath):
    """
    解析单个.log文件，将其内容转换为JSON所需的聊天记录列表，并提取对话日期。

    Args:
        log_filepath (str): .log文件的路径。

    Returns:
        tuple: (datetime_object, list_of_chat_dicts)
               如果解析失败，则返回 (None, None)。
    """
    chat_records = []
    dialog_datetime = None

    try:
        with open(log_filepath, 'r', encoding='utf-8') as f:
            lines = [line.strip() for line in f.readlines() if line.strip()] # 读取并去除空行

        if not lines:
            print(f"警告: 文件 {log_filepath} 为空，跳过。")
            return None, None

        # 1. 解析对话日期
        first_line = lines[0]
        if first_line.startswith("对话日期:"):
            datetime_str = first_line.replace("对话日期:", "").strip()
            try:
                dialog_datetime = datetime.strptime(datetime_str, "%Y-%m-%d %H:%M:%S")
            except ValueError:
                print(f"错误: 文件 {log_filepath} 中的日期时间格式 '{datetime_str}' 不正确，应为 'YYYY-MM-DD HH:MM:SS'。跳过。")
                return None, None
        else:
            print(f"错误: 文件 {log_filepath} 第一行格式不正确，未找到 '对话日期:'。跳过。")
            return None, None

        # 2. 解析聊天内容
        # .log 文件格式特殊：AI的回复可能分布在多行，直到下一个 "用户:" 或文件末尾
        current_speaker = None
        current_content_parts = []

        # 从第二行开始处理对话内容
        for line in lines[1:]:
            if line.startswith("用户:"):
                # 如果之前有AI的内容，先保存
                if current_speaker == "assistant" and current_content_parts:
                    chat_records.append({
                        "role": "assistant",
                        "content": "\n".join(current_content_parts)
                    })
                    current_content_parts = []
                
                content = line.replace("用户:", "").strip()
                chat_records.append({"role": "user", "content": content})
                current_speaker = "user"

            # 假设AI的名字是 "钦灵"，可以根据实际情况修改或扩展
            elif line.startswith("钦灵:"):
                # 如果之前有AI的内容（理论上不应该，除非格式错误），也保存一下
                if current_speaker == "assistant" and current_content_parts:
                     chat_records.append({
                        "role": "assistant",
                        "content": "\n".join(current_content_parts)
                    })
                
                # 开始新的AI回复
                current_content_parts = [line.replace("钦灵:", "").strip()]
                current_speaker = "assistant"
            
            elif current_speaker == "assistant": # 如果当前是AI在说话，且行不以"用户:"或"钦灵:"开头，则认为是AI回复的延续
                current_content_parts.append(line)
            
            # else: # 忽略无法识别的行或将其视为错误
            #     print(f"警告: 在 {log_filepath} 中发现无法识别的行: '{line}'")


        # 处理循环结束后可能剩余的AI内容
        if current_speaker == "assistant" and current_content_parts:
            chat_records.append({
                "role": "assistant",
                "content": "\n".join(current_content_parts)
            })
        
        if not chat_records:
            print(f"警告: 文件 {log_filepath} 未能解析出任何聊天内容。")
            return dialog_datetime, [] # 返回日期和空列表，以便创建空json

        return dialog_datetime, chat_records

    except Exception as e:
        print(f"处理文件 {log_filepath} 时发生严重错误: {e}")
        return None, None

def main():
    # 获取当前脚本所在的目录作为根目录
    # 如果您想指定其他根目录，请修改下面这行
    root_directory = os.getcwd() 
    log_files_pattern = os.path.join(root_directory, "*.log")
    
    log_files = glob.glob(log_files_pattern)

    if not log_files:
        print(f"在目录 '{root_directory}' 下没有找到 .log 文件。")
        return

    print(f"找到以下 .log 文件: {log_files}")

    for log_filepath in log_files:
        print(f"\n正在处理文件: {log_filepath}")
        
        dialog_datetime, chat_data = parse_log_to_json(log_filepath)

        if dialog_datetime is None or chat_data is None:
            # parse_log_to_json内部已打印错误信息
            continue
        
        # 构建输出目录和文件名
        # YYYY年MM月\DD日\session_YYYYMMDD_HHMMSS.json
        year_str = str(dialog_datetime.year)
        month_str = dialog_datetime.strftime("%m") # 01, 02, ..., 12
        day_str = dialog_datetime.strftime("%d")   # 01, 02, ..., 31

        # 创建目录路径： YYYY年MM月
        month_folder_name = f"{year_str}年{month_str}月"
        # 创建完整的日文件夹路径： YYYY年MM月\DD日
        day_folder_name = f"{day_str}日"
        
        # 最终输出目录
        output_directory = os.path.join(root_directory, month_folder_name, day_folder_name)
        
        try:
            os.makedirs(output_directory, exist_ok=True) # exist_ok=True 避免目录已存在时报错
        except OSError as e:
            print(f"创建目录 {output_directory} 失败: {e}")
            continue

        # 构建文件名: session_YYYYMMDD_HHMMSS.json
        timestamp_filename_part = dialog_datetime.strftime("%Y%m%d_%H%M%S")
        output_filename = f"session_{timestamp_filename_part}.json"
        output_filepath = os.path.join(output_directory, output_filename)

        # 写入JSON文件
        try:
            with open(output_filepath, 'w', encoding='utf-8') as json_file:
                json.dump(chat_data, json_file, ensure_ascii=False, indent=4)
            print(f"成功转换并保存到: {output_filepath}")
        except IOError as e:
            print(f"写入JSON文件 {output_filepath} 失败: {e}")
        except Exception as e:
            print(f"在写入 {output_filepath} 时发生未知错误: {e}")


if __name__ == "__main__":
    main()
```

### 文件: `node_modules/body-parser/README.md`

```markdown
# body-parser

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Build Status][ci-image]][ci-url]
[![Test Coverage][coveralls-image]][coveralls-url]
[![OpenSSF Scorecard Badge][ossf-scorecard-badge]][ossf-scorecard-visualizer]

Node.js body parsing middleware.

Parse incoming request bodies in a middleware before your handlers, available
under the `req.body` property.

**Note** As `req.body`'s shape is based on user-controlled input, all
properties and values in this object are untrusted and should be validated
before trusting. For example, `req.body.foo.toString()` may fail in multiple
ways, for example the `foo` property may not be there or may not be a string,
and `toString` may not be a function and instead a string or other user input.

[Learn about the anatomy of an HTTP transaction in Node.js](https://nodejs.org/en/docs/guides/anatomy-of-an-http-transaction/).

_This does not handle multipart bodies_, due to their complex and typically
large nature. For multipart bodies, you may be interested in the following
modules:

  * [busboy](https://www.npmjs.org/package/busboy#readme) and
    [connect-busboy](https://www.npmjs.org/package/connect-busboy#readme)
  * [multiparty](https://www.npmjs.org/package/multiparty#readme) and
    [connect-multiparty](https://www.npmjs.org/package/connect-multiparty#readme)
  * [formidable](https://www.npmjs.org/package/formidable#readme)
  * [multer](https://www.npmjs.org/package/multer#readme)

This module provides the following parsers:

  * [JSON body parser](#bodyparserjsonoptions)
  * [Raw body parser](#bodyparserrawoptions)
  * [Text body parser](#bodyparsertextoptions)
  * [URL-encoded form body parser](#bodyparserurlencodedoptions)

Other body parsers you might be interested in:

- [body](https://www.npmjs.org/package/body#readme)
- [co-body](https://www.npmjs.org/package/co-body#readme)

## Installation

```sh
$ npm install body-parser
```

## API

```js
const bodyParser = require('body-parser')
```

The `bodyParser` object exposes various factories to create middlewares. All
middlewares will populate the `req.body` property with the parsed body when
the `Content-Type` request header matches the `type` option.

The various errors returned by this module are described in the
[errors section](#errors).

### bodyParser.json([options])

Returns middleware that only parses `json` and only looks at requests where
the `Content-Type` header matches the `type` option. This parser accepts any
Unicode encoding of the body and supports automatic inflation of `gzip`,
`br` (brotli) and `deflate` encodings.

A new `body` object containing the parsed data is populated on the `request`
object after the middleware (i.e. `req.body`).

#### Options

The `json` function takes an optional `options` object that may contain any of
the following keys:

##### inflate

When set to `true`, then deflated (compressed) bodies will be inflated; when
`false`, deflated bodies are rejected. Defaults to `true`.

##### limit

Controls the maximum request body size. If this is a number, then the value
specifies the number of bytes; if it is a string, the value is passed to the
[bytes](https://www.npmjs.com/package/bytes) library for parsing. Defaults
to `'100kb'`.

##### reviver

The `reviver` option is passed directly to `JSON.parse` as the second
argument. You can find more information on this argument
[in the MDN documentation about JSON.parse](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/JSON/parse#Example.3A_Using_the_reviver_parameter).

##### strict

When set to `true`, will only accept arrays and objects; when `false` will
accept anything `JSON.parse` accepts. Defaults to `true`.

##### type

The `type` option is used to determine what media type the middleware will
parse. This option can be a string, array of strings, or a function. If not a
function, `type` option is passed directly to the
[type-is](https://www.npmjs.org/package/type-is#readme) library and this can
be an extension name (like `json`), a mime type (like `application/json`), or
a mime type with a wildcard (like `*/*` or `*/json`). If a function, the `type`
option is called as `fn(req)` and the request is parsed if it returns a truthy
value. Defaults to `application/json`.

##### verify

The `verify` option, if supplied, is called as `verify(req, res, buf, encoding)`,
where `buf` is a `Buffer` of the raw request body and `encoding` is the
encoding of the request. The parsing can be aborted by throwing an error.

### bodyParser.raw([options])

Returns middleware that parses all bodies as a `Buffer` and only looks at
requests where the `Content-Type` header matches the `type` option. This
parser supports automatic inflation of `gzip`, `br` (brotli) and `deflate`
encodings.

A new `body` object containing the parsed data is populated on the `request`
object after the middleware (i.e. `req.body`). This will be a `Buffer` object
of the body.

#### Options

The `raw` function takes an optional `options` object that may contain any of
the following keys:

##### inflate

When set to `true`, then deflated (compressed) bodies will be inflated; when
`false`, deflated bodies are rejected. Defaults to `true`.

##### limit

Controls the maximum request body size. If this is a number, then the value
specifies the number of bytes; if it is a string, the value is passed to the
[bytes](https://www.npmjs.com/package/bytes) library for parsing. Defaults
to `'100kb'`.

##### type

The `type` option is used to determine what media type the middleware will
parse. This option can be a string, array of strings, or a function.
If not a function, `type` option is passed directly to the
[type-is](https://www.npmjs.org/package/type-is#readme) library and this
can be an extension name (like `bin`), a mime type (like
`application/octet-stream`), or a mime type with a wildcard (like `*/*` or
`application/*`). If a function, the `type` option is called as `fn(req)`
and the request is parsed if it returns a truthy value. Defaults to
`application/octet-stream`.

##### verify

The `verify` option, if supplied, is called as `verify(req, res, buf, encoding)`,
where `buf` is a `Buffer` of the raw request body and `encoding` is the
encoding of the request. The parsing can be aborted by throwing an error.

### bodyParser.text([options])

Returns middleware that parses all bodies as a string and only looks at
requests where the `Content-Type` header matches the `type` option. This
parser supports automatic inflation of `gzip`, `br` (brotli) and `deflate`
encodings.

A new `body` string containing the parsed data is populated on the `request`
object after the middleware (i.e. `req.body`). This will be a string of the
body.

#### Options

The `text` function takes an optional `options` object that may contain any of
the following keys:

##### defaultCharset

Specify the default character set for the text content if the charset is not
specified in the `Content-Type` header of the request. Defaults to `utf-8`.

##### inflate

When set to `true`, then deflated (compressed) bodies will be inflated; when
`false`, deflated bodies are rejected. Defaults to `true`.

##### limit

Controls the maximum request body size. If this is a number, then the value
specifies the number of bytes; if it is a string, the value is passed to the
[bytes](https://www.npmjs.com/package/bytes) library for parsing. Defaults
to `'100kb'`.

##### type

The `type` option is used to determine what media type the middleware will
parse. This option can be a string, array of strings, or a function. If not
a function, `type` option is passed directly to the
[type-is](https://www.npmjs.org/package/type-is#readme) library and this can
be an extension name (like `txt`), a mime type (like `text/plain`), or a mime
type with a wildcard (like `*/*` or `text/*`). If a function, the `type`
option is called as `fn(req)` and the request is parsed if it returns a
truthy value. Defaults to `text/plain`.

##### verify

The `verify` option, if supplied, is called as `verify(req, res, buf, encoding)`,
where `buf` is a `Buffer` of the raw request body and `encoding` is the
encoding of the request. The parsing can be aborted by throwing an error.

### bodyParser.urlencoded([options])

Returns middleware that only parses `urlencoded` bodies and only looks at
requests where the `Content-Type` header matches the `type` option. This
parser accepts only UTF-8 encoding of the body and supports automatic
inflation of `gzip`, `br` (brotli) and `deflate` encodings.

A new `body` object containing the parsed data is populated on the `request`
object after the middleware (i.e. `req.body`). This object will contain
key-value pairs, where the value can be a string or array (when `extended` is
`false`), or any type (when `extended` is `true`).

#### Options

The `urlencoded` function takes an optional `options` object that may contain
any of the following keys:

##### extended

The "extended" syntax allows for rich objects and arrays to be encoded into the
URL-encoded format, allowing for a JSON-like experience with URL-encoded. For
more information, please [see the qs
library](https://www.npmjs.org/package/qs#readme).

Defaults to `false`.

##### inflate

When set to `true`, then deflated (compressed) bodies will be inflated; when
`false`, deflated bodies are rejected. Defaults to `true`.

##### limit

Controls the maximum request body size. If this is a number, then the value
specifies the number of bytes; if it is a string, the value is passed to the
[bytes](https://www.npmjs.com/package/bytes) library for parsing. Defaults
to `'100kb'`.

##### parameterLimit

The `parameterLimit` option controls the maximum number of parameters that
are allowed in the URL-encoded data. If a request contains more parameters
than this value, a 413 will be returned to the client. Defaults to `1000`.

##### type

The `type` option is used to determine what media type the middleware will
parse. This option can be a string, array of strings, or a function. If not
a function, `type` option is passed directly to the
[type-is](https://www.npmjs.org/package/type-is#readme) library and this can
be an extension name (like `urlencoded`), a mime type (like
`application/x-www-form-urlencoded`), or a mime type with a wildcard (like
`*/x-www-form-urlencoded`). If a function, the `type` option is called as
`fn(req)` and the request is parsed if it returns a truthy value. Defaults
to `application/x-www-form-urlencoded`.

##### verify

The `verify` option, if supplied, is called as `verify(req, res, buf, encoding)`,
where `buf` is a `Buffer` of the raw request body and `encoding` is the
encoding of the request. The parsing can be aborted by throwing an error.

##### defaultCharset

The default charset to parse as, if not specified in content-type. Must be
either `utf-8` or `iso-8859-1`. Defaults to `utf-8`.

##### charsetSentinel

Whether to let the value of the `utf8` parameter take precedence as the charset
selector. It requires the form to contain a parameter named `utf8` with a value
of `✓`. Defaults to `false`.

##### interpretNumericEntities

Whether to decode numeric entities such as `&#9786;` when parsing an iso-8859-1
form. Defaults to `false`.


#### depth

The `depth` option is used to configure the maximum depth of the `qs` library when `extended` is `true`. This allows you to limit the amount of keys that are parsed and can be useful to prevent certain types of abuse. Defaults to `32`. It is recommended to keep this value as low as possible.

## Errors

The middlewares provided by this module create errors using the
[`http-errors` module](https://www.npmjs.com/package/http-errors). The errors
will typically have a `status`/`statusCode` property that contains the suggested
HTTP response code, an `expose` property to determine if the `message` property
should be displayed to the client, a `type` property to determine the type of
error without matching against the `message`, and a `body` property containing
the read body, if available.

The following are the common errors created, though any error can come through
for various reasons.

### content encoding unsupported

This error will occur when the request had a `Content-Encoding` header that
contained an encoding but the "inflation" option was set to `false`. The
`status` property is set to `415`, the `type` property is set to
`'encoding.unsupported'`, and the `charset` property will be set to the
encoding that is unsupported.

### entity parse failed

This error will occur when the request contained an entity that could not be
parsed by the middleware. The `status` property is set to `400`, the `type`
property is set to `'entity.parse.failed'`, and the `body` property is set to
the entity value that failed parsing.

### entity verify failed

This error will occur when the request contained an entity that could not be
failed verification by the defined `verify` option. The `status` property is
set to `403`, the `type` property is set to `'entity.verify.failed'`, and the
`body` property is set to the entity value that failed verification.

### request aborted

This error will occur when the request is aborted by the client before reading
the body has finished. The `received` property will be set to the number of
bytes received before the request was aborted and the `expected` property is
set to the number of expected bytes. The `status` property is set to `400`
and `type` property is set to `'request.aborted'`.

### request entity too large

This error will occur when the request body's size is larger than the "limit"
option. The `limit` property will be set to the byte limit and the `length`
property will be set to the request body's length. The `status` property is
set to `413` and the `type` property is set to `'entity.too.large'`.

### request size did not match content length

This error will occur when the request's length did not match the length from
the `Content-Length` header. This typically occurs when the request is malformed,
typically when the `Content-Length` header was calculated based on characters
instead of bytes. The `status` property is set to `400` and the `type` property
is set to `'request.size.invalid'`.

### stream encoding should not be set

This error will occur when something called the `req.setEncoding` method prior
to this middleware. This module operates directly on bytes only and you cannot
call `req.setEncoding` when using this module. The `status` property is set to
`500` and the `type` property is set to `'stream.encoding.set'`.

### stream is not readable

This error will occur when the request is no longer readable when this middleware
attempts to read it. This typically means something other than a middleware from
this module read the request body already and the middleware was also configured to
read the same request. The `status` property is set to `500` and the `type`
property is set to `'stream.not.readable'`.

### too many parameters

This error will occur when the content of the request exceeds the configured
`parameterLimit` for the `urlencoded` parser. The `status` property is set to
`413` and the `type` property is set to `'parameters.too.many'`.

### unsupported charset "BOGUS"

This error will occur when the request had a charset parameter in the
`Content-Type` header, but the `iconv-lite` module does not support it OR the
parser does not support it. The charset is contained in the message as well
as in the `charset` property. The `status` property is set to `415`, the
`type` property is set to `'charset.unsupported'`, and the `charset` property
is set to the charset that is unsupported.

### unsupported content encoding "bogus"

This error will occur when the request had a `Content-Encoding` header that
contained an unsupported encoding. The encoding is contained in the message
as well as in the `encoding` property. The `status` property is set to `415`,
the `type` property is set to `'encoding.unsupported'`, and the `encoding`
property is set to the encoding that is unsupported.

### The input exceeded the depth

This error occurs when using `bodyParser.urlencoded` with the `extended` property set to `true` and the input exceeds the configured `depth` option. The `status` property is set to `400`. It is recommended to review the `depth` option and evaluate if it requires a higher value. When the `depth` option is set to `32` (default value), the error will not be thrown.

## Examples

### Express/Connect top-level generic

This example demonstrates adding a generic JSON and URL-encoded parser as a
top-level middleware, which will parse the bodies of all incoming requests.
This is the simplest setup.

```js
const express = require('express')
const bodyParser = require('body-parser')

const app = express()

// parse application/x-www-form-urlencoded
app.use(bodyParser.urlencoded())

// parse application/json
app.use(bodyParser.json())

app.use(function (req, res) {
  res.setHeader('Content-Type', 'text/plain')
  res.write('you posted:\n')
  res.end(String(JSON.stringify(req.body, null, 2)))
})
```

### Express route-specific

This example demonstrates adding body parsers specifically to the routes that
need them. In general, this is the most recommended way to use body-parser with
Express.

```js
const express = require('express')
const bodyParser = require('body-parser')

const app = express()

// create application/json parser
const jsonParser = bodyParser.json()

// create application/x-www-form-urlencoded parser
const urlencodedParser = bodyParser.urlencoded()

// POST /login gets urlencoded bodies
app.post('/login', urlencodedParser, function (req, res) {
  if (!req.body || !req.body.username) res.sendStatus(400)
  res.send('welcome, ' + req.body.username)
})

// POST /api/users gets JSON bodies
app.post('/api/users', jsonParser, function (req, res) {
  if (!req.body) res.sendStatus(400)
  // create user in req.body
})
```

### Change accepted type for parsers

All the parsers accept a `type` option which allows you to change the
`Content-Type` that the middleware will parse.

```js
const express = require('express')
const bodyParser = require('body-parser')

const app = express()

// parse various different custom JSON types as JSON
app.use(bodyParser.json({ type: 'application/*+json' }))

// parse some custom thing into a Buffer
app.use(bodyParser.raw({ type: 'application/vnd.custom-type' }))

// parse an HTML body into a string
app.use(bodyParser.text({ type: 'text/html' }))
```

## License

[MIT](LICENSE)

[ci-image]: https://badgen.net/github/checks/expressjs/body-parser/master?label=ci
[ci-url]: https://github.com/expressjs/body-parser/actions/workflows/ci.yml
[coveralls-image]: https://badgen.net/coveralls/c/github/expressjs/body-parser/master
[coveralls-url]: https://coveralls.io/r/expressjs/body-parser?branch=master
[node-version-image]: https://badgen.net/npm/node/body-parser
[node-version-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/body-parser
[npm-url]: https://npmjs.org/package/body-parser
[npm-version-image]: https://badgen.net/npm/v/body-parser
[ossf-scorecard-badge]: https://api.scorecard.dev/projects/github.com/expressjs/body-parser/badge
[ossf-scorecard-visualizer]: https://ossf.github.io/scorecard-visualizer/#/projects/github.com/expressjs/body-parser
```

### 文件: `node_modules/bytes/Readme.md`

```markdown
# Bytes utility

[![NPM Version][npm-image]][npm-url]
[![NPM Downloads][downloads-image]][downloads-url]
[![Build Status][ci-image]][ci-url]
[![Test Coverage][coveralls-image]][coveralls-url]

Utility to parse a string bytes (ex: `1TB`) to bytes (`1099511627776`) and vice-versa.

## Installation

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```bash
$ npm install bytes
```

## Usage

```js
var bytes = require('bytes');
```

#### bytes(number｜string value, [options]): number｜string｜null

Default export function. Delegates to either `bytes.format` or `bytes.parse` based on the type of `value`.

**Arguments**

| Name    | Type     | Description        |
|---------|----------|--------------------|
| value   | `number`｜`string` | Number value to format or string value to parse |
| options | `Object` | Conversion options for `format` |

**Returns**

| Name    | Type             | Description                                     |
|---------|------------------|-------------------------------------------------|
| results | `string`｜`number`｜`null` | Return null upon error. Numeric value in bytes, or string value otherwise. |

**Example**

```js
bytes(1024);
// output: '1KB'

bytes('1KB');
// output: 1024
```

#### bytes.format(number value, [options]): string｜null

Format the given value in bytes into a string. If the value is negative, it is kept as such. If it is a float, it is
 rounded.

**Arguments**

| Name    | Type     | Description        |
|---------|----------|--------------------|
| value   | `number` | Value in bytes     |
| options | `Object` | Conversion options |

**Options**

| Property          | Type   | Description                                                                             |
|-------------------|--------|-----------------------------------------------------------------------------------------|
| decimalPlaces | `number`｜`null` | Maximum number of decimal places to include in output. Default value to `2`. |
| fixedDecimals | `boolean`｜`null` | Whether to always display the maximum number of decimal places. Default value to `false` |
| thousandsSeparator | `string`｜`null` | Example of values: `' '`, `','` and `'.'`... Default value to `''`. |
| unit | `string`｜`null` | The unit in which the result will be returned (B/KB/MB/GB/TB). Default value to `''` (which means auto detect). |
| unitSeparator | `string`｜`null` | Separator to use between number and unit. Default value to `''`. |

**Returns**

| Name    | Type             | Description                                     |
|---------|------------------|-------------------------------------------------|
| results | `string`｜`null` | Return null upon error. String value otherwise. |

**Example**

```js
bytes.format(1024);
// output: '1KB'

bytes.format(1000);
// output: '1000B'

bytes.format(1000, {thousandsSeparator: ' '});
// output: '1 000B'

bytes.format(1024 * 1.7, {decimalPlaces: 0});
// output: '2KB'

bytes.format(1024, {unitSeparator: ' '});
// output: '1 KB'
```

#### bytes.parse(string｜number value): number｜null

Parse the string value into an integer in bytes. If no unit is given, or `value`
is a number, it is assumed the value is in bytes.

Supported units and abbreviations are as follows and are case-insensitive:

  * `b` for bytes
  * `kb` for kilobytes
  * `mb` for megabytes
  * `gb` for gigabytes
  * `tb` for terabytes
  * `pb` for petabytes

The units are in powers of two, not ten. This means 1kb = 1024b according to this parser.

**Arguments**

| Name          | Type   | Description        |
|---------------|--------|--------------------|
| value   | `string`｜`number` | String to parse, or number in bytes.   |

**Returns**

| Name    | Type        | Description             |
|---------|-------------|-------------------------|
| results | `number`｜`null` | Return null upon error. Value in bytes otherwise. |

**Example**

```js
bytes.parse('1KB');
// output: 1024

bytes.parse('1024');
// output: 1024

bytes.parse(1024);
// output: 1024
```

## License

[MIT](LICENSE)

[ci-image]: https://badgen.net/github/checks/visionmedia/bytes.js/master?label=ci
[ci-url]: https://github.com/visionmedia/bytes.js/actions?query=workflow%3Aci
[coveralls-image]: https://badgen.net/coveralls/c/github/visionmedia/bytes.js/master
[coveralls-url]: https://coveralls.io/r/visionmedia/bytes.js?branch=master
[downloads-image]: https://badgen.net/npm/dm/bytes
[downloads-url]: https://npmjs.org/package/bytes
[npm-image]: https://badgen.net/npm/v/bytes
[npm-url]: https://npmjs.org/package/bytes
```

### 文件: `node_modules/call-bind-apply-helpers/README.md`

```markdown
# call-bind-apply-helpers <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![dependency status][deps-svg]][deps-url]
[![dev dependency status][dev-deps-svg]][dev-deps-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Helper functions around Function call/apply/bind, for use in `call-bind`.

The only packages that should likely ever use this package directly are `call-bind` and `get-intrinsic`.
Please use `call-bind` unless you have a very good reason not to.

## Getting started

```sh
npm install --save call-bind-apply-helpers
```

## Usage/Examples

```js
const assert = require('assert');
const callBindBasic = require('call-bind-apply-helpers');

function f(a, b) {
	assert.equal(this, 1);
	assert.equal(a, 2);
	assert.equal(b, 3);
	assert.equal(arguments.length, 2);
}

const fBound = callBindBasic([f, 1]);

delete Function.prototype.call;
delete Function.prototype.bind;

fBound(2, 3);
```

## Tests

Clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/call-bind-apply-helpers
[npm-version-svg]: https://versionbadg.es/ljharb/call-bind-apply-helpers.svg
[deps-svg]: https://david-dm.org/ljharb/call-bind-apply-helpers.svg
[deps-url]: https://david-dm.org/ljharb/call-bind-apply-helpers
[dev-deps-svg]: https://david-dm.org/ljharb/call-bind-apply-helpers/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/call-bind-apply-helpers#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/call-bind-apply-helpers.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/call-bind-apply-helpers.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/call-bind-apply-helpers.svg
[downloads-url]: https://npm-stat.com/charts.html?package=call-bind-apply-helpers
[codecov-image]: https://codecov.io/gh/ljharb/call-bind-apply-helpers/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/call-bind-apply-helpers/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/call-bind-apply-helpers
[actions-url]: https://github.com/ljharb/call-bind-apply-helpers/actions
```

### 文件: `node_modules/call-bound/README.md`

```markdown
# call-bound <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![dependency status][deps-svg]][deps-url]
[![dev dependency status][dev-deps-svg]][dev-deps-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Robust call-bound JavaScript intrinsics, using `call-bind` and `get-intrinsic`.

## Getting started

```sh
npm install --save call-bound
```

## Usage/Examples

```js
const assert = require('assert');
const callBound = require('call-bound');

const slice = callBound('Array.prototype.slice');

delete Function.prototype.call;
delete Function.prototype.bind;
delete Array.prototype.slice;

assert.deepEqual(slice([1, 2, 3, 4], 1, -1), [2, 3]);
```

## Tests

Clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/call-bound
[npm-version-svg]: https://versionbadg.es/ljharb/call-bound.svg
[deps-svg]: https://david-dm.org/ljharb/call-bound.svg
[deps-url]: https://david-dm.org/ljharb/call-bound
[dev-deps-svg]: https://david-dm.org/ljharb/call-bound/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/call-bound#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/call-bound.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/call-bound.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/call-bound.svg
[downloads-url]: https://npm-stat.com/charts.html?package=call-bound
[codecov-image]: https://codecov.io/gh/ljharb/call-bound/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/call-bound/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/call-bound
[actions-url]: https://github.com/ljharb/call-bound/actions
```

### 文件: `node_modules/content-type/README.md`

```markdown
# content-type

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Node.js Version][node-image]][node-url]
[![Build Status][ci-image]][ci-url]
[![Coverage Status][coveralls-image]][coveralls-url]

Create and parse HTTP Content-Type header according to RFC 7231

## Installation

```sh
$ npm install content-type
```

## API

```js
var contentType = require('content-type')
```

### contentType.parse(string)

```js
var obj = contentType.parse('image/svg+xml; charset=utf-8')
```

Parse a `Content-Type` header. This will return an object with the following
properties (examples are shown for the string `'image/svg+xml; charset=utf-8'`):

 - `type`: The media type (the type and subtype, always lower case).
   Example: `'image/svg+xml'`

 - `parameters`: An object of the parameters in the media type (name of parameter
   always lower case). Example: `{charset: 'utf-8'}`

Throws a `TypeError` if the string is missing or invalid.

### contentType.parse(req)

```js
var obj = contentType.parse(req)
```

Parse the `Content-Type` header from the given `req`. Short-cut for
`contentType.parse(req.headers['content-type'])`.

Throws a `TypeError` if the `Content-Type` header is missing or invalid.

### contentType.parse(res)

```js
var obj = contentType.parse(res)
```

Parse the `Content-Type` header set on the given `res`. Short-cut for
`contentType.parse(res.getHeader('content-type'))`.

Throws a `TypeError` if the `Content-Type` header is missing or invalid.

### contentType.format(obj)

```js
var str = contentType.format({
  type: 'image/svg+xml',
  parameters: { charset: 'utf-8' }
})
```

Format an object into a `Content-Type` header. This will return a string of the
content type for the given object with the following properties (examples are
shown that produce the string `'image/svg+xml; charset=utf-8'`):

 - `type`: The media type (will be lower-cased). Example: `'image/svg+xml'`

 - `parameters`: An object of the parameters in the media type (name of the
   parameter will be lower-cased). Example: `{charset: 'utf-8'}`

Throws a `TypeError` if the object contains an invalid type or parameter names.

## License

[MIT](LICENSE)

[ci-image]: https://badgen.net/github/checks/jshttp/content-type/master?label=ci
[ci-url]: https://github.com/jshttp/content-type/actions/workflows/ci.yml
[coveralls-image]: https://badgen.net/coveralls/c/github/jshttp/content-type/master
[coveralls-url]: https://coveralls.io/r/jshttp/content-type?branch=master
[node-image]: https://badgen.net/npm/node/content-type
[node-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/content-type
[npm-url]: https://npmjs.org/package/content-type
[npm-version-image]: https://badgen.net/npm/v/content-type
```

### 文件: `node_modules/debug/README.md`

```markdown
# debug
[![OpenCollective](https://opencollective.com/debug/backers/badge.svg)](#backers)
[![OpenCollective](https://opencollective.com/debug/sponsors/badge.svg)](#sponsors)

<img width="647" src="https://user-images.githubusercontent.com/71256/29091486-fa38524c-7c37-11e7-895f-e7ec8e1039b6.png">

A tiny JavaScript debugging utility modelled after Node.js core's debugging
technique. Works in Node.js and web browsers.

## Installation

```bash
$ npm install debug
```

## Usage

`debug` exposes a function; simply pass this function the name of your module, and it will return a decorated version of `console.error` for you to pass debug statements to. This will allow you to toggle the debug output for different parts of your module as well as the module as a whole.

Example [_app.js_](./examples/node/app.js):

```js
var debug = require('debug')('http')
  , http = require('http')
  , name = 'My App';

// fake app

debug('booting %o', name);

http.createServer(function(req, res){
  debug(req.method + ' ' + req.url);
  res.end('hello\n');
}).listen(3000, function(){
  debug('listening');
});

// fake worker of some kind

require('./worker');
```

Example [_worker.js_](./examples/node/worker.js):

```js
var a = require('debug')('worker:a')
  , b = require('debug')('worker:b');

function work() {
  a('doing lots of uninteresting work');
  setTimeout(work, Math.random() * 1000);
}

work();

function workb() {
  b('doing some work');
  setTimeout(workb, Math.random() * 2000);
}

workb();
```

The `DEBUG` environment variable is then used to enable these based on space or
comma-delimited names.

Here are some examples:

<img width="647" alt="screen shot 2017-08-08 at 12 53 04 pm" src="https://user-images.githubusercontent.com/71256/29091703-a6302cdc-7c38-11e7-8304-7c0b3bc600cd.png">
<img width="647" alt="screen shot 2017-08-08 at 12 53 38 pm" src="https://user-images.githubusercontent.com/71256/29091700-a62a6888-7c38-11e7-800b-db911291ca2b.png">
<img width="647" alt="screen shot 2017-08-08 at 12 53 25 pm" src="https://user-images.githubusercontent.com/71256/29091701-a62ea114-7c38-11e7-826a-2692bedca740.png">

#### Windows command prompt notes

##### CMD

On Windows the environment variable is set using the `set` command.

```cmd
set DEBUG=*,-not_this
```

Example:

```cmd
set DEBUG=* & node app.js
```

##### PowerShell (VS Code default)

PowerShell uses different syntax to set environment variables.

```cmd
$env:DEBUG = "*,-not_this"
```

Example:

```cmd
$env:DEBUG='app';node app.js
```

Then, run the program to be debugged as usual.

npm script example:
```js
  "windowsDebug": "@powershell -Command $env:DEBUG='*';node app.js",
```

## Namespace Colors

Every debug instance has a color generated for it based on its namespace name.
This helps when visually parsing the debug output to identify which debug instance
a debug line belongs to.

#### Node.js

In Node.js, colors are enabled when stderr is a TTY. You also _should_ install
the [`supports-color`](https://npmjs.org/supports-color) module alongside debug,
otherwise debug will only use a small handful of basic colors.

<img width="521" src="https://user-images.githubusercontent.com/71256/29092181-47f6a9e6-7c3a-11e7-9a14-1928d8a711cd.png">

#### Web Browser

Colors are also enabled on "Web Inspectors" that understand the `%c` formatting
option. These are WebKit web inspectors, Firefox ([since version
31](https://hacks.mozilla.org/2014/05/editable-box-model-multiple-selection-sublime-text-keys-much-more-firefox-developer-tools-episode-31/))
and the Firebug plugin for Firefox (any version).

<img width="524" src="https://user-images.githubusercontent.com/71256/29092033-b65f9f2e-7c39-11e7-8e32-f6f0d8e865c1.png">


## Millisecond diff

When actively developing an application it can be useful to see when the time spent between one `debug()` call and the next. Suppose for example you invoke `debug()` before requesting a resource, and after as well, the "+NNNms" will show you how much time was spent between calls.

<img width="647" src="https://user-images.githubusercontent.com/71256/29091486-fa38524c-7c37-11e7-895f-e7ec8e1039b6.png">

When stdout is not a TTY, `Date#toISOString()` is used, making it more useful for logging the debug information as shown below:

<img width="647" src="https://user-images.githubusercontent.com/71256/29091956-6bd78372-7c39-11e7-8c55-c948396d6edd.png">


## Conventions

If you're using this in one or more of your libraries, you _should_ use the name of your library so that developers may toggle debugging as desired without guessing names. If you have more than one debuggers you _should_ prefix them with your library name and use ":" to separate features. For example "bodyParser" from Connect would then be "connect:bodyParser".  If you append a "*" to the end of your name, it will always be enabled regardless of the setting of the DEBUG environment variable.  You can then use it for normal output as well as debug output.

## Wildcards

The `*` character may be used as a wildcard. Suppose for example your library has
debuggers named "connect:bodyParser", "connect:compress", "connect:session",
instead of listing all three with
`DEBUG=connect:bodyParser,connect:compress,connect:session`, you may simply do
`DEBUG=connect:*`, or to run everything using this module simply use `DEBUG=*`.

You can also exclude specific debuggers by prefixing them with a "-" character.
For example, `DEBUG=*,-connect:*` would include all debuggers except those
starting with "connect:".

## Environment Variables

When running through Node.js, you can set a few environment variables that will
change the behavior of the debug logging:

| Name      | Purpose                                         |
|-----------|-------------------------------------------------|
| `DEBUG`   | Enables/disables specific debugging namespaces. |
| `DEBUG_HIDE_DATE` | Hide date from debug output (non-TTY).  |
| `DEBUG_COLORS`| Whether or not to use colors in the debug output. |
| `DEBUG_DEPTH` | Object inspection depth.                    |
| `DEBUG_SHOW_HIDDEN` | Shows hidden properties on inspected objects. |


__Note:__ The environment variables beginning with `DEBUG_` end up being
converted into an Options object that gets used with `%o`/`%O` formatters.
See the Node.js documentation for
[`util.inspect()`](https://nodejs.org/api/util.html#util_util_inspect_object_options)
for the complete list.

## Formatters

Debug uses [printf-style](https://wikipedia.org/wiki/Printf_format_string) formatting.
Below are the officially supported formatters:

| Formatter | Representation |
|-----------|----------------|
| `%O`      | Pretty-print an Object on multiple lines. |
| `%o`      | Pretty-print an Object all on a single line. |
| `%s`      | String. |
| `%d`      | Number (both integer and float). |
| `%j`      | JSON. Replaced with the string '[Circular]' if the argument contains circular references. |
| `%%`      | Single percent sign ('%'). This does not consume an argument. |


### Custom formatters

You can add custom formatters by extending the `debug.formatters` object.
For example, if you wanted to add support for rendering a Buffer as hex with
`%h`, you could do something like:

```js
const createDebug = require('debug')
createDebug.formatters.h = (v) => {
  return v.toString('hex')
}

// …elsewhere
const debug = createDebug('foo')
debug('this is hex: %h', new Buffer('hello world'))
//   foo this is hex: 68656c6c6f20776f726c6421 +0ms
```


## Browser Support

You can build a browser-ready script using [browserify](https://github.com/substack/node-browserify),
or just use the [browserify-as-a-service](https://wzrd.in/) [build](https://wzrd.in/standalone/debug@latest),
if you don't want to build it yourself.

Debug's enable state is currently persisted by `localStorage`.
Consider the situation shown below where you have `worker:a` and `worker:b`,
and wish to debug both. You can enable this using `localStorage.debug`:

```js
localStorage.debug = 'worker:*'
```

And then refresh the page.

```js
a = debug('worker:a');
b = debug('worker:b');

setInterval(function(){
  a('doing some work');
}, 1000);

setInterval(function(){
  b('doing some work');
}, 1200);
```

In Chromium-based web browsers (e.g. Brave, Chrome, and Electron), the JavaScript console will—by default—only show messages logged by `debug` if the "Verbose" log level is _enabled_.

<img width="647" src="https://user-images.githubusercontent.com/7143133/152083257-29034707-c42c-4959-8add-3cee850e6fcf.png">

## Output streams

  By default `debug` will log to stderr, however this can be configured per-namespace by overriding the `log` method:

Example [_stdout.js_](./examples/node/stdout.js):

```js
var debug = require('debug');
var error = debug('app:error');

// by default stderr is used
error('goes to stderr!');

var log = debug('app:log');
// set this namespace to log via console.log
log.log = console.log.bind(console); // don't forget to bind to console!
log('goes to stdout');
error('still goes to stderr!');

// set all output to go via console.info
// overrides all per-namespace log settings
debug.log = console.info.bind(console);
error('now goes to stdout via console.info');
log('still goes to stdout, but via console.info now');
```

## Extend
You can simply extend debugger 
```js
const log = require('debug')('auth');

//creates new debug instance with extended namespace
const logSign = log.extend('sign');
const logLogin = log.extend('login');

log('hello'); // auth hello
logSign('hello'); //auth:sign hello
logLogin('hello'); //auth:login hello
```

## Set dynamically

You can also enable debug dynamically by calling the `enable()` method :

```js
let debug = require('debug');

console.log(1, debug.enabled('test'));

debug.enable('test');
console.log(2, debug.enabled('test'));

debug.disable();
console.log(3, debug.enabled('test'));

```

print :   
```
1 false
2 true
3 false
```

Usage :  
`enable(namespaces)`  
`namespaces` can include modes separated by a colon and wildcards.
   
Note that calling `enable()` completely overrides previously set DEBUG variable : 

```
$ DEBUG=foo node -e 'var dbg = require("debug"); dbg.enable("bar"); console.log(dbg.enabled("foo"))'
=> false
```

`disable()`

Will disable all namespaces. The functions returns the namespaces currently
enabled (and skipped). This can be useful if you want to disable debugging
temporarily without knowing what was enabled to begin with.

For example:

```js
let debug = require('debug');
debug.enable('foo:*,-foo:bar');
let namespaces = debug.disable();
debug.enable(namespaces);
```

Note: There is no guarantee that the string will be identical to the initial
enable string, but semantically they will be identical.

## Checking whether a debug target is enabled

After you've created a debug instance, you can determine whether or not it is
enabled by checking the `enabled` property:

```javascript
const debug = require('debug')('http');

if (debug.enabled) {
  // do stuff...
}
```

You can also manually toggle this property to force the debug instance to be
enabled or disabled.

## Usage in child processes

Due to the way `debug` detects if the output is a TTY or not, colors are not shown in child processes when `stderr` is piped. A solution is to pass the `DEBUG_COLORS=1` environment variable to the child process.  
For example:

```javascript
worker = fork(WORKER_WRAP_PATH, [workerPath], {
  stdio: [
    /* stdin: */ 0,
    /* stdout: */ 'pipe',
    /* stderr: */ 'pipe',
    'ipc',
  ],
  env: Object.assign({}, process.env, {
    DEBUG_COLORS: 1 // without this settings, colors won't be shown
  }),
});

worker.stderr.pipe(process.stderr, { end: false });
```


## Authors

 - TJ Holowaychuk
 - Nathan Rajlich
 - Andrew Rhyne
 - Josh Junon

## Backers

Support us with a monthly donation and help us continue our activities. [[Become a backer](https://opencollective.com/debug#backer)]

<a href="https://opencollective.com/debug/backer/0/website" target="_blank"><img src="https://opencollective.com/debug/backer/0/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/1/website" target="_blank"><img src="https://opencollective.com/debug/backer/1/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/2/website" target="_blank"><img src="https://opencollective.com/debug/backer/2/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/3/website" target="_blank"><img src="https://opencollective.com/debug/backer/3/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/4/website" target="_blank"><img src="https://opencollective.com/debug/backer/4/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/5/website" target="_blank"><img src="https://opencollective.com/debug/backer/5/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/6/website" target="_blank"><img src="https://opencollective.com/debug/backer/6/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/7/website" target="_blank"><img src="https://opencollective.com/debug/backer/7/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/8/website" target="_blank"><img src="https://opencollective.com/debug/backer/8/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/9/website" target="_blank"><img src="https://opencollective.com/debug/backer/9/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/10/website" target="_blank"><img src="https://opencollective.com/debug/backer/10/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/11/website" target="_blank"><img src="https://opencollective.com/debug/backer/11/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/12/website" target="_blank"><img src="https://opencollective.com/debug/backer/12/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/13/website" target="_blank"><img src="https://opencollective.com/debug/backer/13/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/14/website" target="_blank"><img src="https://opencollective.com/debug/backer/14/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/15/website" target="_blank"><img src="https://opencollective.com/debug/backer/15/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/16/website" target="_blank"><img src="https://opencollective.com/debug/backer/16/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/17/website" target="_blank"><img src="https://opencollective.com/debug/backer/17/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/18/website" target="_blank"><img src="https://opencollective.com/debug/backer/18/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/19/website" target="_blank"><img src="https://opencollective.com/debug/backer/19/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/20/website" target="_blank"><img src="https://opencollective.com/debug/backer/20/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/21/website" target="_blank"><img src="https://opencollective.com/debug/backer/21/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/22/website" target="_blank"><img src="https://opencollective.com/debug/backer/22/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/23/website" target="_blank"><img src="https://opencollective.com/debug/backer/23/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/24/website" target="_blank"><img src="https://opencollective.com/debug/backer/24/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/25/website" target="_blank"><img src="https://opencollective.com/debug/backer/25/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/26/website" target="_blank"><img src="https://opencollective.com/debug/backer/26/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/27/website" target="_blank"><img src="https://opencollective.com/debug/backer/27/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/28/website" target="_blank"><img src="https://opencollective.com/debug/backer/28/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/29/website" target="_blank"><img src="https://opencollective.com/debug/backer/29/avatar.svg"></a>


## Sponsors

Become a sponsor and get your logo on our README on Github with a link to your site. [[Become a sponsor](https://opencollective.com/debug#sponsor)]

<a href="https://opencollective.com/debug/sponsor/0/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/0/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/1/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/1/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/2/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/2/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/3/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/3/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/4/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/4/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/5/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/5/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/6/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/6/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/7/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/7/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/8/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/8/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/9/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/9/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/10/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/10/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/11/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/11/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/12/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/12/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/13/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/13/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/14/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/14/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/15/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/15/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/16/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/16/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/17/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/17/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/18/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/18/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/19/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/19/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/20/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/20/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/21/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/21/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/22/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/22/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/23/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/23/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/24/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/24/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/25/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/25/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/26/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/26/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/27/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/27/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/28/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/28/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/29/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/29/avatar.svg"></a>

## License

(The MIT License)

Copyright (c) 2014-2017 TJ Holowaychuk &lt;tj@vision-media.ca&gt;
Copyright (c) 2018-2021 Josh Junon

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the
'Software'), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to
the following conditions:

The above copyright notice and this permission notice shall be
included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

### 文件: `node_modules/depd/Readme.md`

```markdown
# depd

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Node.js Version][node-image]][node-url]
[![Linux Build][travis-image]][travis-url]
[![Windows Build][appveyor-image]][appveyor-url]
[![Coverage Status][coveralls-image]][coveralls-url]

Deprecate all the things

> With great modules comes great responsibility; mark things deprecated!

## Install

This module is installed directly using `npm`:

```sh
$ npm install depd
```

This module can also be bundled with systems like
[Browserify](http://browserify.org/) or [webpack](https://webpack.github.io/),
though by default this module will alter it's API to no longer display or
track deprecations.

## API

<!-- eslint-disable no-unused-vars -->

```js
var deprecate = require('depd')('my-module')
```

This library allows you to display deprecation messages to your users.
This library goes above and beyond with deprecation warnings by
introspection of the call stack (but only the bits that it is interested
in).

Instead of just warning on the first invocation of a deprecated
function and never again, this module will warn on the first invocation
of a deprecated function per unique call site, making it ideal to alert
users of all deprecated uses across the code base, rather than just
whatever happens to execute first.

The deprecation warnings from this module also include the file and line
information for the call into the module that the deprecated function was
in.

**NOTE** this library has a similar interface to the `debug` module, and
this module uses the calling file to get the boundary for the call stacks,
so you should always create a new `deprecate` object in each file and not
within some central file.

### depd(namespace)

Create a new deprecate function that uses the given namespace name in the
messages and will display the call site prior to the stack entering the
file this function was called from. It is highly suggested you use the
name of your module as the namespace.

### deprecate(message)

Call this function from deprecated code to display a deprecation message.
This message will appear once per unique caller site. Caller site is the
first call site in the stack in a different file from the caller of this
function.

If the message is omitted, a message is generated for you based on the site
of the `deprecate()` call and will display the name of the function called,
similar to the name displayed in a stack trace.

### deprecate.function(fn, message)

Call this function to wrap a given function in a deprecation message on any
call to the function. An optional message can be supplied to provide a custom
message.

### deprecate.property(obj, prop, message)

Call this function to wrap a given property on object in a deprecation message
on any accessing or setting of the property. An optional message can be supplied
to provide a custom message.

The method must be called on the object where the property belongs (not
inherited from the prototype).

If the property is a data descriptor, it will be converted to an accessor
descriptor in order to display the deprecation message.

### process.on('deprecation', fn)

This module will allow easy capturing of deprecation errors by emitting the
errors as the type "deprecation" on the global `process`. If there are no
listeners for this type, the errors are written to STDERR as normal, but if
there are any listeners, nothing will be written to STDERR and instead only
emitted. From there, you can write the errors in a different format or to a
logging source.

The error represents the deprecation and is emitted only once with the same
rules as writing to STDERR. The error has the following properties:

  - `message` - This is the message given by the library
  - `name` - This is always `'DeprecationError'`
  - `namespace` - This is the namespace the deprecation came from
  - `stack` - This is the stack of the call to the deprecated thing

Example `error.stack` output:

```
DeprecationError: my-cool-module deprecated oldfunction
    at Object.<anonymous> ([eval]-wrapper:6:22)
    at Module._compile (module.js:456:26)
    at evalScript (node.js:532:25)
    at startup (node.js:80:7)
    at node.js:902:3
```

### process.env.NO_DEPRECATION

As a user of modules that are deprecated, the environment variable `NO_DEPRECATION`
is provided as a quick solution to silencing deprecation warnings from being
output. The format of this is similar to that of `DEBUG`:

```sh
$ NO_DEPRECATION=my-module,othermod node app.js
```

This will suppress deprecations from being output for "my-module" and "othermod".
The value is a list of comma-separated namespaces. To suppress every warning
across all namespaces, use the value `*` for a namespace.

Providing the argument `--no-deprecation` to the `node` executable will suppress
all deprecations (only available in Node.js 0.8 or higher).

**NOTE** This will not suppress the deperecations given to any "deprecation"
event listeners, just the output to STDERR.

### process.env.TRACE_DEPRECATION

As a user of modules that are deprecated, the environment variable `TRACE_DEPRECATION`
is provided as a solution to getting more detailed location information in deprecation
warnings by including the entire stack trace. The format of this is the same as
`NO_DEPRECATION`:

```sh
$ TRACE_DEPRECATION=my-module,othermod node app.js
```

This will include stack traces for deprecations being output for "my-module" and
"othermod". The value is a list of comma-separated namespaces. To trace every
warning across all namespaces, use the value `*` for a namespace.

Providing the argument `--trace-deprecation` to the `node` executable will trace
all deprecations (only available in Node.js 0.8 or higher).

**NOTE** This will not trace the deperecations silenced by `NO_DEPRECATION`.

## Display

![message](files/message.png)

When a user calls a function in your library that you mark deprecated, they
will see the following written to STDERR (in the given colors, similar colors
and layout to the `debug` module):

```
bright cyan    bright yellow
|              |          reset       cyan
|              |          |           |
▼              ▼          ▼           ▼
my-cool-module deprecated oldfunction [eval]-wrapper:6:22
▲              ▲          ▲           ▲
|              |          |           |
namespace      |          |           location of mycoolmod.oldfunction() call
               |          deprecation message
               the word "deprecated"
```

If the user redirects their STDERR to a file or somewhere that does not support
colors, they see (similar layout to the `debug` module):

```
Sun, 15 Jun 2014 05:21:37 GMT my-cool-module deprecated oldfunction at [eval]-wrapper:6:22
▲                             ▲              ▲          ▲              ▲
|                             |              |          |              |
timestamp of message          namespace      |          |             location of mycoolmod.oldfunction() call
                                             |          deprecation message
                                             the word "deprecated"
```

## Examples

### Deprecating all calls to a function

This will display a deprecated message about "oldfunction" being deprecated
from "my-module" on STDERR.

```js
var deprecate = require('depd')('my-cool-module')

// message automatically derived from function name
// Object.oldfunction
exports.oldfunction = deprecate.function(function oldfunction () {
  // all calls to function are deprecated
})

// specific message
exports.oldfunction = deprecate.function(function () {
  // all calls to function are deprecated
}, 'oldfunction')
```

### Conditionally deprecating a function call

This will display a deprecated message about "weirdfunction" being deprecated
from "my-module" on STDERR when called with less than 2 arguments.

```js
var deprecate = require('depd')('my-cool-module')

exports.weirdfunction = function () {
  if (arguments.length < 2) {
    // calls with 0 or 1 args are deprecated
    deprecate('weirdfunction args < 2')
  }
}
```

When calling `deprecate` as a function, the warning is counted per call site
within your own module, so you can display different deprecations depending
on different situations and the users will still get all the warnings:

```js
var deprecate = require('depd')('my-cool-module')

exports.weirdfunction = function () {
  if (arguments.length < 2) {
    // calls with 0 or 1 args are deprecated
    deprecate('weirdfunction args < 2')
  } else if (typeof arguments[0] !== 'string') {
    // calls with non-string first argument are deprecated
    deprecate('weirdfunction non-string first arg')
  }
}
```

### Deprecating property access

This will display a deprecated message about "oldprop" being deprecated
from "my-module" on STDERR when accessed. A deprecation will be displayed
when setting the value and when getting the value.

```js
var deprecate = require('depd')('my-cool-module')

exports.oldprop = 'something'

// message automatically derives from property name
deprecate.property(exports, 'oldprop')

// explicit message
deprecate.property(exports, 'oldprop', 'oldprop >= 0.10')
```

## License

[MIT](LICENSE)

[appveyor-image]: https://badgen.net/appveyor/ci/dougwilson/nodejs-depd/master?label=windows
[appveyor-url]: https://ci.appveyor.com/project/dougwilson/nodejs-depd
[coveralls-image]: https://badgen.net/coveralls/c/github/dougwilson/nodejs-depd/master
[coveralls-url]: https://coveralls.io/r/dougwilson/nodejs-depd?branch=master
[node-image]: https://badgen.net/npm/node/depd
[node-url]: https://nodejs.org/en/download/
[npm-downloads-image]: https://badgen.net/npm/dm/depd
[npm-url]: https://npmjs.org/package/depd
[npm-version-image]: https://badgen.net/npm/v/depd
[travis-image]: https://badgen.net/travis/dougwilson/nodejs-depd/master?label=linux
[travis-url]: https://travis-ci.org/dougwilson/nodejs-depd
```

### 文件: `node_modules/dotenv/README.md`

```markdown
<div align="center">
🎉 announcing <a href="https://github.com/dotenvx/dotenvx">dotenvx</a>. <em>run anywhere, multi-environment, encrypted envs</em>.
</div>

&nbsp;

<div align="center">

**Special thanks to [our sponsors](https://github.com/sponsors/motdotla)**

<br>
<a href="https://www.warp.dev/?utm_source=github&utm_medium=referral&utm_campaign=dotenv_p_20220831">
  <div>
    <img src="https://res.cloudinary.com/dotenv-org/image/upload/v1661980709/warp_hi8oqj.png" width="230" alt="Warp">
  </div>
  <b>Warp is a blazingly fast, Rust-based terminal reimagined to work like a modern app.</b>
  <div>
    <sup>Get more done in the CLI with real text editing, block-based output, and AI command search.</sup>
  </div>
</a>
<br>

<a href="https://graphite.dev/?utm_source=github&utm_medium=repo&utm_campaign=dotenv"><img src="https://res.cloudinary.com/dotenv-org/image/upload/v1744035073/graphite_lgsrl8.gif" width="240" alt="Graphite" /></a>

<a href="https://graphite.dev/?utm_source=github&utm_medium=repo&utm_campaign=dotenv">
  <b>Graphite is the AI developer productivity platform helping teams on GitHub ship higher quality software, faster.</b>
</a>
<hr>
</div>

# dotenv [![NPM version](https://img.shields.io/npm/v/dotenv.svg?style=flat-square)](https://www.npmjs.com/package/dotenv)

<img src="https://raw.githubusercontent.com/motdotla/dotenv/master/dotenv.svg" alt="dotenv" align="right" width="200" />

Dotenv is a zero-dependency module that loads environment variables from a `.env` file into [`process.env`](https://nodejs.org/docs/latest/api/process.html#process_process_env). Storing configuration in the environment separate from code is based on [The Twelve-Factor App](https://12factor.net/config) methodology.

[![js-standard-style](https://img.shields.io/badge/code%20style-standard-brightgreen.svg?style=flat-square)](https://github.com/feross/standard)
[![LICENSE](https://img.shields.io/github/license/motdotla/dotenv.svg)](LICENSE)
[![codecov](https://codecov.io/gh/motdotla/dotenv-expand/graph/badge.svg?token=pawWEyaMfg)](https://codecov.io/gh/motdotla/dotenv-expand)

* [🌱 Install](#-install)
* [🏗️ Usage (.env)](#%EF%B8%8F-usage)
* [🌴 Multiple Environments 🆕](#-manage-multiple-environments)
* [🚀 Deploying (encryption) 🆕](#-deploying)
* [📚 Examples](#-examples)
* [📖 Docs](#-documentation)
* [❓ FAQ](#-faq)
* [⏱️ Changelog](./CHANGELOG.md)

## 🌱 Install

```bash
npm install dotenv --save
```

You can also use an npm-compatible package manager like yarn or bun:

```bash
yarn add dotenv
# or
bun add dotenv
```

## 🏗️ Usage

<a href="https://www.youtube.com/watch?v=YtkZR0NFd1g">
<div align="right">
<img src="https://img.youtube.com/vi/YtkZR0NFd1g/hqdefault.jpg" alt="how to use dotenv video tutorial" align="right" width="330" />
<img src="https://simpleicons.vercel.app/youtube/ff0000" alt="youtube/@dotenvorg" align="right" width="24" />
</div>
</a>

Create a `.env` file in the root of your project (if using a monorepo structure like `apps/backend/app.js`, put it in the root of the folder where your `app.js` process runs):

```dosini
S3_BUCKET="YOURS3BUCKET"
SECRET_KEY="YOURSECRETKEYGOESHERE"
```

As early as possible in your application, import and configure dotenv:

```javascript
require('dotenv').config()
console.log(process.env) // remove this after you've confirmed it is working
```

.. [or using ES6?](#how-do-i-use-dotenv-with-import)

```javascript
import 'dotenv/config'
```

That's it. `process.env` now has the keys and values you defined in your `.env` file:

```javascript
require('dotenv').config()
// or import 'dotenv/config' if you're using ES6

...

s3.getBucketCors({Bucket: process.env.S3_BUCKET}, function(err, data) {})
```

### Multiline values

If you need multiline variables, for example private keys, those are now supported (`>= v15.0.0`) with line breaks:

```dosini
PRIVATE_KEY="-----BEGIN RSA PRIVATE KEY-----
...
Kh9NV...
...
-----END RSA PRIVATE KEY-----"
```

Alternatively, you can double quote strings and use the `\n` character:

```dosini
PRIVATE_KEY="-----BEGIN RSA PRIVATE KEY-----\nKh9NV...\n-----END RSA PRIVATE KEY-----\n"
```

### Comments

Comments may be added to your file on their own line or inline:

```dosini
# This is a comment
SECRET_KEY=YOURSECRETKEYGOESHERE # comment
SECRET_HASH="something-with-a-#-hash"
```

Comments begin where a `#` exists, so if your value contains a `#` please wrap it in quotes. This is a breaking change from `>= v15.0.0` and on.

### Parsing

The engine which parses the contents of your file containing environment variables is available to use. It accepts a String or Buffer and will return an Object with the parsed keys and values.

```javascript
const dotenv = require('dotenv')
const buf = Buffer.from('BASIC=basic')
const config = dotenv.parse(buf) // will return an object
console.log(typeof config, config) // object { BASIC : 'basic' }
```

### Preload

> Note: Consider using [`dotenvx`](https://github.com/dotenvx/dotenvx) instead of preloading. I am now doing (and recommending) so.
>
> It serves the same purpose (you do not need to require and load dotenv), adds better debugging, and works with ANY language, framework, or platform. – [motdotla](https://github.com/motdotla)

You can use the `--require` (`-r`) [command line option](https://nodejs.org/api/cli.html#-r---require-module) to preload dotenv. By doing this, you do not need to require and load dotenv in your application code.

```bash
$ node -r dotenv/config your_script.js
```

The configuration options below are supported as command line arguments in the format `dotenv_config_<option>=value`

```bash
$ node -r dotenv/config your_script.js dotenv_config_path=/custom/path/to/.env dotenv_config_debug=true
```

Additionally, you can use environment variables to set configuration options. Command line arguments will precede these.

```bash
$ DOTENV_CONFIG_<OPTION>=value node -r dotenv/config your_script.js
```

```bash
$ DOTENV_CONFIG_ENCODING=latin1 DOTENV_CONFIG_DEBUG=true node -r dotenv/config your_script.js dotenv_config_path=/custom/path/to/.env
```

### Variable Expansion

You need to add the value of another variable in one of your variables? Use [dotenv-expand](https://github.com/motdotla/dotenv-expand).

### Command Substitution

Use [dotenvx](https://github.com/dotenvx/dotenvx) to use command substitution.

Add the output of a command to one of your variables in your .env file.

```ini
# .env
DATABASE_URL="postgres://$(whoami)@localhost/my_database"
```
```js
// index.js
console.log('DATABASE_URL', process.env.DATABASE_URL)
```
```sh
$ dotenvx run --debug -- node index.js
[dotenvx@0.14.1] injecting env (1) from .env
DATABASE_URL postgres://yourusername@localhost/my_database
```

### Syncing

You need to keep `.env` files in sync between machines, environments, or team members? Use [dotenvx](https://github.com/dotenvx/dotenvx) to encrypt your `.env` files and safely include them in source control. This still subscribes to the twelve-factor app rules by generating a decryption key separate from code.

### Multiple Environments

Use [dotenvx](https://github.com/dotenvx/dotenvx) to generate `.env.ci`, `.env.production` files, and more.

### Deploying

You need to deploy your secrets in a cloud-agnostic manner? Use [dotenvx](https://github.com/dotenvx/dotenvx) to generate a private decryption key that is set on your production server.

## 🌴 Manage Multiple Environments

Use [dotenvx](https://github.com/dotenvx/dotenvx)

Run any environment locally. Create a `.env.ENVIRONMENT` file and use `--env-file` to load it. It's straightforward, yet flexible.

```bash
$ echo "HELLO=production" > .env.production
$ echo "console.log('Hello ' + process.env.HELLO)" > index.js

$ dotenvx run --env-file=.env.production -- node index.js
Hello production
> ^^
```

or with multiple .env files

```bash
$ echo "HELLO=local" > .env.local
$ echo "HELLO=World" > .env
$ echo "console.log('Hello ' + process.env.HELLO)" > index.js

$ dotenvx run --env-file=.env.local --env-file=.env -- node index.js
Hello local
```

[more environment examples](https://dotenvx.com/docs/quickstart/environments)

## 🚀 Deploying

Use [dotenvx](https://github.com/dotenvx/dotenvx).

Add encryption to your `.env` files with a single command. Pass the `--encrypt` flag.

```
$ dotenvx set HELLO Production --encrypt -f .env.production
$ echo "console.log('Hello ' + process.env.HELLO)" > index.js

$ DOTENV_PRIVATE_KEY_PRODUCTION="<.env.production private key>" dotenvx run -- node index.js
[dotenvx] injecting env (2) from .env.production
Hello Production
```

[learn more](https://github.com/dotenvx/dotenvx?tab=readme-ov-file#encryption)

## 📚 Examples

See [examples](https://github.com/dotenv-org/examples) of using dotenv with various frameworks, languages, and configurations.

* [nodejs](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-nodejs)
* [nodejs (debug on)](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-nodejs-debug)
* [nodejs (override on)](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-nodejs-override)
* [nodejs (processEnv override)](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-custom-target)
* [esm](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-esm)
* [esm (preload)](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-esm-preload)
* [typescript](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-typescript)
* [typescript parse](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-typescript-parse)
* [typescript config](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-typescript-config)
* [webpack](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-webpack)
* [webpack (plugin)](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-webpack2)
* [react](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-react)
* [react (typescript)](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-react-typescript)
* [express](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-express)
* [nestjs](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-nestjs)
* [fastify](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-fastify)

## 📖 Documentation

Dotenv exposes four functions:

* `config`
* `parse`
* `populate`
* `decrypt`

### Config

`config` will read your `.env` file, parse the contents, assign it to
[`process.env`](https://nodejs.org/docs/latest/api/process.html#process_process_env),
and return an Object with a `parsed` key containing the loaded content or an `error` key if it failed.

```js
const result = dotenv.config()

if (result.error) {
  throw result.error
}

console.log(result.parsed)
```

You can additionally, pass options to `config`.

#### Options

##### path

Default: `path.resolve(process.cwd(), '.env')`

Specify a custom path if your file containing environment variables is located elsewhere.

```js
require('dotenv').config({ path: '/custom/path/to/.env' })
```

By default, `config` will look for a file called .env in the current working directory.

Pass in multiple files as an array, and they will be parsed in order and combined with `process.env` (or `option.processEnv`, if set). The first value set for a variable will win, unless the `options.override` flag is set, in which case the last value set will win.  If a value already exists in `process.env` and the `options.override` flag is NOT set, no changes will be made to that value. 

```js  
require('dotenv').config({ path: ['.env.local', '.env'] })
```

##### encoding

Default: `utf8`

Specify the encoding of your file containing environment variables.

```js
require('dotenv').config({ encoding: 'latin1' })
```

##### debug

Default: `false`

Turn on logging to help debug why certain keys or values are not being set as you expect.

```js
require('dotenv').config({ debug: process.env.DEBUG })
```

##### override

Default: `false`

Override any environment variables that have already been set on your machine with values from your .env file(s). If multiple files have been provided in `option.path` the override will also be used as each file is combined with the next. Without `override` being set, the first value wins. With `override` set the last value wins. 

```js
require('dotenv').config({ override: true })
```

##### processEnv

Default: `process.env`

Specify an object to write your environment variables to. Defaults to `process.env` environment variables.

```js
const myObject = {}
require('dotenv').config({ processEnv: myObject })

console.log(myObject) // values from .env
console.log(process.env) // this was not changed or written to
```

### Parse

The engine which parses the contents of your file containing environment
variables is available to use. It accepts a String or Buffer and will return
an Object with the parsed keys and values.

```js
const dotenv = require('dotenv')
const buf = Buffer.from('BASIC=basic')
const config = dotenv.parse(buf) // will return an object
console.log(typeof config, config) // object { BASIC : 'basic' }
```

#### Options

##### debug

Default: `false`

Turn on logging to help debug why certain keys or values are not being set as you expect.

```js
const dotenv = require('dotenv')
const buf = Buffer.from('hello world')
const opt = { debug: true }
const config = dotenv.parse(buf, opt)
// expect a debug message because the buffer is not in KEY=VAL form
```

### Populate

The engine which populates the contents of your .env file to `process.env` is available for use. It accepts a target, a source, and options. This is useful for power users who want to supply their own objects.

For example, customizing the source:

```js
const dotenv = require('dotenv')
const parsed = { HELLO: 'world' }

dotenv.populate(process.env, parsed)

console.log(process.env.HELLO) // world
```

For example, customizing the source AND target:

```js
const dotenv = require('dotenv')
const parsed = { HELLO: 'universe' }
const target = { HELLO: 'world' } // empty object

dotenv.populate(target, parsed, { override: true, debug: true })

console.log(target) // { HELLO: 'universe' }
```

#### options

##### Debug

Default: `false`

Turn on logging to help debug why certain keys or values are not being populated as you expect.

##### override

Default: `false`

Override any environment variables that have already been set.

## ❓ FAQ

### Why is the `.env` file not loading my environment variables successfully?

Most likely your `.env` file is not in the correct place. [See this stack overflow](https://stackoverflow.com/questions/42335016/dotenv-file-is-not-loading-environment-variables).

Turn on debug mode and try again..

```js
require('dotenv').config({ debug: true })
```

You will receive a helpful error outputted to your console.

### Should I commit my `.env` file?

No. We **strongly** recommend against committing your `.env` file to version
control. It should only include environment-specific values such as database
passwords or API keys. Your production database should have a different
password than your development database.

### Should I have multiple `.env` files?

We recommend creating one `.env` file per environment. Use `.env` for local/development, `.env.production` for production and so on. This still follows the twelve factor principles as each is attributed individually to its own environment. Avoid custom set ups that work in inheritance somehow (`.env.production` inherits values form `.env` for example). It is better to duplicate values if necessary across each `.env.environment` file.

> In a twelve-factor app, env vars are granular controls, each fully orthogonal to other env vars. They are never grouped together as “environments”, but instead are independently managed for each deploy. This is a model that scales up smoothly as the app naturally expands into more deploys over its lifetime.
>
> – [The Twelve-Factor App](http://12factor.net/config)

### What rules does the parsing engine follow?

The parsing engine currently supports the following rules:

- `BASIC=basic` becomes `{BASIC: 'basic'}`
- empty lines are skipped
- lines beginning with `#` are treated as comments
- `#` marks the beginning of a comment (unless when the value is wrapped in quotes)
- empty values become empty strings (`EMPTY=` becomes `{EMPTY: ''}`)
- inner quotes are maintained (think JSON) (`JSON={"foo": "bar"}` becomes `{JSON:"{\"foo\": \"bar\"}"`)
- whitespace is removed from both ends of unquoted values (see more on [`trim`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String/Trim)) (`FOO=  some value  ` becomes `{FOO: 'some value'}`)
- single and double quoted values are escaped (`SINGLE_QUOTE='quoted'` becomes `{SINGLE_QUOTE: "quoted"}`)
- single and double quoted values maintain whitespace from both ends (`FOO="  some value  "` becomes `{FOO: '  some value  '}`)
- double quoted values expand new lines (`MULTILINE="new\nline"` becomes

```
{MULTILINE: 'new
line'}
```

- backticks are supported (`` BACKTICK_KEY=`This has 'single' and "double" quotes inside of it.` ``)

### What happens to environment variables that were already set?

By default, we will never modify any environment variables that have already been set. In particular, if there is a variable in your `.env` file which collides with one that already exists in your environment, then that variable will be skipped.

If instead, you want to override `process.env` use the `override` option.

```javascript
require('dotenv').config({ override: true })
```

### How come my environment variables are not showing up for React?

Your React code is run in Webpack, where the `fs` module or even the `process` global itself are not accessible out-of-the-box. `process.env` can only be injected through Webpack configuration.

If you are using [`react-scripts`](https://www.npmjs.com/package/react-scripts), which is distributed through [`create-react-app`](https://create-react-app.dev/), it has dotenv built in but with a quirk. Preface your environment variables with `REACT_APP_`. See [this stack overflow](https://stackoverflow.com/questions/42182577/is-it-possible-to-use-dotenv-in-a-react-project) for more details.

If you are using other frameworks (e.g. Next.js, Gatsby...), you need to consult their documentation for how to inject environment variables into the client.

### Can I customize/write plugins for dotenv?

Yes! `dotenv.config()` returns an object representing the parsed `.env` file. This gives you everything you need to continue setting values on `process.env`. For example:

```js
const dotenv = require('dotenv')
const variableExpansion = require('dotenv-expand')
const myEnv = dotenv.config()
variableExpansion(myEnv)
```

### How do I use dotenv with `import`?

Simply..

```javascript
// index.mjs (ESM)
import 'dotenv/config' // see https://github.com/motdotla/dotenv#how-do-i-use-dotenv-with-import
import express from 'express'
```

A little background..

> When you run a module containing an `import` declaration, the modules it imports are loaded first, then each module body is executed in a depth-first traversal of the dependency graph, avoiding cycles by skipping anything already executed.
>
> – [ES6 In Depth: Modules](https://hacks.mozilla.org/2015/08/es6-in-depth-modules/)

What does this mean in plain language? It means you would think the following would work but it won't.

`errorReporter.mjs`:
```js
class Client {
  constructor (apiKey) {
    console.log('apiKey', apiKey)

    this.apiKey = apiKey
  }
}

export default new Client(process.env.API_KEY)
```
`index.mjs`:
```js
// Note: this is INCORRECT and will not work
import * as dotenv from 'dotenv'
dotenv.config()

import errorReporter from './errorReporter.mjs' // process.env.API_KEY will be blank!
```

`process.env.API_KEY` will be blank.

Instead, `index.mjs` should be written as..

```js
import 'dotenv/config'

import errorReporter from './errorReporter.mjs'
```

Does that make sense? It's a bit unintuitive, but it is how importing of ES6 modules work. Here is a [working example of this pitfall](https://github.com/dotenv-org/examples/tree/master/usage/dotenv-es6-import-pitfall).

There are two alternatives to this approach:

1. Preload dotenv: `node --require dotenv/config index.js` (_Note: you do not need to `import` dotenv with this approach_)
2. Create a separate file that will execute `config` first as outlined in [this comment on #133](https://github.com/motdotla/dotenv/issues/133#issuecomment-255298822)

### Why am I getting the error `Module not found: Error: Can't resolve 'crypto|os|path'`?

You are using dotenv on the front-end and have not included a polyfill. Webpack < 5 used to include these for you. Do the following:

```bash
npm install node-polyfill-webpack-plugin
```

Configure your `webpack.config.js` to something like the following.

```js
require('dotenv').config()

const path = require('path');
const webpack = require('webpack')

const NodePolyfillPlugin = require('node-polyfill-webpack-plugin')

module.exports = {
  mode: 'development',
  entry: './src/index.ts',
  output: {
    filename: 'bundle.js',
    path: path.resolve(__dirname, 'dist'),
  },
  plugins: [
    new NodePolyfillPlugin(),
    new webpack.DefinePlugin({
      'process.env': {
        HELLO: JSON.stringify(process.env.HELLO)
      }
    }),
  ]
};
```

Alternatively, just use [dotenv-webpack](https://github.com/mrsteele/dotenv-webpack) which does this and more behind the scenes for you.

### What about variable expansion?

Try [dotenv-expand](https://github.com/motdotla/dotenv-expand)

### What about syncing and securing .env files?

Use [dotenvx](https://github.com/dotenvx/dotenvx)

### What if I accidentally commit my `.env` file to code?

Remove it, [remove git history](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository) and then install the [git pre-commit hook](https://github.com/dotenvx/dotenvx#pre-commit) to prevent this from ever happening again. 

```
brew install dotenvx/brew/dotenvx
dotenvx precommit --install
```

### How can I prevent committing my `.env` file to a Docker build?

Use the [docker prebuild hook](https://dotenvx.com/docs/features/prebuild).

```bash
# Dockerfile
...
RUN curl -fsS https://dotenvx.sh/ | sh
...
RUN dotenvx prebuild
CMD ["dotenvx", "run", "--", "node", "index.js"]
```

## Contributing Guide

See [CONTRIBUTING.md](CONTRIBUTING.md)

## CHANGELOG

See [CHANGELOG.md](CHANGELOG.md)

## Who's using dotenv?

[These npm modules depend on it.](https://www.npmjs.com/browse/depended/dotenv)

Projects that expand it often use the [keyword "dotenv" on npm](https://www.npmjs.com/search?q=keywords:dotenv).
```

### 文件: `node_modules/dunder-proto/README.md`

```markdown
# dunder-proto <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

If available, the `Object.prototype.__proto__` accessor and mutator, call-bound.

## Getting started

```sh
npm install --save dunder-proto
```

## Usage/Examples

```js
const assert = require('assert');
const getDunder = require('dunder-proto/get');
const setDunder = require('dunder-proto/set');

const obj = {};

assert.equal('toString' in obj, true);
assert.equal(getDunder(obj), Object.prototype);

setDunder(obj, null);

assert.equal('toString' in obj, false);
assert.equal(getDunder(obj), null);
```

## Tests

Clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/dunder-proto
[npm-version-svg]: https://versionbadg.es/es-shims/dunder-proto.svg
[deps-svg]: https://david-dm.org/es-shims/dunder-proto.svg
[deps-url]: https://david-dm.org/es-shims/dunder-proto
[dev-deps-svg]: https://david-dm.org/es-shims/dunder-proto/dev-status.svg
[dev-deps-url]: https://david-dm.org/es-shims/dunder-proto#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/dunder-proto.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/dunder-proto.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/dunder-proto.svg
[downloads-url]: https://npm-stat.com/charts.html?package=dunder-proto
[codecov-image]: https://codecov.io/gh/es-shims/dunder-proto/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/es-shims/dunder-proto/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/es-shims/dunder-proto
[actions-url]: https://github.com/es-shims/dunder-proto/actions
```

### 文件: `node_modules/ee-first/README.md`

```markdown
# EE First

[![NPM version][npm-image]][npm-url]
[![Build status][travis-image]][travis-url]
[![Test coverage][coveralls-image]][coveralls-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]
[![Gittip][gittip-image]][gittip-url]

Get the first event in a set of event emitters and event pairs,
then clean up after itself.

## Install

```sh
$ npm install ee-first
```

## API

```js
var first = require('ee-first')
```

### first(arr, listener)

Invoke `listener` on the first event from the list specified in `arr`. `arr` is
an array of arrays, with each array in the format `[ee, ...event]`. `listener`
will be called only once, the first time any of the given events are emitted. If
`error` is one of the listened events, then if that fires first, the `listener`
will be given the `err` argument.

The `listener` is invoked as `listener(err, ee, event, args)`, where `err` is the
first argument emitted from an `error` event, if applicable; `ee` is the event
emitter that fired; `event` is the string event name that fired; and `args` is an
array of the arguments that were emitted on the event.

```js
var ee1 = new EventEmitter()
var ee2 = new EventEmitter()

first([
  [ee1, 'close', 'end', 'error'],
  [ee2, 'error']
], function (err, ee, event, args) {
  // listener invoked
})
```

#### .cancel()

The group of listeners can be cancelled before being invoked and have all the event
listeners removed from the underlying event emitters.

```js
var thunk = first([
  [ee1, 'close', 'end', 'error'],
  [ee2, 'error']
], function (err, ee, event, args) {
  // listener invoked
})

// cancel and clean up
thunk.cancel()
```

[npm-image]: https://img.shields.io/npm/v/ee-first.svg?style=flat-square
[npm-url]: https://npmjs.org/package/ee-first
[github-tag]: http://img.shields.io/github/tag/jonathanong/ee-first.svg?style=flat-square
[github-url]: https://github.com/jonathanong/ee-first/tags
[travis-image]: https://img.shields.io/travis/jonathanong/ee-first.svg?style=flat-square
[travis-url]: https://travis-ci.org/jonathanong/ee-first
[coveralls-image]: https://img.shields.io/coveralls/jonathanong/ee-first.svg?style=flat-square
[coveralls-url]: https://coveralls.io/r/jonathanong/ee-first?branch=master
[license-image]: http://img.shields.io/npm/l/ee-first.svg?style=flat-square
[license-url]: LICENSE.md
[downloads-image]: http://img.shields.io/npm/dm/ee-first.svg?style=flat-square
[downloads-url]: https://npmjs.org/package/ee-first
[gittip-image]: https://img.shields.io/gittip/jonathanong.svg?style=flat-square
[gittip-url]: https://www.gittip.com/jonathanong/
```

### 文件: `node_modules/es-define-property/README.md`

```markdown
# es-define-property <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

`Object.defineProperty`, but not IE 8's broken one.

## Example

```js
const assert = require('assert');

const $defineProperty = require('es-define-property');

if ($defineProperty) {
    assert.equal($defineProperty, Object.defineProperty);
} else if (Object.defineProperty) {
    assert.equal($defineProperty, false, 'this is IE 8');
} else {
    assert.equal($defineProperty, false, 'this is an ES3 engine');
}
```

## Tests
Simply clone the repo, `npm install`, and run `npm test`

## Security

Please email [@ljharb](https://github.com/ljharb) or see https://tidelift.com/security if you have a potential security vulnerability to report.

[package-url]: https://npmjs.org/package/es-define-property
[npm-version-svg]: https://versionbadg.es/ljharb/es-define-property.svg
[deps-svg]: https://david-dm.org/ljharb/es-define-property.svg
[deps-url]: https://david-dm.org/ljharb/es-define-property
[dev-deps-svg]: https://david-dm.org/ljharb/es-define-property/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/es-define-property#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/es-define-property.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/es-define-property.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/es-define-property.svg
[downloads-url]: https://npm-stat.com/charts.html?package=es-define-property
[codecov-image]: https://codecov.io/gh/ljharb/es-define-property/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/es-define-property/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/es-define-property
[actions-url]: https://github.com/ljharb/es-define-property/actions
```

### 文件: `node_modules/es-errors/README.md`

```markdown
# es-errors <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

A simple cache for a few of the JS Error constructors.

## Example

```js
const assert = require('assert');

const Base = require('es-errors');
const Eval = require('es-errors/eval');
const Range = require('es-errors/range');
const Ref = require('es-errors/ref');
const Syntax = require('es-errors/syntax');
const Type = require('es-errors/type');
const URI = require('es-errors/uri');

assert.equal(Base, Error);
assert.equal(Eval, EvalError);
assert.equal(Range, RangeError);
assert.equal(Ref, ReferenceError);
assert.equal(Syntax, SyntaxError);
assert.equal(Type, TypeError);
assert.equal(URI, URIError);
```

## Tests
Simply clone the repo, `npm install`, and run `npm test`

## Security

Please email [@ljharb](https://github.com/ljharb) or see https://tidelift.com/security if you have a potential security vulnerability to report.

[package-url]: https://npmjs.org/package/es-errors
[npm-version-svg]: https://versionbadg.es/ljharb/es-errors.svg
[deps-svg]: https://david-dm.org/ljharb/es-errors.svg
[deps-url]: https://david-dm.org/ljharb/es-errors
[dev-deps-svg]: https://david-dm.org/ljharb/es-errors/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/es-errors#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/es-errors.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/es-errors.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/es-errors.svg
[downloads-url]: https://npm-stat.com/charts.html?package=es-errors
[codecov-image]: https://codecov.io/gh/ljharb/es-errors/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/es-errors/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/es-errors
[actions-url]: https://github.com/ljharb/es-errors/actions
```

### 文件: `node_modules/es-object-atoms/README.md`

```markdown
# es-object-atoms <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

ES Object-related atoms: Object, ToObject, RequireObjectCoercible.

## Example

```js
const assert = require('assert');

const $Object = require('es-object-atoms');
const isObject = require('es-object-atoms/isObject');
const ToObject = require('es-object-atoms/ToObject');
const RequireObjectCoercible = require('es-object-atoms/RequireObjectCoercible');

assert.equal($Object, Object);
assert.throws(() => ToObject(null), TypeError);
assert.throws(() => ToObject(undefined), TypeError);
assert.throws(() => RequireObjectCoercible(null), TypeError);
assert.throws(() => RequireObjectCoercible(undefined), TypeError);

assert.equal(isObject(undefined), false);
assert.equal(isObject(null), false);
assert.equal(isObject({}), true);
assert.equal(isObject([]), true);
assert.equal(isObject(function () {}), true);

assert.deepEqual(RequireObjectCoercible(true), true);
assert.deepEqual(ToObject(true), Object(true));

const obj = {};
assert.equal(RequireObjectCoercible(obj), obj);
assert.equal(ToObject(obj), obj);
```

## Tests
Simply clone the repo, `npm install`, and run `npm test`

## Security

Please email [@ljharb](https://github.com/ljharb) or see https://tidelift.com/security if you have a potential security vulnerability to report.

[package-url]: https://npmjs.org/package/es-object-atoms
[npm-version-svg]: https://versionbadg.es/ljharb/es-object-atoms.svg
[deps-svg]: https://david-dm.org/ljharb/es-object-atoms.svg
[deps-url]: https://david-dm.org/ljharb/es-object-atoms
[dev-deps-svg]: https://david-dm.org/ljharb/es-object-atoms/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/es-object-atoms#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/es-object-atoms.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/es-object-atoms.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/es-object.svg
[downloads-url]: https://npm-stat.com/charts.html?package=es-object-atoms
[codecov-image]: https://codecov.io/gh/ljharb/es-object-atoms/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/es-object-atoms/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/es-object-atoms
[actions-url]: https://github.com/ljharb/es-object-atoms/actions
```

### 文件: `node_modules/es6-promise/README.md`

```markdown
# ES6-Promise (subset of [rsvp.js](https://github.com/tildeio/rsvp.js)) [![Build Status](https://travis-ci.org/stefanpenner/es6-promise.svg?branch=master)](https://travis-ci.org/stefanpenner/es6-promise)

This is a polyfill of the [ES6 Promise](http://www.ecma-international.org/ecma-262/6.0/#sec-promise-constructor). The implementation is a subset of [rsvp.js](https://github.com/tildeio/rsvp.js) extracted by @jakearchibald, if you're wanting extra features and more debugging options, check out the [full library](https://github.com/tildeio/rsvp.js).

For API details and how to use promises, see the <a href="http://www.html5rocks.com/en/tutorials/es6/promises/">JavaScript Promises HTML5Rocks article</a>.

## Downloads

* [es6-promise 27.86 KB (7.33 KB gzipped)](https://cdn.jsdelivr.net/npm/es6-promise/dist/es6-promise.js)
* [es6-promise-auto 27.78 KB (7.3 KB gzipped)](https://cdn.jsdelivr.net/npm/es6-promise/dist/es6-promise.auto.js) - Automatically provides/replaces `Promise` if missing or broken.
* [es6-promise-min 6.17 KB (2.4 KB gzipped)](https://cdn.jsdelivr.net/npm/es6-promise/dist/es6-promise.min.js)
* [es6-promise-auto-min 6.19 KB (2.4 KB gzipped)](https://cdn.jsdelivr.net/npm/es6-promise/dist/es6-promise.auto.min.js) - Minified version of `es6-promise-auto` above.

## CDN 

To use via a CDN include this in your html:

```html
<!-- Automatically provides/replaces `Promise` if missing or broken. -->
<script src="https://cdn.jsdelivr.net/npm/es6-promise@4/dist/es6-promise.js"></script>
<script src="https://cdn.jsdelivr.net/npm/es6-promise@4/dist/es6-promise.auto.js"></script> 

<!-- Minified version of `es6-promise-auto` below. -->
<script src="https://cdn.jsdelivr.net/npm/es6-promise@4/dist/es6-promise.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/es6-promise@4/dist/es6-promise.auto.min.js"></script> 

```

## Node.js

To install:

```sh
yarn add es6-promise
```

or

```sh
npm install es6-promise
```

To use:

```js
var Promise = require('es6-promise').Promise;
```


## Usage in IE<9

`catch` and `finally` are reserved keywords in IE<9, meaning
`promise.catch(func)` or `promise.finally(func)` throw a syntax error. To work
around this, you can use a string to access the property as shown in the
following example.

However most minifiers will automatically fix this for you, making the
resulting code safe for old browsers and production:

```js
promise['catch'](function(err) {
  // ...
});
```

```js
promise['finally'](function() {
  // ...
});
```

## Auto-polyfill

To polyfill the global environment (either in Node or in the browser via CommonJS) use the following code snippet:

```js
require('es6-promise').polyfill();
```

Alternatively

```js
require('es6-promise/auto');
```

Notice that we don't assign the result of `polyfill()` to any variable. The `polyfill()` method will patch the global environment (in this case to the `Promise` name) when called.

## Building & Testing

You will need to have PhantomJS installed globally in order to run the tests.

`npm install -g phantomjs`

* `npm run build` to build
* `npm test` to run tests
* `npm start` to run a build watcher, and webserver to test
* `npm run test:server` for a testem test runner and watching builder
```

### 文件: `node_modules/express-http-proxy/README.md`

```markdown
# express-http-proxy [![NPM version](https://badge.fury.io/js/express-http-proxy.svg)](http://badge.fury.io/js/express-http-proxy) [![Build Status](https://travis-ci.org/villadora/express-http-proxy.svg?branch=master)](https://travis-ci.org/villadora/express-http-proxy) 


Express middleware to proxy request to another host and pass response back to original caller.

## Install

```bash
$ npm install express-http-proxy --save
```

## Usage
```js
proxy(host, options);
```

### Example:
To proxy URLS starting with '/proxy' to the host 'www.google.com':

```js
var proxy = require('express-http-proxy');
var app = require('express')();

app.use('/proxy', proxy('www.google.com'));
```

### Streaming

Proxy requests and user responses are piped/streamed/chunked by default.

If you define a response modifier (userResDecorator, userResHeaderDecorator),
or need to inspect the response before continuing (maybeSkipToNext), streaming
is disabled, and the request and response are buffered.
This can cause performance issues with large payloads.

### Promises

Many function hooks support Promises.
If any Promise is rejected, ```next(x)``` is called in the hosting application, where ```x``` is whatever you pass to ```Promise.reject```;


e.g.
```js
  app.use(proxy('/reject-promise', {
    proxyReqOptDecorator: function() {
      return Promise.reject('An arbitrary rejection message.');
    }
  }));
```

eventually calls

```js
next('An arbitrary rejection messasage');
```

### Host

The first positional argument is for the proxy host;  in many cases you will use a static string here, eg.

```js
app.use('/', proxy('http://google.com'))
```

However, this argument can also be a function, and that function can be
memoized or computed on each request, based on the setting of
```memoizeHost```.

```js
function selectProxyHost() {
  return (new Date() % 2) ? 'http://google.com' : 'http://altavista.com';
}

app.use('/', proxy(selectProxyHost));
```

Notie: Host is only the host name. Any params after in url will be ignored. For ``http://google.com/myPath`, ``myPath`` will be ignored because the host name is ``google.com``. 
See ``proxyReqPathResolver`` for more detailed path information.


### Middleware mixing

If you use 'https://www.npmjs.com/package/body-parser' you should declare it AFTER the proxy configuration, otherwise  original 'POST' body could be modified and not proxied correctly.

```js
app.use('/proxy', proxy('http://foo.bar.com'))

// Declare use of body-parser AFTER the use of proxy
app.use(bodyParser.foo(bar))
app.use('/api', ...)
```

If this cannot be avoided and you MUST proxy after `body-parser` has been registered, set `parseReqBody` to `false` and explicitly specify the body you wish to send in `proxyReqBodyDecorator`.

```js
app.use(bodyParser.foo(bar))

app.use('/proxy', proxy('http://foo.bar.com', {
  parseReqBody: false,
  proxyReqBodyDecorator: function () {

  },
}))
```

### Options

#### proxyReqPathResolver (supports Promises)

Note: In ```express-http-proxy```, the ```path``` is considered the portion of
the url after the host, and including all query params.  E.g. for the URL
```http://smoogle.com/search/path?q=123```; the path is
```/search/path?q=123```.   Authors using this resolver must also handle the query parameter portion of the path.

Provide a proxyReqPathResolver function if you'd like to
operate on the path before issuing the proxy request.  Use a Promise for async
operations.

```js
  app.use(proxy('localhost:12345', {
    proxyReqPathResolver: function (req) {
      var parts = req.url.split('?');
      var queryString = parts[1];
      var updatedPath = parts[0].replace(/test/, 'tent');
      return updatedPath + (queryString ? '?' + queryString : '');
    }
  }));
```
Promise form

```js
app.use('/proxy', proxy('localhost:12345', {
  proxyReqPathResolver: function(req) {
    return new Promise(function (resolve, reject) {
      setTimeout(function () {   // simulate async
        var parts = req.url.split('?');
        var queryString = parts[1];
        var updatedPath = parts[0].replace(/test/, 'tent');
        var resolvedPathValue = updatedPath + (queryString ? '?' + queryString : '');
        resolve(resolvedPathValue);
      }, 200);
    });
  }
}));
```

#### forwardPath

DEPRECATED.  See proxyReqPathResolver

#### forwardPathAsync

DEPRECATED. See proxyReqPathResolver

#### filter (supports Promises)

The ```filter``` option can be used to limit what requests are proxied.  Return
```true``` to continue to execute proxy; return false-y to skip proxy for this
request.

For example, if you only want to proxy get request:

```js
app.use('/proxy', proxy('www.google.com', {
  filter: function(req, res) {
     return req.method == 'GET';
  }
}));
```

Promise form:

```js
  app.use(proxy('localhost:12346', {
    filter: function (req, res) { 
      return new Promise(function (resolve) { 
        resolve(req.method === 'GET');
      }); 
    }
  }));
```

Note that in the previous example, `resolve(false)` will execute the happy path
for filter here (skipping the rest of the proxy, and calling `next()`).
`reject()` will also skip the rest of proxy and call `next()`. 

#### userResDecorator (was: intercept) (supports Promise)

You can modify the proxy's response before sending it to the client.

```js
app.use('/proxy', proxy('www.google.com', {
  userResDecorator: function(proxyRes, proxyResData, userReq, userRes) {
    data = JSON.parse(proxyResData.toString('utf8'));
    data.newProperty = 'exciting data';
    return JSON.stringify(data);
  }
}));
```

```js
app.use(proxy('httpbin.org', {
  userResDecorator: function(proxyRes, proxyResData) {
    return new Promise(function(resolve) {
      proxyResData.funkyMessage = 'oi io oo ii';
      setTimeout(function() {
        resolve(proxyResData);
      }, 200);
    });
  }
}));
```

##### 304 - Not Modified

When your proxied service returns 304, not modified, this step will be skipped, since there is no body to decorate.

##### exploiting references
The intent is that this be used to modify the proxy response data only.

Note:
The other arguments (proxyRes, userReq, userRes) are passed by reference, so
you *can* currently exploit this to modify either response's headers, for
instance, but this is not a reliable interface. I expect to close this
exploit in a future release, while providing an additional hook for mutating
the userRes before sending.

##### gzip responses

If your proxy response is gzipped, this program will automatically unzip
it before passing to your function, then zip it back up before piping it to the
user response.  There is currently no way to short-circuit this behavior.

#### limit

This sets the body size limit (default: `1mb`). If the body size is larger than the specified (or default) limit,
a `413 Request Entity Too Large`  error will be returned. See [bytes.js](https://www.npmjs.com/package/bytes) for
a list of supported formats.

```js
app.use('/proxy', proxy('www.google.com', {
  limit: '5mb'
}));
```

#### memoizeHost

Defaults to ```true```.

When true, the ```host``` argument will be parsed on first request, and
memoized for subsequent requests.

When ```false```, ```host``` argument will be parsed on each request.

E.g.,

```js

  function coinToss() { return Math.random() > .5 }
  function getHost() { return coinToss() ? 'http://yahoo.com' : 'http://google.com' }

  app.use(proxy(getHost, {
    memoizeHost: false
  }))
```

In this example, when ```memoizeHost:false```, the coinToss occurs on each
request, and each request could get either value.

Conversely, When ```memoizeHost:true```,  the coinToss would occur on the first
request, and all additional requests would return the value resolved on the
first request.


### userResHeaderDecorator

When a `userResHeaderDecorator` is defined, the return of this method will replace (rather than be merged on to) the headers for `userRes`.

```js
app.use('/proxy', proxy('www.google.com', {
  userResHeaderDecorator(headers, userReq, userRes, proxyReq, proxyRes) {
    // recieves an Object of headers, returns an Object of headers.
    return headers;
  }
}));
```


#### decorateRequest

REMOVED:  See ```proxyReqOptDecorator``` and ```proxyReqBodyDecorator```.


#### skipToNextHandlerFilter(supports Promise form)
(experimental: this interface may change in upcoming versions)

Allows you to inspect the proxy response, and decide if you want to continue processing (via express-http-proxy) or call ```next()``` to return control to express.

```js
app.use('/proxy', proxy('www.google.com', {
  skipToNextHandlerFilter: function(proxyRes) {
    return proxyRes.statusCode === 404;
  }
}));
```

### proxyErrorHandler

By default, ```express-http-proxy``` will pass any errors except ECONNRESET to
next, so that your application can handle or react to them, or just drop
through to your default error handling.   ECONNRESET errors are immediately
returned to the user for historical reasons.

If you would like to modify this behavior, you can provide your own ```proxyErrorHandler```.


```js
// Example of skipping all error handling.

app.use(proxy('localhost:12346', {
  proxyErrorHandler: function(err, res, next) {
    next(err);
  }
}));


// Example of rolling your own

app.use(proxy('localhost:12346', {
  proxyErrorHandler: function(err, res, next) {
    switch (err && err.code) {
      case 'ECONNRESET':    { return res.status(405).send('504 became 405'); }
      case 'ECONNREFUSED':  { return res.status(200).send('gotcher back'); }
      default:              { next(err); }
    }
}}));
```



#### proxyReqOptDecorator  (supports Promise form)

You can override most request options before issuing the proxyRequest.
proxyReqOpt represents the options argument passed to the (http|https).request
module.

NOTE:  req.path cannot be changed via this method;  use ```proxyReqPathResolver``` instead.   (see https://github.com/villadora/express-http-proxy/issues/243)

```js
app.use('/proxy', proxy('www.google.com', {
  proxyReqOptDecorator: function(proxyReqOpts, srcReq) {
    // you can update headers
    proxyReqOpts.headers['Content-Type'] = 'text/html';
    // you can change the method
    proxyReqOpts.method = 'GET';
    return proxyReqOpts;
  }
}));
```

You can use a Promise for async style.

```js
app.use('/proxy', proxy('www.google.com', {
  proxyReqOptDecorator: function(proxyReqOpts, srcReq) {
    return new Promise(function(resolve, reject) {
      proxyReqOpts.headers['Content-Type'] = 'text/html';
      resolve(proxyReqOpts);
    })
  }
}));
```

#### proxyReqBodyDecorator  (supports Promise form)

You can mutate the body content before sending the proxyRequest.

```js
app.use('/proxy', proxy('www.google.com', {
  proxyReqBodyDecorator: function(bodyContent, srcReq) {
    return bodyContent.split('').reverse().join('');
  }
}));
```

You can use a Promise for async style.

```js
app.use('/proxy', proxy('www.google.com', {
  proxyReqBodyDecorator: function(proxyReq, srcReq) {
    return new Promise(function(resolve, reject) {
      http.get('http://dev/null', function (err, res) {
        if (err) { reject(err); }
        resolve(res);
      });
    })
  }
}));
```

#### https

Normally, your proxy request will be made on the same protocol as the `host`
parameter.  If you'd like to force the proxy request to be https, use this
option.

```js
app.use('/proxy', proxy('www.google.com', {
  https: true
}));
```

#### preserveHostHdr

You can copy the host HTTP header to the proxied express server using the `preserveHostHdr` option.

```js
app.use('/proxy', proxy('www.google.com', {
  preserveHostHdr: true
}));
```

#### parseReqBody

The ```parseReqBody``` option allows you to control parsing the request body.
For example, disabling body parsing is useful for large uploads where it would be inefficient
to hold the data in memory.

##### Note: this setting is required for binary uploads.   A future version of this library may handle this for you.

This defaults to true in order to preserve legacy behavior.

When false, no action will be taken on the body and accordingly ```req.body``` will no longer be set.

Note that setting this to false overrides ```reqAsBuffer``` and ```reqBodyEncoding``` below.

```js
app.use('/proxy', proxy('www.google.com', {
  parseReqBody: false
}));
```

#### reqAsBuffer

Note: this is an experimental feature.  ymmv

The ```reqAsBuffer``` option allows you to ensure the req body is encoded as a Node
```Buffer``` when sending a proxied request.   Any value for this is truthy.

This defaults to to false in order to preserve legacy behavior. Note that
the value of ```reqBodyEnconding``` is used as the encoding when coercing strings
(and stringified JSON) to Buffer.

Ignored if ```parseReqBody``` is set to false.

```js
app.use('/proxy', proxy('www.google.com', {
  reqAsBuffer: true
}));
```

#### reqBodyEncoding

Encoding used to decode request body. Defaults to ```utf-8```.

Use ```null``` to preserve as Buffer when proxied request body is a Buffer. (e.g image upload)
Accept any values supported by [raw-body](https://www.npmjs.com/package/raw-body#readme).

The same encoding is used in the intercept method.

Ignored if ```parseReqBody``` is set to false.

```js
app.use('/post', proxy('httpbin.org', {
  reqBodyEncoding: null
}));
```

#### timeout

By default, node does not express a timeout on connections.
Use timeout option to impose a specific timeout.
Timed-out requests will respond with 504 status code and a X-Timeout-Reason header.

```js
app.use('/', proxy('httpbin.org', {
  timeout: 2000  // in milliseconds, two seconds
}));
```

##  Trace debugging

The node-debug module is used to provide a trace debugging capability.

```
DEBUG=express-http-proxy npm run YOUR_PROGRAM
DEBUG=express-http-proxy npm run YOUR_PROGRAM  | grep 'express-http-proxy'   # to filter down to just these messages
```

Will trace the execution of the express-http-proxy module in order to aide debugging.




## Upgrade to 1.0, transition guide and breaking changes

1.
```decorateRequest``` has been REMOVED, and will generate an error when called.  See ```proxyReqOptDecorator``` and ```proxyReqBodyDecorator```.

Resolution:  Most authors will simply need to change the method name for their
decorateRequest method;  if author was decorating reqOpts and reqBody in the
same method, this will need to be split up.


2.
```intercept``` has been REMOVED, and will generate an error when called.  See ```userResDecorator```.

Resolution:  Most authors will simply need to change the method name from ```intercept``` to ```userResDecorator```, and exit the method by returning the value, rather than passing it to a callback.   E.g.:

Before:

```js
app.use('/proxy', proxy('www.google.com', {
  intercept: function(proxyRes, proxyResData, userReq, userRes, cb) {
    data = JSON.parse(proxyResData.toString('utf8'));
    data.newProperty = 'exciting data';
    cb(null,  JSON.stringify(data));
  }
}));
```

Now:

```js
app.use('/proxy', proxy('www.google.com', {
  userResDecorator: function(proxyRes, proxyResData, userReq, userRes) {
    data = JSON.parse(proxyResData.toString('utf8'));
    data.newProperty = 'exciting data';
    return JSON.stringify(data);
  }
}));
```

3.
```forwardPath``` and ```forwardPathAsync``` have been DEPRECATED and will generate a warning when called.  See ```proxyReqPathResolver```.

Resolution:  Simple update the name of either ```forwardPath``` or ```forwardPathAsync``` to ```proxyReqPathResolver```.

## When errors occur on your proxy server

When your proxy server responds with an error, express-http-proxy returns a response with the same status code.  See ```test/catchingErrors``` for syntax details.

When your proxy server times out, express-http-proxy will continue to wait indefinitely for a response, unless you define a ```timeout``` as described above.


## Questions

### Q: Does it support https proxy?

The library will automatically use https if the provided path has 'https://' or ':443'.  You may also set option ```https``` to true to always use https.

You can use ```proxyReqOptDecorator``` to ammend any auth or challenge headers required to succeed https.

### Q: How can I support non-standard certificate chains?

You can use the ability to decorate the proxy request prior to sending.    See ```proxyReqOptDecorator``` for more details.

```js
app.use('/', proxy('internalhost.example.com', {
  proxyReqOptDecorator: function(proxyReqOpts, originalReq) {
    proxyReqOpts.ca =  [caCert, intermediaryCert]
    return proxyReqOpts;
  }
})
```

### Q: How to ignore self-signed certificates ?

You can set the `rejectUnauthorized` value in proxy request options prior to sending.    See ```proxyReqOptDecorator``` for more details.

```js
app.use('/', proxy('internalhost.example.com', {
  proxyReqOptDecorator: function(proxyReqOpts, originalReq) {
    proxyReqOpts.rejectUnauthorized = false
    return proxyReqOpts;
  }
}))
```


## Release Notes

| Release | Notes |
| --- | --- |
| 2.1.1 | (trivial) Fixes formatting in README.|
| 2.1.0 | Fixes parsing error in content-types. Improves behavior of proxyReqBodyDecorator when parseReqBody=false. Repairs issue where authors can't use proxy() twice in Express middleware stack.  Fix `new Buffer` deprecation warning. |
| 2.0.0 | Update all dependencies; set stage for next iteration. `express-http-proxy` interface has not changed, but the underlying libraries are not guaranteed to be backward compatible. Versions beyond this point are expected to be run in node verions >= 16. |
| ----- | ----------------------------------------------------------------------- |
| 1.6.3 | [#453] Author should be able to delete headers in userResHeaderDecorator.
| 1.6.2 | Update node.js versions used by ci. |
| 1.6.1 | Minor bug fixes and documentation. |
| 1.6.0 | Do gzip and gunzip aysyncronously.   Test and documentation improvements, dependency updates. |
| 1.5.1 | Fixes bug in stringifying debug messages. |
| 1.5.0 | Fixes bug in `filter` signature.  Fix bug in skipToNextHandler, add expressHttpProxy value to user res when skipped.  Add tests for host as ip address. |
| 1.4.0 | DEPRECATED. Critical bug in the `filter` api.| 
| 1.3.0 | DEPRECATED. Critical bug in the `filter` api. `filter` now supports Promises.  Update linter to eslint.  |
| 1.2.0 | Auto-stream when no decorations are made to req/res. Improved docs, fixes issues in maybeSkipToNexthandler,  allow authors to manage error handling. | 
| 1.1.0 | Add step to allow response headers to be modified.
| 1.0.7 | Update dependencies.  Improve docs on promise rejection.   Fix promise rejection on body limit.   Improve debug output. |
| 1.0.6 | Fixes preserveHostHdr not working, skip userResDecorator on 304, add maybeSkipToNext, test improvements and cleanup. |
| 1.0.5 | Minor documentation and  test patches |
| 1.0.4 | Minor documentation, test, and package fixes |
| 1.0.3 | Fixes 'limit option is not taken into account |
| 1.0.2 | Minor docs corrections. |
| 1.0.1 | Minor docs adjustments. |
| 1.0.0 | Major revision.  <br > REMOVE decorateRequest, ADD proxyReqOptDecorator and proxyReqBodyDecorator. <br />  REMOVE intercept, ADD userResDecorator <br />  userResDecorator supports a Promise form for async operations.  <br /> General cleanup of structure and application of hooks.  Documentation improvements.   Update all dependencies.  Re-organize code as a series of workflow steps, each (potentially) supporting a promise, and creating a reusable pattern for future development. |
| 0.11.0 | Allow author to prevent host from being memoized between requests.   General program cleanup. |
| 0.10.1| Fixed issue where 'body encoding' was being incorrectly set to the character encoding. <br />  Dropped explicit support for node 0.10. <br />   Intercept can now deal with gziped responses. <br />   Author can now 'force https', even if the original request is over http. <br />  Do not call next after ECONNRESET catch. |
| 0.10.0 | Fix regression in forwardPath implementation. |
| 0.9.1 | Documentation updates.  Set 'Accept-Encoding' header to match bodyEncoding. |
| 0.9.0 | Better handling for request body when body is JSON. |
| 0.8.0 | Features:  add forwardPathAsync option <br />Updates:  modernize dependencies <br />Fixes: Exceptions parsing proxied response causes error: Can't set headers after they are sent. (#111) <br />If client request aborts, proxied request is aborted too (#107) |
| 0.7.4 | Move jscs to devDependencies to avoid conflict with nsp. |
| 0.7.3 | Adds a timeout option.   Code organization and small bug fixes. |
| 0.7.2 | Collecting many minor documentation and test improvements. |
| 0.4.0 | Signature of `intercept` callback changed from `function(data, req, res, callback)` to `function(rsp, data, req, res, callback)` where `rsp` is the original response from the target |

## Licence

MIT
<!-- do not want to make nodeinit to complicated, you can edit this whenever you want. -->
```

### 文件: `node_modules/express-http-proxy/node_modules/debug/README.md`

```markdown
# debug
[![Build Status](https://travis-ci.org/visionmedia/debug.svg?branch=master)](https://travis-ci.org/visionmedia/debug)  [![Coverage Status](https://coveralls.io/repos/github/visionmedia/debug/badge.svg?branch=master)](https://coveralls.io/github/visionmedia/debug?branch=master)  [![Slack](https://visionmedia-community-slackin.now.sh/badge.svg)](https://visionmedia-community-slackin.now.sh/) [![OpenCollective](https://opencollective.com/debug/backers/badge.svg)](#backers)
[![OpenCollective](https://opencollective.com/debug/sponsors/badge.svg)](#sponsors)

<img width="647" src="https://user-images.githubusercontent.com/71256/29091486-fa38524c-7c37-11e7-895f-e7ec8e1039b6.png">

A tiny JavaScript debugging utility modelled after Node.js core's debugging
technique. Works in Node.js and web browsers.

## Installation

```bash
$ npm install debug
```

## Usage

`debug` exposes a function; simply pass this function the name of your module, and it will return a decorated version of `console.error` for you to pass debug statements to. This will allow you to toggle the debug output for different parts of your module as well as the module as a whole.

Example [_app.js_](./examples/node/app.js):

```js
var debug = require('debug')('http')
  , http = require('http')
  , name = 'My App';

// fake app

debug('booting %o', name);

http.createServer(function(req, res){
  debug(req.method + ' ' + req.url);
  res.end('hello\n');
}).listen(3000, function(){
  debug('listening');
});

// fake worker of some kind

require('./worker');
```

Example [_worker.js_](./examples/node/worker.js):

```js
var a = require('debug')('worker:a')
  , b = require('debug')('worker:b');

function work() {
  a('doing lots of uninteresting work');
  setTimeout(work, Math.random() * 1000);
}

work();

function workb() {
  b('doing some work');
  setTimeout(workb, Math.random() * 2000);
}

workb();
```

The `DEBUG` environment variable is then used to enable these based on space or
comma-delimited names.

Here are some examples:

<img width="647" alt="screen shot 2017-08-08 at 12 53 04 pm" src="https://user-images.githubusercontent.com/71256/29091703-a6302cdc-7c38-11e7-8304-7c0b3bc600cd.png">
<img width="647" alt="screen shot 2017-08-08 at 12 53 38 pm" src="https://user-images.githubusercontent.com/71256/29091700-a62a6888-7c38-11e7-800b-db911291ca2b.png">
<img width="647" alt="screen shot 2017-08-08 at 12 53 25 pm" src="https://user-images.githubusercontent.com/71256/29091701-a62ea114-7c38-11e7-826a-2692bedca740.png">

#### Windows command prompt notes

##### CMD

On Windows the environment variable is set using the `set` command.

```cmd
set DEBUG=*,-not_this
```

Example:

```cmd
set DEBUG=* & node app.js
```

##### PowerShell (VS Code default)

PowerShell uses different syntax to set environment variables.

```cmd
$env:DEBUG = "*,-not_this"
```

Example:

```cmd
$env:DEBUG='app';node app.js
```

Then, run the program to be debugged as usual.

npm script example:
```js
  "windowsDebug": "@powershell -Command $env:DEBUG='*';node app.js",
```

## Namespace Colors

Every debug instance has a color generated for it based on its namespace name.
This helps when visually parsing the debug output to identify which debug instance
a debug line belongs to.

#### Node.js

In Node.js, colors are enabled when stderr is a TTY. You also _should_ install
the [`supports-color`](https://npmjs.org/supports-color) module alongside debug,
otherwise debug will only use a small handful of basic colors.

<img width="521" src="https://user-images.githubusercontent.com/71256/29092181-47f6a9e6-7c3a-11e7-9a14-1928d8a711cd.png">

#### Web Browser

Colors are also enabled on "Web Inspectors" that understand the `%c` formatting
option. These are WebKit web inspectors, Firefox ([since version
31](https://hacks.mozilla.org/2014/05/editable-box-model-multiple-selection-sublime-text-keys-much-more-firefox-developer-tools-episode-31/))
and the Firebug plugin for Firefox (any version).

<img width="524" src="https://user-images.githubusercontent.com/71256/29092033-b65f9f2e-7c39-11e7-8e32-f6f0d8e865c1.png">


## Millisecond diff

When actively developing an application it can be useful to see when the time spent between one `debug()` call and the next. Suppose for example you invoke `debug()` before requesting a resource, and after as well, the "+NNNms" will show you how much time was spent between calls.

<img width="647" src="https://user-images.githubusercontent.com/71256/29091486-fa38524c-7c37-11e7-895f-e7ec8e1039b6.png">

When stdout is not a TTY, `Date#toISOString()` is used, making it more useful for logging the debug information as shown below:

<img width="647" src="https://user-images.githubusercontent.com/71256/29091956-6bd78372-7c39-11e7-8c55-c948396d6edd.png">


## Conventions

If you're using this in one or more of your libraries, you _should_ use the name of your library so that developers may toggle debugging as desired without guessing names. If you have more than one debuggers you _should_ prefix them with your library name and use ":" to separate features. For example "bodyParser" from Connect would then be "connect:bodyParser".  If you append a "*" to the end of your name, it will always be enabled regardless of the setting of the DEBUG environment variable.  You can then use it for normal output as well as debug output.

## Wildcards

The `*` character may be used as a wildcard. Suppose for example your library has
debuggers named "connect:bodyParser", "connect:compress", "connect:session",
instead of listing all three with
`DEBUG=connect:bodyParser,connect:compress,connect:session`, you may simply do
`DEBUG=connect:*`, or to run everything using this module simply use `DEBUG=*`.

You can also exclude specific debuggers by prefixing them with a "-" character.
For example, `DEBUG=*,-connect:*` would include all debuggers except those
starting with "connect:".

## Environment Variables

When running through Node.js, you can set a few environment variables that will
change the behavior of the debug logging:

| Name      | Purpose                                         |
|-----------|-------------------------------------------------|
| `DEBUG`   | Enables/disables specific debugging namespaces. |
| `DEBUG_HIDE_DATE` | Hide date from debug output (non-TTY).  |
| `DEBUG_COLORS`| Whether or not to use colors in the debug output. |
| `DEBUG_DEPTH` | Object inspection depth.                    |
| `DEBUG_SHOW_HIDDEN` | Shows hidden properties on inspected objects. |


__Note:__ The environment variables beginning with `DEBUG_` end up being
converted into an Options object that gets used with `%o`/`%O` formatters.
See the Node.js documentation for
[`util.inspect()`](https://nodejs.org/api/util.html#util_util_inspect_object_options)
for the complete list.

## Formatters

Debug uses [printf-style](https://wikipedia.org/wiki/Printf_format_string) formatting.
Below are the officially supported formatters:

| Formatter | Representation |
|-----------|----------------|
| `%O`      | Pretty-print an Object on multiple lines. |
| `%o`      | Pretty-print an Object all on a single line. |
| `%s`      | String. |
| `%d`      | Number (both integer and float). |
| `%j`      | JSON. Replaced with the string '[Circular]' if the argument contains circular references. |
| `%%`      | Single percent sign ('%'). This does not consume an argument. |


### Custom formatters

You can add custom formatters by extending the `debug.formatters` object.
For example, if you wanted to add support for rendering a Buffer as hex with
`%h`, you could do something like:

```js
const createDebug = require('debug')
createDebug.formatters.h = (v) => {
  return v.toString('hex')
}

// …elsewhere
const debug = createDebug('foo')
debug('this is hex: %h', new Buffer('hello world'))
//   foo this is hex: 68656c6c6f20776f726c6421 +0ms
```


## Browser Support

You can build a browser-ready script using [browserify](https://github.com/substack/node-browserify),
or just use the [browserify-as-a-service](https://wzrd.in/) [build](https://wzrd.in/standalone/debug@latest),
if you don't want to build it yourself.

Debug's enable state is currently persisted by `localStorage`.
Consider the situation shown below where you have `worker:a` and `worker:b`,
and wish to debug both. You can enable this using `localStorage.debug`:

```js
localStorage.debug = 'worker:*'
```

And then refresh the page.

```js
a = debug('worker:a');
b = debug('worker:b');

setInterval(function(){
  a('doing some work');
}, 1000);

setInterval(function(){
  b('doing some work');
}, 1200);
```


## Output streams

  By default `debug` will log to stderr, however this can be configured per-namespace by overriding the `log` method:

Example [_stdout.js_](./examples/node/stdout.js):

```js
var debug = require('debug');
var error = debug('app:error');

// by default stderr is used
error('goes to stderr!');

var log = debug('app:log');
// set this namespace to log via console.log
log.log = console.log.bind(console); // don't forget to bind to console!
log('goes to stdout');
error('still goes to stderr!');

// set all output to go via console.info
// overrides all per-namespace log settings
debug.log = console.info.bind(console);
error('now goes to stdout via console.info');
log('still goes to stdout, but via console.info now');
```

## Extend
You can simply extend debugger 
```js
const log = require('debug')('auth');

//creates new debug instance with extended namespace
const logSign = log.extend('sign');
const logLogin = log.extend('login');

log('hello'); // auth hello
logSign('hello'); //auth:sign hello
logLogin('hello'); //auth:login hello
```

## Set dynamically

You can also enable debug dynamically by calling the `enable()` method :

```js
let debug = require('debug');

console.log(1, debug.enabled('test'));

debug.enable('test');
console.log(2, debug.enabled('test'));

debug.disable();
console.log(3, debug.enabled('test'));

```

print :   
```
1 false
2 true
3 false
```

Usage :  
`enable(namespaces)`  
`namespaces` can include modes separated by a colon and wildcards.
   
Note that calling `enable()` completely overrides previously set DEBUG variable : 

```
$ DEBUG=foo node -e 'var dbg = require("debug"); dbg.enable("bar"); console.log(dbg.enabled("foo"))'
=> false
```

## Checking whether a debug target is enabled

After you've created a debug instance, you can determine whether or not it is
enabled by checking the `enabled` property:

```javascript
const debug = require('debug')('http');

if (debug.enabled) {
  // do stuff...
}
```

You can also manually toggle this property to force the debug instance to be
enabled or disabled.


## Authors

 - TJ Holowaychuk
 - Nathan Rajlich
 - Andrew Rhyne

## Backers

Support us with a monthly donation and help us continue our activities. [[Become a backer](https://opencollective.com/debug#backer)]

<a href="https://opencollective.com/debug/backer/0/website" target="_blank"><img src="https://opencollective.com/debug/backer/0/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/1/website" target="_blank"><img src="https://opencollective.com/debug/backer/1/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/2/website" target="_blank"><img src="https://opencollective.com/debug/backer/2/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/3/website" target="_blank"><img src="https://opencollective.com/debug/backer/3/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/4/website" target="_blank"><img src="https://opencollective.com/debug/backer/4/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/5/website" target="_blank"><img src="https://opencollective.com/debug/backer/5/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/6/website" target="_blank"><img src="https://opencollective.com/debug/backer/6/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/7/website" target="_blank"><img src="https://opencollective.com/debug/backer/7/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/8/website" target="_blank"><img src="https://opencollective.com/debug/backer/8/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/9/website" target="_blank"><img src="https://opencollective.com/debug/backer/9/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/10/website" target="_blank"><img src="https://opencollective.com/debug/backer/10/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/11/website" target="_blank"><img src="https://opencollective.com/debug/backer/11/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/12/website" target="_blank"><img src="https://opencollective.com/debug/backer/12/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/13/website" target="_blank"><img src="https://opencollective.com/debug/backer/13/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/14/website" target="_blank"><img src="https://opencollective.com/debug/backer/14/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/15/website" target="_blank"><img src="https://opencollective.com/debug/backer/15/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/16/website" target="_blank"><img src="https://opencollective.com/debug/backer/16/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/17/website" target="_blank"><img src="https://opencollective.com/debug/backer/17/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/18/website" target="_blank"><img src="https://opencollective.com/debug/backer/18/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/19/website" target="_blank"><img src="https://opencollective.com/debug/backer/19/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/20/website" target="_blank"><img src="https://opencollective.com/debug/backer/20/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/21/website" target="_blank"><img src="https://opencollective.com/debug/backer/21/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/22/website" target="_blank"><img src="https://opencollective.com/debug/backer/22/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/23/website" target="_blank"><img src="https://opencollective.com/debug/backer/23/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/24/website" target="_blank"><img src="https://opencollective.com/debug/backer/24/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/25/website" target="_blank"><img src="https://opencollective.com/debug/backer/25/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/26/website" target="_blank"><img src="https://opencollective.com/debug/backer/26/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/27/website" target="_blank"><img src="https://opencollective.com/debug/backer/27/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/28/website" target="_blank"><img src="https://opencollective.com/debug/backer/28/avatar.svg"></a>
<a href="https://opencollective.com/debug/backer/29/website" target="_blank"><img src="https://opencollective.com/debug/backer/29/avatar.svg"></a>


## Sponsors

Become a sponsor and get your logo on our README on Github with a link to your site. [[Become a sponsor](https://opencollective.com/debug#sponsor)]

<a href="https://opencollective.com/debug/sponsor/0/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/0/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/1/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/1/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/2/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/2/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/3/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/3/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/4/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/4/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/5/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/5/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/6/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/6/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/7/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/7/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/8/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/8/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/9/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/9/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/10/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/10/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/11/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/11/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/12/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/12/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/13/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/13/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/14/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/14/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/15/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/15/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/16/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/16/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/17/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/17/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/18/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/18/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/19/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/19/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/20/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/20/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/21/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/21/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/22/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/22/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/23/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/23/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/24/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/24/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/25/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/25/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/26/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/26/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/27/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/27/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/28/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/28/avatar.svg"></a>
<a href="https://opencollective.com/debug/sponsor/29/website" target="_blank"><img src="https://opencollective.com/debug/sponsor/29/avatar.svg"></a>

## License

(The MIT License)

Copyright (c) 2014-2017 TJ Holowaychuk &lt;tj@vision-media.ca&gt;

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the
'Software'), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to
the following conditions:

The above copyright notice and this permission notice shall be
included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

### 文件: `node_modules/express-http-proxy/node_modules/iconv-lite/README.md`

```markdown
## Pure JS character encoding conversion [![Build Status](https://travis-ci.org/ashtuchkin/iconv-lite.svg?branch=master)](https://travis-ci.org/ashtuchkin/iconv-lite)

 * Doesn't need native code compilation. Works on Windows and in sandboxed environments like [Cloud9](http://c9.io).
 * Used in popular projects like [Express.js (body_parser)](https://github.com/expressjs/body-parser), 
   [Grunt](http://gruntjs.com/), [Nodemailer](http://www.nodemailer.com/), [Yeoman](http://yeoman.io/) and others.
 * Faster than [node-iconv](https://github.com/bnoordhuis/node-iconv) (see below for performance comparison).
 * Intuitive encode/decode API
 * Streaming support for Node v0.10+
 * [Deprecated] Can extend Node.js primitives (buffers, streams) to support all iconv-lite encodings.
 * In-browser usage via [Browserify](https://github.com/substack/node-browserify) (~180k gzip compressed with Buffer shim included).
 * Typescript [type definition file](https://github.com/ashtuchkin/iconv-lite/blob/master/lib/index.d.ts) included.
 * React Native is supported (need to explicitly `npm install` two more modules: `buffer` and `stream`).
 * License: MIT.

[![NPM Stats](https://nodei.co/npm/iconv-lite.png?downloads=true&downloadRank=true)](https://npmjs.org/packages/iconv-lite/)

## Usage
### Basic API
```javascript
var iconv = require('iconv-lite');

// Convert from an encoded buffer to js string.
str = iconv.decode(Buffer.from([0x68, 0x65, 0x6c, 0x6c, 0x6f]), 'win1251');

// Convert from js string to an encoded buffer.
buf = iconv.encode("Sample input string", 'win1251');

// Check if encoding is supported
iconv.encodingExists("us-ascii")
```

### Streaming API (Node v0.10+)
```javascript

// Decode stream (from binary stream to js strings)
http.createServer(function(req, res) {
    var converterStream = iconv.decodeStream('win1251');
    req.pipe(converterStream);

    converterStream.on('data', function(str) {
        console.log(str); // Do something with decoded strings, chunk-by-chunk.
    });
});

// Convert encoding streaming example
fs.createReadStream('file-in-win1251.txt')
    .pipe(iconv.decodeStream('win1251'))
    .pipe(iconv.encodeStream('ucs2'))
    .pipe(fs.createWriteStream('file-in-ucs2.txt'));

// Sugar: all encode/decode streams have .collect(cb) method to accumulate data.
http.createServer(function(req, res) {
    req.pipe(iconv.decodeStream('win1251')).collect(function(err, body) {
        assert(typeof body == 'string');
        console.log(body); // full request body string
    });
});
```

### [Deprecated] Extend Node.js own encodings
> NOTE: This doesn't work on latest Node versions. See [details](https://github.com/ashtuchkin/iconv-lite/wiki/Node-v4-compatibility).

```javascript
// After this call all Node basic primitives will understand iconv-lite encodings.
iconv.extendNodeEncodings();

// Examples:
buf = new Buffer(str, 'win1251');
buf.write(str, 'gbk');
str = buf.toString('latin1');
assert(Buffer.isEncoding('iso-8859-15'));
Buffer.byteLength(str, 'us-ascii');

http.createServer(function(req, res) {
    req.setEncoding('big5');
    req.collect(function(err, body) {
        console.log(body);
    });
});

fs.createReadStream("file.txt", "shift_jis");

// External modules are also supported (if they use Node primitives, which they probably do).
request = require('request');
request({
    url: "http://github.com/", 
    encoding: "cp932"
});

// To remove extensions
iconv.undoExtendNodeEncodings();
```

## Supported encodings

 *  All node.js native encodings: utf8, ucs2 / utf16-le, ascii, binary, base64, hex.
 *  Additional unicode encodings: utf16, utf16-be, utf-7, utf-7-imap.
 *  All widespread singlebyte encodings: Windows 125x family, ISO-8859 family, 
    IBM/DOS codepages, Macintosh family, KOI8 family, all others supported by iconv library. 
    Aliases like 'latin1', 'us-ascii' also supported.
 *  All widespread multibyte encodings: CP932, CP936, CP949, CP950, GB2312, GBK, GB18030, Big5, Shift_JIS, EUC-JP.

See [all supported encodings on wiki](https://github.com/ashtuchkin/iconv-lite/wiki/Supported-Encodings).

Most singlebyte encodings are generated automatically from [node-iconv](https://github.com/bnoordhuis/node-iconv). Thank you Ben Noordhuis and libiconv authors!

Multibyte encodings are generated from [Unicode.org mappings](http://www.unicode.org/Public/MAPPINGS/) and [WHATWG Encoding Standard mappings](http://encoding.spec.whatwg.org/). Thank you, respective authors!


## Encoding/decoding speed

Comparison with node-iconv module (1000x256kb, on MacBook Pro, Core i5/2.6 GHz, Node v0.12.0). 
Note: your results may vary, so please always check on your hardware.

    operation             iconv@2.1.4   iconv-lite@0.4.7
    ----------------------------------------------------------
    encode('win1251')     ~96 Mb/s      ~320 Mb/s
    decode('win1251')     ~95 Mb/s      ~246 Mb/s

## BOM handling

 * Decoding: BOM is stripped by default, unless overridden by passing `stripBOM: false` in options
   (f.ex. `iconv.decode(buf, enc, {stripBOM: false})`).
   A callback might also be given as a `stripBOM` parameter - it'll be called if BOM character was actually found.
 * If you want to detect UTF-8 BOM when decoding other encodings, use [node-autodetect-decoder-stream](https://github.com/danielgindi/node-autodetect-decoder-stream) module.
 * Encoding: No BOM added, unless overridden by `addBOM: true` option.

## UTF-16 Encodings

This library supports UTF-16LE, UTF-16BE and UTF-16 encodings. First two are straightforward, but UTF-16 is trying to be
smart about endianness in the following ways:
 * Decoding: uses BOM and 'spaces heuristic' to determine input endianness. Default is UTF-16LE, but can be 
   overridden with `defaultEncoding: 'utf-16be'` option. Strips BOM unless `stripBOM: false`.
 * Encoding: uses UTF-16LE and writes BOM by default. Use `addBOM: false` to override.

## Other notes

When decoding, be sure to supply a Buffer to decode() method, otherwise [bad things usually happen](https://github.com/ashtuchkin/iconv-lite/wiki/Use-Buffers-when-decoding).  
Untranslatable characters are set to � or ?. No transliteration is currently supported.  
Node versions 0.10.31 and 0.11.13 are buggy, don't use them (see #65, #77).  

## Testing

```bash
$ git clone git@github.com:ashtuchkin/iconv-lite.git
$ cd iconv-lite
$ npm install
$ npm test
    
$ # To view performance:
$ node test/performance.js

$ # To view test coverage:
$ npm run coverage
$ open coverage/lcov-report/index.html
```
```

### 文件: `node_modules/express-http-proxy/node_modules/raw-body/README.md`

```markdown
# raw-body

[![NPM Version][npm-image]][npm-url]
[![NPM Downloads][downloads-image]][downloads-url]
[![Node.js Version][node-version-image]][node-version-url]
[![Build status][github-actions-ci-image]][github-actions-ci-url]
[![Test coverage][coveralls-image]][coveralls-url]

Gets the entire buffer of a stream either as a `Buffer` or a string.
Validates the stream's length against an expected length and maximum limit.
Ideal for parsing request bodies.

## Install

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```sh
$ npm install raw-body
```

### TypeScript

This module includes a [TypeScript](https://www.typescriptlang.org/)
declaration file to enable auto complete in compatible editors and type
information for TypeScript projects. This module depends on the Node.js
types, so install `@types/node`:

```sh
$ npm install @types/node
```

## API

```js
var getRawBody = require('raw-body')
```

### getRawBody(stream, [options], [callback])

**Returns a promise if no callback specified and global `Promise` exists.**

Options:

- `length` - The length of the stream.
  If the contents of the stream do not add up to this length,
  an `400` error code is returned.
- `limit` - The byte limit of the body.
  This is the number of bytes or any string format supported by
  [bytes](https://www.npmjs.com/package/bytes),
  for example `1000`, `'500kb'` or `'3mb'`.
  If the body ends up being larger than this limit,
  a `413` error code is returned.
- `encoding` - The encoding to use to decode the body into a string.
  By default, a `Buffer` instance will be returned when no encoding is specified.
  Most likely, you want `utf-8`, so setting `encoding` to `true` will decode as `utf-8`.
  You can use any type of encoding supported by [iconv-lite](https://www.npmjs.org/package/iconv-lite#readme).

You can also pass a string in place of options to just specify the encoding.

If an error occurs, the stream will be paused, everything unpiped,
and you are responsible for correctly disposing the stream.
For HTTP requests, you may need to finish consuming the stream if
you want to keep the socket open for future requests. For streams
that use file descriptors, you should `stream.destroy()` or
`stream.close()` to prevent leaks.

## Errors

This module creates errors depending on the error condition during reading.
The error may be an error from the underlying Node.js implementation, but is
otherwise an error created by this module, which has the following attributes:

  * `limit` - the limit in bytes
  * `length` and `expected` - the expected length of the stream
  * `received` - the received bytes
  * `encoding` - the invalid encoding
  * `status` and `statusCode` - the corresponding status code for the error
  * `type` - the error type

### Types

The errors from this module have a `type` property which allows for the programmatic
determination of the type of error returned.

#### encoding.unsupported

This error will occur when the `encoding` option is specified, but the value does
not map to an encoding supported by the [iconv-lite](https://www.npmjs.org/package/iconv-lite#readme)
module.

#### entity.too.large

This error will occur when the `limit` option is specified, but the stream has
an entity that is larger.

#### request.aborted

This error will occur when the request stream is aborted by the client before
reading the body has finished.

#### request.size.invalid

This error will occur when the `length` option is specified, but the stream has
emitted more bytes.

#### stream.encoding.set

This error will occur when the given stream has an encoding set on it, making it
a decoded stream. The stream should not have an encoding set and is expected to
emit `Buffer` objects.

#### stream.not.readable

This error will occur when the given stream is not readable.

## Examples

### Simple Express example

```js
var contentType = require('content-type')
var express = require('express')
var getRawBody = require('raw-body')

var app = express()

app.use(function (req, res, next) {
  getRawBody(req, {
    length: req.headers['content-length'],
    limit: '1mb',
    encoding: contentType.parse(req).parameters.charset
  }, function (err, string) {
    if (err) return next(err)
    req.text = string
    next()
  })
})

// now access req.text
```

### Simple Koa example

```js
var contentType = require('content-type')
var getRawBody = require('raw-body')
var koa = require('koa')

var app = koa()

app.use(function * (next) {
  this.text = yield getRawBody(this.req, {
    length: this.req.headers['content-length'],
    limit: '1mb',
    encoding: contentType.parse(this.req).parameters.charset
  })
  yield next
})

// now access this.text
```

### Using as a promise

To use this library as a promise, simply omit the `callback` and a promise is
returned, provided that a global `Promise` is defined.

```js
var getRawBody = require('raw-body')
var http = require('http')

var server = http.createServer(function (req, res) {
  getRawBody(req)
    .then(function (buf) {
      res.statusCode = 200
      res.end(buf.length + ' bytes submitted')
    })
    .catch(function (err) {
      res.statusCode = 500
      res.end(err.message)
    })
})

server.listen(3000)
```

### Using with TypeScript

```ts
import * as getRawBody from 'raw-body';
import * as http from 'http';

const server = http.createServer((req, res) => {
  getRawBody(req)
  .then((buf) => {
    res.statusCode = 200;
    res.end(buf.length + ' bytes submitted');
  })
  .catch((err) => {
    res.statusCode = err.statusCode;
    res.end(err.message);
  });
});

server.listen(3000);
```

## License

[MIT](LICENSE)

[npm-image]: https://img.shields.io/npm/v/raw-body.svg
[npm-url]: https://npmjs.org/package/raw-body
[node-version-image]: https://img.shields.io/node/v/raw-body.svg
[node-version-url]: https://nodejs.org/en/download/
[coveralls-image]: https://img.shields.io/coveralls/stream-utils/raw-body/master.svg
[coveralls-url]: https://coveralls.io/r/stream-utils/raw-body?branch=master
[downloads-image]: https://img.shields.io/npm/dm/raw-body.svg
[downloads-url]: https://npmjs.org/package/raw-body
[github-actions-ci-image]: https://img.shields.io/github/actions/workflow/status/stream-utils/raw-body/ci.yml?branch=master&label=ci
[github-actions-ci-url]: https://github.com/jshttp/stream-utils/raw-body?query=workflow%3Aci
```

### 文件: `node_modules/function-bind/README.md`

```markdown
# function-bind <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
<!--[![coverage][codecov-image]][codecov-url]-->
[![dependency status][deps-svg]][deps-url]
[![dev dependency status][dev-deps-svg]][dev-deps-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Implementation of function.prototype.bind

Old versions of phantomjs, Internet Explorer < 9, and node < 0.6 don't support `Function.prototype.bind`.

## Example

```js
Function.prototype.bind = require("function-bind")
```

## Installation

`npm install function-bind`

## Contributors

 - Raynos

## MIT Licenced

[package-url]: https://npmjs.org/package/function-bind
[npm-version-svg]: https://versionbadg.es/Raynos/function-bind.svg
[deps-svg]: https://david-dm.org/Raynos/function-bind.svg
[deps-url]: https://david-dm.org/Raynos/function-bind
[dev-deps-svg]: https://david-dm.org/Raynos/function-bind/dev-status.svg
[dev-deps-url]: https://david-dm.org/Raynos/function-bind#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/function-bind.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/function-bind.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/function-bind.svg
[downloads-url]: https://npm-stat.com/charts.html?package=function-bind
[codecov-image]: https://codecov.io/gh/Raynos/function-bind/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/Raynos/function-bind/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/Raynos/function-bind
[actions-url]: https://github.com/Raynos/function-bind/actions
```

### 文件: `node_modules/get-intrinsic/README.md`

```markdown
# get-intrinsic <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![dependency status][deps-svg]][deps-url]
[![dev dependency status][dev-deps-svg]][dev-deps-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Get and robustly cache all JS language-level intrinsics at first require time.

See the syntax described [in the JS spec](https://tc39.es/ecma262/#sec-well-known-intrinsic-objects) for reference.

## Example

```js
var GetIntrinsic = require('get-intrinsic');
var assert = require('assert');

// static methods
assert.equal(GetIntrinsic('%Math.pow%'), Math.pow);
assert.equal(Math.pow(2, 3), 8);
assert.equal(GetIntrinsic('%Math.pow%')(2, 3), 8);
delete Math.pow;
assert.equal(GetIntrinsic('%Math.pow%')(2, 3), 8);

// instance methods
var arr = [1];
assert.equal(GetIntrinsic('%Array.prototype.push%'), Array.prototype.push);
assert.deepEqual(arr, [1]);

arr.push(2);
assert.deepEqual(arr, [1, 2]);

GetIntrinsic('%Array.prototype.push%').call(arr, 3);
assert.deepEqual(arr, [1, 2, 3]);

delete Array.prototype.push;
GetIntrinsic('%Array.prototype.push%').call(arr, 4);
assert.deepEqual(arr, [1, 2, 3, 4]);

// missing features
delete JSON.parse; // to simulate a real intrinsic that is missing in the environment
assert.throws(() => GetIntrinsic('%JSON.parse%'));
assert.equal(undefined, GetIntrinsic('%JSON.parse%', true));
```

## Tests
Simply clone the repo, `npm install`, and run `npm test`

## Security

Please email [@ljharb](https://github.com/ljharb) or see https://tidelift.com/security if you have a potential security vulnerability to report.

[package-url]: https://npmjs.org/package/get-intrinsic
[npm-version-svg]: https://versionbadg.es/ljharb/get-intrinsic.svg
[deps-svg]: https://david-dm.org/ljharb/get-intrinsic.svg
[deps-url]: https://david-dm.org/ljharb/get-intrinsic
[dev-deps-svg]: https://david-dm.org/ljharb/get-intrinsic/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/get-intrinsic#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/get-intrinsic.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/get-intrinsic.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/get-intrinsic.svg
[downloads-url]: https://npm-stat.com/charts.html?package=get-intrinsic
[codecov-image]: https://codecov.io/gh/ljharb/get-intrinsic/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/get-intrinsic/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/get-intrinsic
[actions-url]: https://github.com/ljharb/get-intrinsic/actions
```

### 文件: `node_modules/get-proto/README.md`

```markdown
# get-proto <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Robustly get the [[Prototype]] of an object. Uses the best available method.

## Getting started

```sh
npm install --save get-proto
```

## Usage/Examples

```js
const assert = require('assert');
const getProto = require('get-proto');

const a = { a: 1, b: 2, [Symbol.toStringTag]: 'foo' };
const b = { c: 3, __proto__: a };

assert.equal(getProto(b), a);
assert.equal(getProto(a), Object.prototype);
assert.equal(getProto({ __proto__: null }), null);
```

## Tests

Clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/get-proto
[npm-version-svg]: https://versionbadg.es/ljharb/get-proto.svg
[deps-svg]: https://david-dm.org/ljharb/get-proto.svg
[deps-url]: https://david-dm.org/ljharb/get-proto
[dev-deps-svg]: https://david-dm.org/ljharb/get-proto/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/get-proto#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/get-proto.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/get-proto.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/get-proto.svg
[downloads-url]: https://npm-stat.com/charts.html?package=get-proto
[codecov-image]: https://codecov.io/gh/ljharb/get-proto/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/get-proto/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/get-proto
[actions-url]: https://github.com/ljharb/get-proto/actions
```

### 文件: `node_modules/gopd/README.md`

```markdown
# gopd <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

`Object.getOwnPropertyDescriptor`, but accounts for IE's broken implementation.

## Usage

```javascript
var gOPD = require('gopd');
var assert = require('assert');

if (gOPD) {
	assert.equal(typeof gOPD, 'function', 'descriptors supported');
	// use gOPD like Object.getOwnPropertyDescriptor here
} else {
	assert.ok(!gOPD, 'descriptors not supported');
}
```

[package-url]: https://npmjs.org/package/gopd
[npm-version-svg]: https://versionbadg.es/ljharb/gopd.svg
[deps-svg]: https://david-dm.org/ljharb/gopd.svg
[deps-url]: https://david-dm.org/ljharb/gopd
[dev-deps-svg]: https://david-dm.org/ljharb/gopd/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/gopd#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/gopd.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/gopd.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/gopd.svg
[downloads-url]: https://npm-stat.com/charts.html?package=gopd
[codecov-image]: https://codecov.io/gh/ljharb/gopd/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/gopd/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/gopd
[actions-url]: https://github.com/ljharb/gopd/actions
```

### 文件: `node_modules/has-symbols/README.md`

```markdown
# has-symbols <sup>[![Version Badge][2]][1]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![dependency status][5]][6]
[![dev dependency status][7]][8]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][11]][1]

Determine if the JS environment has Symbol support. Supports spec, or shams.

## Example

```js
var hasSymbols = require('has-symbols');

hasSymbols() === true; // if the environment has native Symbol support. Not polyfillable, not forgeable.

var hasSymbolsKinda = require('has-symbols/shams');
hasSymbolsKinda() === true; // if the environment has a Symbol sham that mostly follows the spec.
```

## Supported Symbol shams
 - get-own-property-symbols [npm](https://www.npmjs.com/package/get-own-property-symbols) | [github](https://github.com/WebReflection/get-own-property-symbols)
 - core-js [npm](https://www.npmjs.com/package/core-js) | [github](https://github.com/zloirock/core-js)

## Tests
Simply clone the repo, `npm install`, and run `npm test`

[1]: https://npmjs.org/package/has-symbols
[2]: https://versionbadg.es/inspect-js/has-symbols.svg
[5]: https://david-dm.org/inspect-js/has-symbols.svg
[6]: https://david-dm.org/inspect-js/has-symbols
[7]: https://david-dm.org/inspect-js/has-symbols/dev-status.svg
[8]: https://david-dm.org/inspect-js/has-symbols#info=devDependencies
[11]: https://nodei.co/npm/has-symbols.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/has-symbols.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/has-symbols.svg
[downloads-url]: https://npm-stat.com/charts.html?package=has-symbols
[codecov-image]: https://codecov.io/gh/inspect-js/has-symbols/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/inspect-js/has-symbols/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/inspect-js/has-symbols
[actions-url]: https://github.com/inspect-js/has-symbols/actions
```

### 文件: `node_modules/hasown/README.md`

```markdown
# hasown <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

A robust, ES3 compatible, "has own property" predicate.

## Example

```js
const assert = require('assert');
const hasOwn = require('hasown');

assert.equal(hasOwn({}, 'toString'), false);
assert.equal(hasOwn([], 'length'), true);
assert.equal(hasOwn({ a: 42 }, 'a'), true);
```

## Tests
Simply clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/hasown
[npm-version-svg]: https://versionbadg.es/inspect-js/hasown.svg
[deps-svg]: https://david-dm.org/inspect-js/hasOwn.svg
[deps-url]: https://david-dm.org/inspect-js/hasOwn
[dev-deps-svg]: https://david-dm.org/inspect-js/hasOwn/dev-status.svg
[dev-deps-url]: https://david-dm.org/inspect-js/hasOwn#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/hasown.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/hasown.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/hasown.svg
[downloads-url]: https://npm-stat.com/charts.html?package=hasown
[codecov-image]: https://codecov.io/gh/inspect-js/hasOwn/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/inspect-js/hasOwn/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/inspect-js/hasOwn
[actions-url]: https://github.com/inspect-js/hasOwn/actions
```

### 文件: `node_modules/http-errors/README.md`

```markdown
# http-errors

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][node-url]
[![Node.js Version][node-image]][node-url]
[![Build Status][ci-image]][ci-url]
[![Test Coverage][coveralls-image]][coveralls-url]

Create HTTP errors for Express, Koa, Connect, etc. with ease.

## Install

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```console
$ npm install http-errors
```

## Example

```js
var createError = require('http-errors')
var express = require('express')
var app = express()

app.use(function (req, res, next) {
  if (!req.user) return next(createError(401, 'Please login to view this page.'))
  next()
})
```

## API

This is the current API, currently extracted from Koa and subject to change.

### Error Properties

- `expose` - can be used to signal if `message` should be sent to the client,
  defaulting to `false` when `status` >= 500
- `headers` - can be an object of header names to values to be sent to the
  client, defaulting to `undefined`. When defined, the key names should all
  be lower-cased
- `message` - the traditional error message, which should be kept short and all
  single line
- `status` - the status code of the error, mirroring `statusCode` for general
  compatibility
- `statusCode` - the status code of the error, defaulting to `500`

### createError([status], [message], [properties])

Create a new error object with the given message `msg`.
The error object inherits from `createError.HttpError`.

```js
var err = createError(404, 'This video does not exist!')
```

- `status: 500` - the status code as a number
- `message` - the message of the error, defaulting to node's text for that status code.
- `properties` - custom properties to attach to the object

### createError([status], [error], [properties])

Extend the given `error` object with `createError.HttpError`
properties. This will not alter the inheritance of the given
`error` object, and the modified `error` object is the
return value.

<!-- eslint-disable no-redeclare -->

```js
fs.readFile('foo.txt', function (err, buf) {
  if (err) {
    if (err.code === 'ENOENT') {
      var httpError = createError(404, err, { expose: false })
    } else {
      var httpError = createError(500, err)
    }
  }
})
```

- `status` - the status code as a number
- `error` - the error object to extend
- `properties` - custom properties to attach to the object

### createError.isHttpError(val)

Determine if the provided `val` is an `HttpError`. This will return `true`
if the error inherits from the `HttpError` constructor of this module or
matches the "duck type" for an error this module creates. All outputs from
the `createError` factory will return `true` for this function, including
if an non-`HttpError` was passed into the factory.

### new createError\[code || name\](\[msg]\))

Create a new error object with the given message `msg`.
The error object inherits from `createError.HttpError`.

```js
var err = new createError.NotFound()
```

- `code` - the status code as a number
- `name` - the name of the error as a "bumpy case", i.e. `NotFound` or `InternalServerError`.

#### List of all constructors

|Status Code|Constructor Name             |
|-----------|-----------------------------|
|400        |BadRequest                   |
|401        |Unauthorized                 |
|402        |PaymentRequired              |
|403        |Forbidden                    |
|404        |NotFound                     |
|405        |MethodNotAllowed             |
|406        |NotAcceptable                |
|407        |ProxyAuthenticationRequired  |
|408        |RequestTimeout               |
|409        |Conflict                     |
|410        |Gone                         |
|411        |LengthRequired               |
|412        |PreconditionFailed           |
|413        |PayloadTooLarge              |
|414        |URITooLong                   |
|415        |UnsupportedMediaType         |
|416        |RangeNotSatisfiable          |
|417        |ExpectationFailed            |
|418        |ImATeapot                    |
|421        |MisdirectedRequest           |
|422        |UnprocessableEntity          |
|423        |Locked                       |
|424        |FailedDependency             |
|425        |TooEarly                     |
|426        |UpgradeRequired              |
|428        |PreconditionRequired         |
|429        |TooManyRequests              |
|431        |RequestHeaderFieldsTooLarge  |
|451        |UnavailableForLegalReasons   |
|500        |InternalServerError          |
|501        |NotImplemented               |
|502        |BadGateway                   |
|503        |ServiceUnavailable           |
|504        |GatewayTimeout               |
|505        |HTTPVersionNotSupported      |
|506        |VariantAlsoNegotiates        |
|507        |InsufficientStorage          |
|508        |LoopDetected                 |
|509        |BandwidthLimitExceeded       |
|510        |NotExtended                  |
|511        |NetworkAuthenticationRequired|

## License

[MIT](LICENSE)

[ci-image]: https://badgen.net/github/checks/jshttp/http-errors/master?label=ci
[ci-url]: https://github.com/jshttp/http-errors/actions?query=workflow%3Aci
[coveralls-image]: https://badgen.net/coveralls/c/github/jshttp/http-errors/master
[coveralls-url]: https://coveralls.io/r/jshttp/http-errors?branch=master
[node-image]: https://badgen.net/npm/node/http-errors
[node-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/http-errors
[npm-url]: https://npmjs.org/package/http-errors
[npm-version-image]: https://badgen.net/npm/v/http-errors
[travis-image]: https://badgen.net/travis/jshttp/http-errors/master
[travis-url]: https://travis-ci.org/jshttp/http-errors
```

### 文件: `node_modules/iconv-lite/README.md`

```markdown
## iconv-lite: Pure JS character encoding conversion

 * No need for native code compilation. Quick to install, works on Windows and in sandboxed environments like [Cloud9](http://c9.io).
 * Used in popular projects like [Express.js (body_parser)](https://github.com/expressjs/body-parser), 
   [Grunt](http://gruntjs.com/), [Nodemailer](http://www.nodemailer.com/), [Yeoman](http://yeoman.io/) and others.
 * Faster than [node-iconv](https://github.com/bnoordhuis/node-iconv) (see below for performance comparison).
 * Intuitive encode/decode API, including Streaming support.
 * In-browser usage via [browserify](https://github.com/substack/node-browserify) or [webpack](https://webpack.js.org/) (~180kb gzip compressed with Buffer shim included).
 * Typescript [type definition file](https://github.com/ashtuchkin/iconv-lite/blob/master/lib/index.d.ts) included.
 * React Native is supported (need to install `stream` module to enable Streaming API).
 * License: MIT.

[![NPM Stats](https://nodei.co/npm/iconv-lite.png)](https://npmjs.org/package/iconv-lite/)  
[![Build Status](https://travis-ci.org/ashtuchkin/iconv-lite.svg?branch=master)](https://travis-ci.org/ashtuchkin/iconv-lite)
[![npm](https://img.shields.io/npm/v/iconv-lite.svg)](https://npmjs.org/package/iconv-lite/)
[![npm downloads](https://img.shields.io/npm/dm/iconv-lite.svg)](https://npmjs.org/package/iconv-lite/)
[![npm bundle size](https://img.shields.io/bundlephobia/min/iconv-lite.svg)](https://npmjs.org/package/iconv-lite/)

## Usage
### Basic API
```javascript
var iconv = require('iconv-lite');

// Convert from an encoded buffer to a js string.
str = iconv.decode(Buffer.from([0x68, 0x65, 0x6c, 0x6c, 0x6f]), 'win1251');

// Convert from a js string to an encoded buffer.
buf = iconv.encode("Sample input string", 'win1251');

// Check if encoding is supported
iconv.encodingExists("us-ascii")
```

### Streaming API
```javascript

// Decode stream (from binary data stream to js strings)
http.createServer(function(req, res) {
    var converterStream = iconv.decodeStream('win1251');
    req.pipe(converterStream);

    converterStream.on('data', function(str) {
        console.log(str); // Do something with decoded strings, chunk-by-chunk.
    });
});

// Convert encoding streaming example
fs.createReadStream('file-in-win1251.txt')
    .pipe(iconv.decodeStream('win1251'))
    .pipe(iconv.encodeStream('ucs2'))
    .pipe(fs.createWriteStream('file-in-ucs2.txt'));

// Sugar: all encode/decode streams have .collect(cb) method to accumulate data.
http.createServer(function(req, res) {
    req.pipe(iconv.decodeStream('win1251')).collect(function(err, body) {
        assert(typeof body == 'string');
        console.log(body); // full request body string
    });
});
```

## Supported encodings

 *  All node.js native encodings: utf8, ucs2 / utf16-le, ascii, binary, base64, hex.
 *  Additional unicode encodings: utf16, utf16-be, utf-7, utf-7-imap, utf32, utf32-le, and utf32-be.
 *  All widespread singlebyte encodings: Windows 125x family, ISO-8859 family, 
    IBM/DOS codepages, Macintosh family, KOI8 family, all others supported by iconv library. 
    Aliases like 'latin1', 'us-ascii' also supported.
 *  All widespread multibyte encodings: CP932, CP936, CP949, CP950, GB2312, GBK, GB18030, Big5, Shift_JIS, EUC-JP.

See [all supported encodings on wiki](https://github.com/ashtuchkin/iconv-lite/wiki/Supported-Encodings).

Most singlebyte encodings are generated automatically from [node-iconv](https://github.com/bnoordhuis/node-iconv). Thank you Ben Noordhuis and libiconv authors!

Multibyte encodings are generated from [Unicode.org mappings](http://www.unicode.org/Public/MAPPINGS/) and [WHATWG Encoding Standard mappings](http://encoding.spec.whatwg.org/). Thank you, respective authors!


## Encoding/decoding speed

Comparison with node-iconv module (1000x256kb, on MacBook Pro, Core i5/2.6 GHz, Node v0.12.0). 
Note: your results may vary, so please always check on your hardware.

    operation             iconv@2.1.4   iconv-lite@0.4.7
    ----------------------------------------------------------
    encode('win1251')     ~96 Mb/s      ~320 Mb/s
    decode('win1251')     ~95 Mb/s      ~246 Mb/s

## BOM handling

 * Decoding: BOM is stripped by default, unless overridden by passing `stripBOM: false` in options
   (f.ex. `iconv.decode(buf, enc, {stripBOM: false})`).
   A callback might also be given as a `stripBOM` parameter - it'll be called if BOM character was actually found.
 * If you want to detect UTF-8 BOM when decoding other encodings, use [node-autodetect-decoder-stream](https://github.com/danielgindi/node-autodetect-decoder-stream) module.
 * Encoding: No BOM added, unless overridden by `addBOM: true` option.

## UTF-16 Encodings

This library supports UTF-16LE, UTF-16BE and UTF-16 encodings. First two are straightforward, but UTF-16 is trying to be
smart about endianness in the following ways:
 * Decoding: uses BOM and 'spaces heuristic' to determine input endianness. Default is UTF-16LE, but can be 
   overridden with `defaultEncoding: 'utf-16be'` option. Strips BOM unless `stripBOM: false`.
 * Encoding: uses UTF-16LE and writes BOM by default. Use `addBOM: false` to override.

## UTF-32 Encodings

This library supports UTF-32LE, UTF-32BE and UTF-32 encodings. Like the UTF-16 encoding above, UTF-32 defaults to UTF-32LE, but uses BOM and 'spaces heuristics' to determine input endianness. 
 * The default of UTF-32LE can be overridden with the `defaultEncoding: 'utf-32be'` option. Strips BOM unless `stripBOM: false`.
 * Encoding: uses UTF-32LE and writes BOM by default. Use `addBOM: false` to override. (`defaultEncoding: 'utf-32be'` can also be used here to change encoding.)

## Other notes

When decoding, be sure to supply a Buffer to decode() method, otherwise [bad things usually happen](https://github.com/ashtuchkin/iconv-lite/wiki/Use-Buffers-when-decoding).  
Untranslatable characters are set to � or ?. No transliteration is currently supported.  
Node versions 0.10.31 and 0.11.13 are buggy, don't use them (see #65, #77).  

## Testing

```bash
$ git clone git@github.com:ashtuchkin/iconv-lite.git
$ cd iconv-lite
$ npm install
$ npm test
    
$ # To view performance:
$ node test/performance.js

$ # To view test coverage:
$ npm run coverage
$ open coverage/lcov-report/index.html
```
```

### 文件: `node_modules/inherits/README.md`

```markdown
Browser-friendly inheritance fully compatible with standard node.js
[inherits](http://nodejs.org/api/util.html#util_util_inherits_constructor_superconstructor).

This package exports standard `inherits` from node.js `util` module in
node environment, but also provides alternative browser-friendly
implementation through [browser
field](https://gist.github.com/shtylman/4339901). Alternative
implementation is a literal copy of standard one located in standalone
module to avoid requiring of `util`. It also has a shim for old
browsers with no `Object.create` support.

While keeping you sure you are using standard `inherits`
implementation in node.js environment, it allows bundlers such as
[browserify](https://github.com/substack/node-browserify) to not
include full `util` package to your client code if all you need is
just `inherits` function. It worth, because browser shim for `util`
package is large and `inherits` is often the single function you need
from it.

It's recommended to use this package instead of
`require('util').inherits` for any code that has chances to be used
not only in node.js but in browser too.

## usage

```js
var inherits = require('inherits');
// then use exactly as the standard one
```

## note on version ~1.0

Version ~1.0 had completely different motivation and is not compatible
neither with 2.0 nor with standard node.js `inherits`.

If you are using version ~1.0 and planning to switch to ~2.0, be
careful:

* new version uses `super_` instead of `super` for referencing
  superclass
* new version overwrites current prototype while old one preserves any
  existing fields on it
```

### 文件: `node_modules/math-intrinsics/README.md`

```markdown
# math-intrinsics <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

ES Math-related intrinsics and helpers, robustly cached.

 - `abs`
 - `floor`
 - `isFinite`
 - `isInteger`
 - `isNaN`
 - `isNegativeZero`
 - `max`
 - `min`
 - `mod`
 - `pow`
 - `round`
 - `sign`
 - `constants/maxArrayLength`
 - `constants/maxSafeInteger`
 - `constants/maxValue`


## Tests
Simply clone the repo, `npm install`, and run `npm test`

## Security

Please email [@ljharb](https://github.com/ljharb) or see https://tidelift.com/security if you have a potential security vulnerability to report.

[package-url]: https://npmjs.org/package/math-intrinsics
[npm-version-svg]: https://versionbadg.es/es-shims/math-intrinsics.svg
[deps-svg]: https://david-dm.org/es-shims/math-intrinsics.svg
[deps-url]: https://david-dm.org/es-shims/math-intrinsics
[dev-deps-svg]: https://david-dm.org/es-shims/math-intrinsics/dev-status.svg
[dev-deps-url]: https://david-dm.org/es-shims/math-intrinsics#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/math-intrinsics.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/math-intrinsics.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/es-object.svg
[downloads-url]: https://npm-stat.com/charts.html?package=math-intrinsics
[codecov-image]: https://codecov.io/gh/es-shims/math-intrinsics/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/es-shims/math-intrinsics/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/es-shims/math-intrinsics
[actions-url]: https://github.com/es-shims/math-intrinsics/actions
```

### 文件: `node_modules/media-typer/README.md`

```markdown
# media-typer

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Node.js Version][node-version-image]][node-version-url]
[![Build Status][travis-image]][travis-url]
[![Test Coverage][coveralls-image]][coveralls-url]

Simple RFC 6838 media type parser.

This module will parse a given media type into it's component parts, like type,
subtype, and suffix. A formatter is also provided to put them back together and
the two can be combined to normalize media types into a canonical form.

If you are looking to parse the string that represents a media type and it's
parameters in HTTP (for example, the `Content-Type` header), use the
[content-type module](https://www.npmjs.com/package/content-type).

## Installation

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```sh
$ npm install media-typer
```

## API

<!-- eslint-disable no-unused-vars -->

```js
var typer = require('media-typer')
```

### typer.parse(string)

<!-- eslint-disable no-undef, no-unused-vars -->

```js
var obj = typer.parse('image/svg+xml')
```

Parse a media type string. This will return an object with the following
properties (examples are shown for the string `'image/svg+xml; charset=utf-8'`):

 - `type`: The type of the media type (always lower case). Example: `'image'`

 - `subtype`: The subtype of the media type (always lower case). Example: `'svg'`

 - `suffix`: The suffix of the media type (always lower case). Example: `'xml'`

If the given type string is invalid, then a `TypeError` is thrown.

### typer.format(obj)

<!-- eslint-disable no-undef, no-unused-vars -->

```js
var obj = typer.format({ type: 'image', subtype: 'svg', suffix: 'xml' })
```

Format an object into a media type string. This will return a string of the
mime type for the given object. For the properties of the object, see the
documentation for `typer.parse(string)`.

If any of the given object values are invalid, then a `TypeError` is thrown.

### typer.test(string)

<!-- eslint-disable no-undef, no-unused-vars -->

```js
var valid = typer.test('image/svg+xml')
```

Validate a media type string. This will return `true` is the string is a well-
formatted media type, or `false` otherwise.

## License

[MIT](LICENSE)

[coveralls-image]: https://badgen.net/coveralls/c/github/jshttp/media-typer/master
[coveralls-url]: https://coveralls.io/r/jshttp/media-typer?branch=master
[node-version-image]: https://badgen.net/npm/node/media-typer
[node-version-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/media-typer
[npm-url]: https://npmjs.org/package/media-typer
[npm-version-image]: https://badgen.net/npm/v/media-typer
[travis-image]: https://badgen.net/travis/jshttp/media-typer/master
[travis-url]: https://travis-ci.org/jshttp/media-typer
```

### 文件: `node_modules/mime-db/README.md`

```markdown
# mime-db

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Node.js Version][node-image]][node-url]
[![Build Status][ci-image]][ci-url]
[![Coverage Status][coveralls-image]][coveralls-url]

This is a large database of mime types and information about them.
It consists of a single, public JSON file and does not include any logic,
allowing it to remain as un-opinionated as possible with an API.
It aggregates data from the following sources:

- https://www.iana.org/assignments/media-types/media-types.xhtml
- https://svn.apache.org/repos/asf/httpd/httpd/trunk/docs/conf/mime.types
- https://hg.nginx.org/nginx/raw-file/default/conf/mime.types

## Installation

```bash
npm install mime-db
```

### Database Download

If you intend to use this in a web browser, you can conveniently access the JSON file via [jsDelivr](https://www.jsdelivr.com/), a popular CDN (Content Delivery Network). To ensure stability and compatibility, it is advisable to specify [a release tag](https://github.com/jshttp/mime-db/tags) instead of using the 'master' branch. This is because the JSON file's format might change in future updates, and relying on a specific release tag will prevent potential issues arising from these changes.

```
https://cdn.jsdelivr.net/gh/jshttp/mime-db@master/db.json
```

## Usage

```js
var db = require('mime-db')

// grab data on .js files
var data = db['application/javascript']
```

## Data Structure

The JSON file is a map lookup for lowercased mime types.
Each mime type has the following properties:

- `.source` - where the mime type is defined.
    If not set, it's probably a custom media type.
    - `apache` - [Apache common media types](https://svn.apache.org/repos/asf/httpd/httpd/trunk/docs/conf/mime.types)
    - `iana` - [IANA-defined media types](https://www.iana.org/assignments/media-types/media-types.xhtml)
    - `nginx` - [nginx media types](https://hg.nginx.org/nginx/raw-file/default/conf/mime.types)
- `.extensions[]` - known extensions associated with this mime type.
- `.compressible` - whether a file of this type can be gzipped.
- `.charset` - the default charset associated with this type, if any.

If unknown, every property could be `undefined`.

## Note on MIME Type Data and Semver

This package considers the programmatic api as the semver compatibility. This means the MIME type resolution is *not* considered
in the semver bumps. This means that if you want to pin your `mime-db` data you will need to do it in your application. While
this expectation was not set in docs until now, it is how the pacakge operated, so we do not feel this is a breaking change.

## Contributing

The primary way to contribute to this database is by updating the data in
one of the upstream sources. The database is updated from the upstreams
periodically and will pull in any changes.

### Registering Media Types

The best way to get new media types included in this library is to register
them with the IANA. The community registration procedure is outlined in
[RFC 6838 section 5](https://tools.ietf.org/html/rfc6838#section-5). Types
registered with the IANA are automatically pulled into this library.

### Direct Inclusion

If that is not possible / feasible, they can be added directly here as a
"custom" type. To do this, it is required to have a primary source that
definitively lists the media type. If an extension is going to be listed as
associated with this media type, the source must definitively link the
media type and extension as well.

To edit the database, only make PRs against `src/custom-types.json` or
`src/custom-suffix.json`.

The `src/custom-types.json` file is a JSON object with the MIME type as the
keys and the values being an object with the following keys:

- `compressible` - leave out if you don't know, otherwise `true`/`false` to
  indicate whether the data represented by the type is typically compressible.
- `extensions` - include an array of file extensions that are associated with
  the type.
- `notes` - human-readable notes about the type, typically what the type is.
- `sources` - include an array of URLs of where the MIME type and the associated
  extensions are sourced from. This needs to be a [primary source](https://en.wikipedia.org/wiki/Primary_source);
  links to type aggregating sites and Wikipedia are _not acceptable_.

To update the build, run `npm run build`.

[ci-image]: https://badgen.net/github/checks/jshttp/mime-db/master?label=ci
[ci-url]: https://github.com/jshttp/mime-db/actions/workflows/ci.yml
[coveralls-image]: https://badgen.net/coveralls/c/github/jshttp/mime-db/master
[coveralls-url]: https://coveralls.io/r/jshttp/mime-db?branch=master
[node-image]: https://badgen.net/npm/node/mime-db
[node-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/mime-db
[npm-url]: https://npmjs.org/package/mime-db
[npm-version-image]: https://badgen.net/npm/v/mime-db
```

### 文件: `node_modules/mime-types/README.md`

```markdown
# mime-types

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Node.js Version][node-version-image]][node-version-url]
[![Build Status][ci-image]][ci-url]
[![Test Coverage][coveralls-image]][coveralls-url]

The ultimate javascript content-type utility.

Similar to [the `mime@1.x` module](https://www.npmjs.com/package/mime), except:

- __No fallbacks.__ Instead of naively returning the first available type,
  `mime-types` simply returns `false`, so do
  `var type = mime.lookup('unrecognized') || 'application/octet-stream'`.
- No `new Mime()` business, so you could do `var lookup = require('mime-types').lookup`.
- No `.define()` functionality
- Bug fixes for `.lookup(path)`

Otherwise, the API is compatible with `mime` 1.x.

## Install

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```sh
$ npm install mime-types
```

## Note on MIME Type Data and Semver

This package considers the programmatic api as the semver compatibility. Additionally, the package which provides the MIME data
for this package (`mime-db`) *also* considers it's programmatic api as the semver contract. This means the MIME type resolution is *not* considered
in the semver bumps.

In the past the version of `mime-db` was pinned to give two decision points when adopting MIME data changes. This is no longer true. We still update the
`mime-db` package here as a `minor` release when necessary, but will use a `^` range going forward. This means that if you want to pin your `mime-db` data
you will need to do it in your application. While this expectation was not set in docs until now, it is how the pacakge operated, so we do not feel this is
a breaking change.

If you wish to pin your `mime-db` version you can do that with overrides via your package manager of choice. See their documentation for how to correctly configure that.

## Adding Types

All mime types are based on [mime-db](https://www.npmjs.com/package/mime-db),
so open a PR there if you'd like to add mime types.

## API

```js
var mime = require('mime-types')
```

All functions return `false` if input is invalid or not found.

### mime.lookup(path)

Lookup the content-type associated with a file.

```js
mime.lookup('json') // 'application/json'
mime.lookup('.md') // 'text/markdown'
mime.lookup('file.html') // 'text/html'
mime.lookup('folder/file.js') // 'application/javascript'
mime.lookup('folder/.htaccess') // false

mime.lookup('cats') // false
```

### mime.contentType(type)

Create a full content-type header given a content-type or extension.
When given an extension, `mime.lookup` is used to get the matching
content-type, otherwise the given content-type is used. Then if the
content-type does not already have a `charset` parameter, `mime.charset`
is used to get the default charset and add to the returned content-type.

```js
mime.contentType('markdown') // 'text/x-markdown; charset=utf-8'
mime.contentType('file.json') // 'application/json; charset=utf-8'
mime.contentType('text/html') // 'text/html; charset=utf-8'
mime.contentType('text/html; charset=iso-8859-1') // 'text/html; charset=iso-8859-1'

// from a full path
mime.contentType(path.extname('/path/to/file.json')) // 'application/json; charset=utf-8'
```

### mime.extension(type)

Get the default extension for a content-type.

```js
mime.extension('application/octet-stream') // 'bin'
```

### mime.charset(type)

Lookup the implied default charset of a content-type.

```js
mime.charset('text/markdown') // 'UTF-8'
```

### var type = mime.types[extension]

A map of content-types by extension.

### [extensions...] = mime.extensions[type]

A map of extensions by content-type.

## License

[MIT](LICENSE)

[ci-image]: https://badgen.net/github/checks/jshttp/mime-types/master?label=ci
[ci-url]: https://github.com/jshttp/mime-types/actions/workflows/ci.yml
[coveralls-image]: https://badgen.net/coveralls/c/github/jshttp/mime-types/master
[coveralls-url]: https://coveralls.io/r/jshttp/mime-types?branch=master
[node-version-image]: https://badgen.net/npm/node/mime-types
[node-version-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/mime-types
[npm-url]: https://npmjs.org/package/mime-types
[npm-version-image]: https://badgen.net/npm/v/mime-types
```

### 文件: `node_modules/ms/readme.md`

```markdown
# ms

![CI](https://github.com/vercel/ms/workflows/CI/badge.svg)

Use this package to easily convert various time formats to milliseconds.

## Examples

```js
ms('2 days')  // 172800000
ms('1d')      // 86400000
ms('10h')     // 36000000
ms('2.5 hrs') // 9000000
ms('2h')      // 7200000
ms('1m')      // 60000
ms('5s')      // 5000
ms('1y')      // 31557600000
ms('100')     // 100
ms('-3 days') // -259200000
ms('-1h')     // -3600000
ms('-200')    // -200
```

### Convert from Milliseconds

```js
ms(60000)             // "1m"
ms(2 * 60000)         // "2m"
ms(-3 * 60000)        // "-3m"
ms(ms('10 hours'))    // "10h"
```

### Time Format Written-Out

```js
ms(60000, { long: true })             // "1 minute"
ms(2 * 60000, { long: true })         // "2 minutes"
ms(-3 * 60000, { long: true })        // "-3 minutes"
ms(ms('10 hours'), { long: true })    // "10 hours"
```

## Features

- Works both in [Node.js](https://nodejs.org) and in the browser
- If a number is supplied to `ms`, a string with a unit is returned
- If a string that contains the number is supplied, it returns it as a number (e.g.: it returns `100` for `'100'`)
- If you pass a string with a number and a valid unit, the number of equivalent milliseconds is returned

## Related Packages

- [ms.macro](https://github.com/knpwrs/ms.macro) - Run `ms` as a macro at build-time.

## Caught a Bug?

1. [Fork](https://help.github.com/articles/fork-a-repo/) this repository to your own GitHub account and then [clone](https://help.github.com/articles/cloning-a-repository/) it to your local device
2. Link the package to the global module directory: `npm link`
3. Within the module you want to test your local development instance of ms, just link it to the dependencies: `npm link ms`. Instead of the default one from npm, Node.js will now use your clone of ms!

As always, you can run the tests using: `npm test`
```

### 文件: `node_modules/on-finished/README.md`

```markdown
# on-finished

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Node.js Version][node-image]][node-url]
[![Build Status][ci-image]][ci-url]
[![Coverage Status][coveralls-image]][coveralls-url]

Execute a callback when a HTTP request closes, finishes, or errors.

## Install

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```sh
$ npm install on-finished
```

## API

```js
var onFinished = require('on-finished')
```

### onFinished(res, listener)

Attach a listener to listen for the response to finish. The listener will
be invoked only once when the response finished. If the response finished
to an error, the first argument will contain the error. If the response
has already finished, the listener will be invoked.

Listening to the end of a response would be used to close things associated
with the response, like open files.

Listener is invoked as `listener(err, res)`.

<!-- eslint-disable handle-callback-err -->

```js
onFinished(res, function (err, res) {
  // clean up open fds, etc.
  // err contains the error if request error'd
})
```

### onFinished(req, listener)

Attach a listener to listen for the request to finish. The listener will
be invoked only once when the request finished. If the request finished
to an error, the first argument will contain the error. If the request
has already finished, the listener will be invoked.

Listening to the end of a request would be used to know when to continue
after reading the data.

Listener is invoked as `listener(err, req)`.

<!-- eslint-disable handle-callback-err -->

```js
var data = ''

req.setEncoding('utf8')
req.on('data', function (str) {
  data += str
})

onFinished(req, function (err, req) {
  // data is read unless there is err
})
```

### onFinished.isFinished(res)

Determine if `res` is already finished. This would be useful to check and
not even start certain operations if the response has already finished.

### onFinished.isFinished(req)

Determine if `req` is already finished. This would be useful to check and
not even start certain operations if the request has already finished.

## Special Node.js requests

### HTTP CONNECT method

The meaning of the `CONNECT` method from RFC 7231, section 4.3.6:

> The CONNECT method requests that the recipient establish a tunnel to
> the destination origin server identified by the request-target and,
> if successful, thereafter restrict its behavior to blind forwarding
> of packets, in both directions, until the tunnel is closed.  Tunnels
> are commonly used to create an end-to-end virtual connection, through
> one or more proxies, which can then be secured using TLS (Transport
> Layer Security, [RFC5246]).

In Node.js, these request objects come from the `'connect'` event on
the HTTP server.

When this module is used on a HTTP `CONNECT` request, the request is
considered "finished" immediately, **due to limitations in the Node.js
interface**. This means if the `CONNECT` request contains a request entity,
the request will be considered "finished" even before it has been read.

There is no such thing as a response object to a `CONNECT` request in
Node.js, so there is no support for one.

### HTTP Upgrade request

The meaning of the `Upgrade` header from RFC 7230, section 6.1:

> The "Upgrade" header field is intended to provide a simple mechanism
> for transitioning from HTTP/1.1 to some other protocol on the same
> connection.

In Node.js, these request objects come from the `'upgrade'` event on
the HTTP server.

When this module is used on a HTTP request with an `Upgrade` header, the
request is considered "finished" immediately, **due to limitations in the
Node.js interface**. This means if the `Upgrade` request contains a request
entity, the request will be considered "finished" even before it has been
read.

There is no such thing as a response object to a `Upgrade` request in
Node.js, so there is no support for one.

## Example

The following code ensures that file descriptors are always closed
once the response finishes.

```js
var destroy = require('destroy')
var fs = require('fs')
var http = require('http')
var onFinished = require('on-finished')

http.createServer(function onRequest (req, res) {
  var stream = fs.createReadStream('package.json')
  stream.pipe(res)
  onFinished(res, function () {
    destroy(stream)
  })
})
```

## License

[MIT](LICENSE)

[ci-image]: https://badgen.net/github/checks/jshttp/on-finished/master?label=ci
[ci-url]: https://github.com/jshttp/on-finished/actions/workflows/ci.yml
[coveralls-image]: https://badgen.net/coveralls/c/github/jshttp/on-finished/master
[coveralls-url]: https://coveralls.io/r/jshttp/on-finished?branch=master
[node-image]: https://badgen.net/npm/node/on-finished
[node-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/on-finished
[npm-url]: https://npmjs.org/package/on-finished
[npm-version-image]: https://badgen.net/npm/v/on-finished
```

### 文件: `node_modules/qs/README.md`

```markdown
<p align="center">
    <img alt="qs" src="./logos/banner_default.png" width="800" />
</p>

# qs <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]
[![CII Best Practices](https://bestpractices.coreinfrastructure.org/projects/9058/badge)](https://bestpractices.coreinfrastructure.org/projects/9058)

[![npm badge][npm-badge-png]][package-url]

A querystring parsing and stringifying library with some added security.

Lead Maintainer: [Jordan Harband](https://github.com/ljharb)

The **qs** module was originally created and maintained by [TJ Holowaychuk](https://github.com/visionmedia/node-querystring).

## Usage

```javascript
var qs = require('qs');
var assert = require('assert');

var obj = qs.parse('a=c');
assert.deepEqual(obj, { a: 'c' });

var str = qs.stringify(obj);
assert.equal(str, 'a=c');
```

### Parsing Objects

[](#preventEval)
```javascript
qs.parse(string, [options]);
```

**qs** allows you to create nested objects within your query strings, by surrounding the name of sub-keys with square brackets `[]`.
For example, the string `'foo[bar]=baz'` converts to:

```javascript
assert.deepEqual(qs.parse('foo[bar]=baz'), {
    foo: {
        bar: 'baz'
    }
});
```

When using the `plainObjects` option the parsed value is returned as a null object, created via `{ __proto__: null }` and as such you should be aware that prototype methods will not exist on it and a user may set those names to whatever value they like:

```javascript
var nullObject = qs.parse('a[hasOwnProperty]=b', { plainObjects: true });
assert.deepEqual(nullObject, { a: { hasOwnProperty: 'b' } });
```

By default parameters that would overwrite properties on the object prototype are ignored, if you wish to keep the data from those fields either use `plainObjects` as mentioned above, or set `allowPrototypes` to `true` which will allow user input to overwrite those properties.
*WARNING* It is generally a bad idea to enable this option as it can cause problems when attempting to use the properties that have been overwritten.
Always be careful with this option.

```javascript
var protoObject = qs.parse('a[hasOwnProperty]=b', { allowPrototypes: true });
assert.deepEqual(protoObject, { a: { hasOwnProperty: 'b' } });
```

URI encoded strings work too:

```javascript
assert.deepEqual(qs.parse('a%5Bb%5D=c'), {
    a: { b: 'c' }
});
```

You can also nest your objects, like `'foo[bar][baz]=foobarbaz'`:

```javascript
assert.deepEqual(qs.parse('foo[bar][baz]=foobarbaz'), {
    foo: {
        bar: {
            baz: 'foobarbaz'
        }
    }
});
```

By default, when nesting objects **qs** will only parse up to 5 children deep.
This means if you attempt to parse a string like `'a[b][c][d][e][f][g][h][i]=j'` your resulting object will be:

```javascript
var expected = {
    a: {
        b: {
            c: {
                d: {
                    e: {
                        f: {
                            '[g][h][i]': 'j'
                        }
                    }
                }
            }
        }
    }
};
var string = 'a[b][c][d][e][f][g][h][i]=j';
assert.deepEqual(qs.parse(string), expected);
```

This depth can be overridden by passing a `depth` option to `qs.parse(string, [options])`:

```javascript
var deep = qs.parse('a[b][c][d][e][f][g][h][i]=j', { depth: 1 });
assert.deepEqual(deep, { a: { b: { '[c][d][e][f][g][h][i]': 'j' } } });
```

You can configure **qs** to throw an error when parsing nested input beyond this depth using the `strictDepth` option (defaulted to false):

```javascript
try {
    qs.parse('a[b][c][d][e][f][g][h][i]=j', { depth: 1, strictDepth: true });
} catch (err) {
    assert(err instanceof RangeError);
    assert.strictEqual(err.message, 'Input depth exceeded depth option of 1 and strictDepth is true');
}
```

The depth limit helps mitigate abuse when **qs** is used to parse user input, and it is recommended to keep it a reasonably small number. The strictDepth option adds a layer of protection by throwing an error when the limit is exceeded, allowing you to catch and handle such cases.

For similar reasons, by default **qs** will only parse up to 1000 parameters. This can be overridden by passing a `parameterLimit` option:

```javascript
var limited = qs.parse('a=b&c=d', { parameterLimit: 1 });
assert.deepEqual(limited, { a: 'b' });
```

If you want an error to be thrown whenever the a limit is exceeded (eg, `parameterLimit`, `arrayLimit`), set the `throwOnLimitExceeded` option to `true`. This option will generate a descriptive error if the query string exceeds a configured limit.
```javascript
try {
    qs.parse('a=1&b=2&c=3&d=4', { parameterLimit: 3, throwOnLimitExceeded: true });
} catch (err) {
    assert(err instanceof Error);
    assert.strictEqual(err.message, 'Parameter limit exceeded. Only 3 parameters allowed.');
}
```

When `throwOnLimitExceeded` is set to `false` (default), **qs** will parse up to the specified `parameterLimit` and ignore the rest without throwing an error.

To bypass the leading question mark, use `ignoreQueryPrefix`:

```javascript
var prefixed = qs.parse('?a=b&c=d', { ignoreQueryPrefix: true });
assert.deepEqual(prefixed, { a: 'b', c: 'd' });
```

An optional delimiter can also be passed:

```javascript
var delimited = qs.parse('a=b;c=d', { delimiter: ';' });
assert.deepEqual(delimited, { a: 'b', c: 'd' });
```

Delimiters can be a regular expression too:

```javascript
var regexed = qs.parse('a=b;c=d,e=f', { delimiter: /[;,]/ });
assert.deepEqual(regexed, { a: 'b', c: 'd', e: 'f' });
```

Option `allowDots` can be used to enable dot notation:

```javascript
var withDots = qs.parse('a.b=c', { allowDots: true });
assert.deepEqual(withDots, { a: { b: 'c' } });
```

Option `decodeDotInKeys` can be used to decode dots in keys
Note: it implies `allowDots`, so `parse` will error if you set `decodeDotInKeys` to `true`, and `allowDots` to `false`.

```javascript
var withDots = qs.parse('name%252Eobj.first=John&name%252Eobj.last=Doe', { decodeDotInKeys: true });
assert.deepEqual(withDots, { 'name.obj': { first: 'John', last: 'Doe' }});
```

Option `allowEmptyArrays` can be used to allowing empty array values in object
```javascript
var withEmptyArrays = qs.parse('foo[]&bar=baz', { allowEmptyArrays: true });
assert.deepEqual(withEmptyArrays, { foo: [], bar: 'baz' });
```

Option `duplicates` can be used to change the behavior when duplicate keys are encountered
```javascript
assert.deepEqual(qs.parse('foo=bar&foo=baz'), { foo: ['bar', 'baz'] });
assert.deepEqual(qs.parse('foo=bar&foo=baz', { duplicates: 'combine' }), { foo: ['bar', 'baz'] });
assert.deepEqual(qs.parse('foo=bar&foo=baz', { duplicates: 'first' }), { foo: 'bar' });
assert.deepEqual(qs.parse('foo=bar&foo=baz', { duplicates: 'last' }), { foo: 'baz' });
```

If you have to deal with legacy browsers or services, there's also support for decoding percent-encoded octets as iso-8859-1:

```javascript
var oldCharset = qs.parse('a=%A7', { charset: 'iso-8859-1' });
assert.deepEqual(oldCharset, { a: '§' });
```

Some services add an initial `utf8=✓` value to forms so that old Internet Explorer versions are more likely to submit the form as utf-8.
Additionally, the server can check the value against wrong encodings of the checkmark character and detect that a query string or `application/x-www-form-urlencoded` body was *not* sent as utf-8, eg. if the form had an `accept-charset` parameter or the containing page had a different character set.

**qs** supports this mechanism via the `charsetSentinel` option.
If specified, the `utf8` parameter will be omitted from the returned object.
It will be used to switch to `iso-8859-1`/`utf-8` mode depending on how the checkmark is encoded.

**Important**: When you specify both the `charset` option and the `charsetSentinel` option, the `charset` will be overridden when the request contains a `utf8` parameter from which the actual charset can be deduced.
In that sense the `charset` will behave as the default charset rather than the authoritative charset.

```javascript
var detectedAsUtf8 = qs.parse('utf8=%E2%9C%93&a=%C3%B8', {
    charset: 'iso-8859-1',
    charsetSentinel: true
});
assert.deepEqual(detectedAsUtf8, { a: 'ø' });

// Browsers encode the checkmark as &#10003; when submitting as iso-8859-1:
var detectedAsIso8859_1 = qs.parse('utf8=%26%2310003%3B&a=%F8', {
    charset: 'utf-8',
    charsetSentinel: true
});
assert.deepEqual(detectedAsIso8859_1, { a: 'ø' });
```

If you want to decode the `&#...;` syntax to the actual character, you can specify the `interpretNumericEntities` option as well:

```javascript
var detectedAsIso8859_1 = qs.parse('a=%26%239786%3B', {
    charset: 'iso-8859-1',
    interpretNumericEntities: true
});
assert.deepEqual(detectedAsIso8859_1, { a: '☺' });
```

It also works when the charset has been detected in `charsetSentinel` mode.

### Parsing Arrays

**qs** can also parse arrays using a similar `[]` notation:

```javascript
var withArray = qs.parse('a[]=b&a[]=c');
assert.deepEqual(withArray, { a: ['b', 'c'] });
```

You may specify an index as well:

```javascript
var withIndexes = qs.parse('a[1]=c&a[0]=b');
assert.deepEqual(withIndexes, { a: ['b', 'c'] });
```

Note that the only difference between an index in an array and a key in an object is that the value between the brackets must be a number to create an array.
When creating arrays with specific indices, **qs** will compact a sparse array to only the existing values preserving their order:

```javascript
var noSparse = qs.parse('a[1]=b&a[15]=c');
assert.deepEqual(noSparse, { a: ['b', 'c'] });
```

You may also use `allowSparse` option to parse sparse arrays:

```javascript
var sparseArray = qs.parse('a[1]=2&a[3]=5', { allowSparse: true });
assert.deepEqual(sparseArray, { a: [, '2', , '5'] });
```

Note that an empty string is also a value, and will be preserved:

```javascript
var withEmptyString = qs.parse('a[]=&a[]=b');
assert.deepEqual(withEmptyString, { a: ['', 'b'] });

var withIndexedEmptyString = qs.parse('a[0]=b&a[1]=&a[2]=c');
assert.deepEqual(withIndexedEmptyString, { a: ['b', '', 'c'] });
```

**qs** will also limit specifying indices in an array to a maximum index of `20`.
Any array members with an index of greater than `20` will instead be converted to an object with the index as the key.
This is needed to handle cases when someone sent, for example, `a[999999999]` and it will take significant time to iterate over this huge array.

```javascript
var withMaxIndex = qs.parse('a[100]=b');
assert.deepEqual(withMaxIndex, { a: { '100': 'b' } });
```

This limit can be overridden by passing an `arrayLimit` option:

```javascript
var withArrayLimit = qs.parse('a[1]=b', { arrayLimit: 0 });
assert.deepEqual(withArrayLimit, { a: { '1': 'b' } });
```

If you want to throw an error whenever the array limit is exceeded, set the `throwOnLimitExceeded` option to `true`. This option will generate a descriptive error if the query string exceeds a configured limit.
```javascript
try {
    qs.parse('a[1]=b', { arrayLimit: 0, throwOnLimitExceeded: true });
} catch (err) {
    assert(err instanceof Error);
    assert.strictEqual(err.message, 'Array limit exceeded. Only 0 elements allowed in an array.');
}
```

When `throwOnLimitExceeded` is set to `false` (default), **qs** will parse up to the specified `arrayLimit` and if the limit is exceeded, the array will instead be converted to an object with the index as the key

To disable array parsing entirely, set `parseArrays` to `false`.

```javascript
var noParsingArrays = qs.parse('a[]=b', { parseArrays: false });
assert.deepEqual(noParsingArrays, { a: { '0': 'b' } });
```

If you mix notations, **qs** will merge the two items into an object:

```javascript
var mixedNotation = qs.parse('a[0]=b&a[b]=c');
assert.deepEqual(mixedNotation, { a: { '0': 'b', b: 'c' } });
```

You can also create arrays of objects:

```javascript
var arraysOfObjects = qs.parse('a[][b]=c');
assert.deepEqual(arraysOfObjects, { a: [{ b: 'c' }] });
```

Some people use comma to join array, **qs** can parse it:
```javascript
var arraysOfObjects = qs.parse('a=b,c', { comma: true })
assert.deepEqual(arraysOfObjects, { a: ['b', 'c'] })
```
(_this cannot convert nested objects, such as `a={b:1},{c:d}`_)

### Parsing primitive/scalar values (numbers, booleans, null, etc)

By default, all values are parsed as strings.
This behavior will not change and is explained in [issue #91](https://github.com/ljharb/qs/issues/91).

```javascript
var primitiveValues = qs.parse('a=15&b=true&c=null');
assert.deepEqual(primitiveValues, { a: '15', b: 'true', c: 'null' });
```

If you wish to auto-convert values which look like numbers, booleans, and other values into their primitive counterparts, you can use the [query-types Express JS middleware](https://github.com/xpepermint/query-types) which will auto-convert all request query parameters.

### Stringifying

[](#preventEval)
```javascript
qs.stringify(object, [options]);
```

When stringifying, **qs** by default URI encodes output. Objects are stringified as you would expect:

```javascript
assert.equal(qs.stringify({ a: 'b' }), 'a=b');
assert.equal(qs.stringify({ a: { b: 'c' } }), 'a%5Bb%5D=c');
```

This encoding can be disabled by setting the `encode` option to `false`:

```javascript
var unencoded = qs.stringify({ a: { b: 'c' } }, { encode: false });
assert.equal(unencoded, 'a[b]=c');
```

Encoding can be disabled for keys by setting the `encodeValuesOnly` option to `true`:
```javascript
var encodedValues = qs.stringify(
    { a: 'b', c: ['d', 'e=f'], f: [['g'], ['h']] },
    { encodeValuesOnly: true }
);
assert.equal(encodedValues,'a=b&c[0]=d&c[1]=e%3Df&f[0][0]=g&f[1][0]=h');
```

This encoding can also be replaced by a custom encoding method set as `encoder` option:

```javascript
var encoded = qs.stringify({ a: { b: 'c' } }, { encoder: function (str) {
    // Passed in values `a`, `b`, `c`
    return // Return encoded string
}})
```

_(Note: the `encoder` option does not apply if `encode` is `false`)_

Analogue to the `encoder` there is a `decoder` option for `parse` to override decoding of properties and values:

```javascript
var decoded = qs.parse('x=z', { decoder: function (str) {
    // Passed in values `x`, `z`
    return // Return decoded string
}})
```

You can encode keys and values using different logic by using the type argument provided to the encoder:

```javascript
var encoded = qs.stringify({ a: { b: 'c' } }, { encoder: function (str, defaultEncoder, charset, type) {
    if (type === 'key') {
        return // Encoded key
    } else if (type === 'value') {
        return // Encoded value
    }
}})
```

The type argument is also provided to the decoder:

```javascript
var decoded = qs.parse('x=z', { decoder: function (str, defaultDecoder, charset, type) {
    if (type === 'key') {
        return // Decoded key
    } else if (type === 'value') {
        return // Decoded value
    }
}})
```

Examples beyond this point will be shown as though the output is not URI encoded for clarity.
Please note that the return values in these cases *will* be URI encoded during real usage.

When arrays are stringified, they follow the `arrayFormat` option, which defaults to `indices`:

```javascript
qs.stringify({ a: ['b', 'c', 'd'] });
// 'a[0]=b&a[1]=c&a[2]=d'
```

You may override this by setting the `indices` option to `false`, or to be more explicit, the `arrayFormat` option to `repeat`:

```javascript
qs.stringify({ a: ['b', 'c', 'd'] }, { indices: false });
// 'a=b&a=c&a=d'
```

You may use the `arrayFormat` option to specify the format of the output array:

```javascript
qs.stringify({ a: ['b', 'c'] }, { arrayFormat: 'indices' })
// 'a[0]=b&a[1]=c'
qs.stringify({ a: ['b', 'c'] }, { arrayFormat: 'brackets' })
// 'a[]=b&a[]=c'
qs.stringify({ a: ['b', 'c'] }, { arrayFormat: 'repeat' })
// 'a=b&a=c'
qs.stringify({ a: ['b', 'c'] }, { arrayFormat: 'comma' })
// 'a=b,c'
```

Note: when using `arrayFormat` set to `'comma'`, you can also pass the `commaRoundTrip` option set to `true` or `false`, to append `[]` on single-item arrays, so that they can round trip through a parse.

When objects are stringified, by default they use bracket notation:

```javascript
qs.stringify({ a: { b: { c: 'd', e: 'f' } } });
// 'a[b][c]=d&a[b][e]=f'
```

You may override this to use dot notation by setting the `allowDots` option to `true`:

```javascript
qs.stringify({ a: { b: { c: 'd', e: 'f' } } }, { allowDots: true });
// 'a.b.c=d&a.b.e=f'
```

You may encode the dot notation in the keys of object with option `encodeDotInKeys` by setting it to `true`:
Note: it implies `allowDots`, so `stringify` will error if you set `decodeDotInKeys` to `true`, and `allowDots` to `false`.
Caveat: when `encodeValuesOnly` is `true` as well as `encodeDotInKeys`, only dots in keys and nothing else will be encoded.
```javascript
qs.stringify({ "name.obj": { "first": "John", "last": "Doe" } }, { allowDots: true, encodeDotInKeys: true })
// 'name%252Eobj.first=John&name%252Eobj.last=Doe'
```

You may allow empty array values by setting the `allowEmptyArrays` option to `true`:
```javascript
qs.stringify({ foo: [], bar: 'baz' }, { allowEmptyArrays: true });
// 'foo[]&bar=baz'
```

Empty strings and null values will omit the value, but the equals sign (=) remains in place:

```javascript
assert.equal(qs.stringify({ a: '' }), 'a=');
```

Key with no values (such as an empty object or array) will return nothing:

```javascript
assert.equal(qs.stringify({ a: [] }), '');
assert.equal(qs.stringify({ a: {} }), '');
assert.equal(qs.stringify({ a: [{}] }), '');
assert.equal(qs.stringify({ a: { b: []} }), '');
assert.equal(qs.stringify({ a: { b: {}} }), '');
```

Properties that are set to `undefined` will be omitted entirely:

```javascript
assert.equal(qs.stringify({ a: null, b: undefined }), 'a=');
```

The query string may optionally be prepended with a question mark:

```javascript
assert.equal(qs.stringify({ a: 'b', c: 'd' }, { addQueryPrefix: true }), '?a=b&c=d');
```

The delimiter may be overridden with stringify as well:

```javascript
assert.equal(qs.stringify({ a: 'b', c: 'd' }, { delimiter: ';' }), 'a=b;c=d');
```

If you only want to override the serialization of `Date` objects, you can provide a `serializeDate` option:

```javascript
var date = new Date(7);
assert.equal(qs.stringify({ a: date }), 'a=1970-01-01T00:00:00.007Z'.replace(/:/g, '%3A'));
assert.equal(
    qs.stringify({ a: date }, { serializeDate: function (d) { return d.getTime(); } }),
    'a=7'
);
```

You may use the `sort` option to affect the order of parameter keys:

```javascript
function alphabeticalSort(a, b) {
    return a.localeCompare(b);
}
assert.equal(qs.stringify({ a: 'c', z: 'y', b : 'f' }, { sort: alphabeticalSort }), 'a=c&b=f&z=y');
```

Finally, you can use the `filter` option to restrict which keys will be included in the stringified output.
If you pass a function, it will be called for each key to obtain the replacement value.
Otherwise, if you pass an array, it will be used to select properties and array indices for stringification:

```javascript
function filterFunc(prefix, value) {
    if (prefix == 'b') {
        // Return an `undefined` value to omit a property.
        return;
    }
    if (prefix == 'e[f]') {
        return value.getTime();
    }
    if (prefix == 'e[g][0]') {
        return value * 2;
    }
    return value;
}
qs.stringify({ a: 'b', c: 'd', e: { f: new Date(123), g: [2] } }, { filter: filterFunc });
// 'a=b&c=d&e[f]=123&e[g][0]=4'
qs.stringify({ a: 'b', c: 'd', e: 'f' }, { filter: ['a', 'e'] });
// 'a=b&e=f'
qs.stringify({ a: ['b', 'c', 'd'], e: 'f' }, { filter: ['a', 0, 2] });
// 'a[0]=b&a[2]=d'
```

You could also use `filter` to inject custom serialization for user defined types.
Consider you're working with some api that expects query strings of the format for ranges:

```
https://domain.com/endpoint?range=30...70
```

For which you model as:

```javascript
class Range {
    constructor(from, to) {
        this.from = from;
        this.to = to;
    }
}
```

You could _inject_ a custom serializer to handle values of this type:

```javascript
qs.stringify(
    {
        range: new Range(30, 70),
    },
    {
        filter: (prefix, value) => {
            if (value instanceof Range) {
                return `${value.from}...${value.to}`;
            }
            // serialize the usual way
            return value;
        },
    }
);
// range=30...70
```

### Handling of `null` values

By default, `null` values are treated like empty strings:

```javascript
var withNull = qs.stringify({ a: null, b: '' });
assert.equal(withNull, 'a=&b=');
```

Parsing does not distinguish between parameters with and without equal signs.
Both are converted to empty strings.

```javascript
var equalsInsensitive = qs.parse('a&b=');
assert.deepEqual(equalsInsensitive, { a: '', b: '' });
```

To distinguish between `null` values and empty strings use the `strictNullHandling` flag. In the result string the `null`
values have no `=` sign:

```javascript
var strictNull = qs.stringify({ a: null, b: '' }, { strictNullHandling: true });
assert.equal(strictNull, 'a&b=');
```

To parse values without `=` back to `null` use the `strictNullHandling` flag:

```javascript
var parsedStrictNull = qs.parse('a&b=', { strictNullHandling: true });
assert.deepEqual(parsedStrictNull, { a: null, b: '' });
```

To completely skip rendering keys with `null` values, use the `skipNulls` flag:

```javascript
var nullsSkipped = qs.stringify({ a: 'b', c: null}, { skipNulls: true });
assert.equal(nullsSkipped, 'a=b');
```

If you're communicating with legacy systems, you can switch to `iso-8859-1` using the `charset` option:

```javascript
var iso = qs.stringify({ æ: 'æ' }, { charset: 'iso-8859-1' });
assert.equal(iso, '%E6=%E6');
```

Characters that don't exist in `iso-8859-1` will be converted to numeric entities, similar to what browsers do:

```javascript
var numeric = qs.stringify({ a: '☺' }, { charset: 'iso-8859-1' });
assert.equal(numeric, 'a=%26%239786%3B');
```

You can use the `charsetSentinel` option to announce the character by including an `utf8=✓` parameter with the proper encoding if the checkmark, similar to what Ruby on Rails and others do when submitting forms.

```javascript
var sentinel = qs.stringify({ a: '☺' }, { charsetSentinel: true });
assert.equal(sentinel, 'utf8=%E2%9C%93&a=%E2%98%BA');

var isoSentinel = qs.stringify({ a: 'æ' }, { charsetSentinel: true, charset: 'iso-8859-1' });
assert.equal(isoSentinel, 'utf8=%26%2310003%3B&a=%E6');
```

### Dealing with special character sets

By default the encoding and decoding of characters is done in `utf-8`, and `iso-8859-1` support is also built in via the `charset` parameter.

If you wish to encode querystrings to a different character set (i.e.
[Shift JIS](https://en.wikipedia.org/wiki/Shift_JIS)) you can use the
[`qs-iconv`](https://github.com/martinheidegger/qs-iconv) library:

```javascript
var encoder = require('qs-iconv/encoder')('shift_jis');
var shiftJISEncoded = qs.stringify({ a: 'こんにちは！' }, { encoder: encoder });
assert.equal(shiftJISEncoded, 'a=%82%B1%82%F1%82%C9%82%BF%82%CD%81I');
```

This also works for decoding of query strings:

```javascript
var decoder = require('qs-iconv/decoder')('shift_jis');
var obj = qs.parse('a=%82%B1%82%F1%82%C9%82%BF%82%CD%81I', { decoder: decoder });
assert.deepEqual(obj, { a: 'こんにちは！' });
```

### RFC 3986 and RFC 1738 space encoding

RFC3986 used as default option and encodes ' ' to *%20* which is backward compatible.
In the same time, output can be stringified as per RFC1738 with ' ' equal to '+'.

```
assert.equal(qs.stringify({ a: 'b c' }), 'a=b%20c');
assert.equal(qs.stringify({ a: 'b c' }, { format : 'RFC3986' }), 'a=b%20c');
assert.equal(qs.stringify({ a: 'b c' }, { format : 'RFC1738' }), 'a=b+c');
```

## Security

Please email [@ljharb](https://github.com/ljharb) or see https://tidelift.com/security if you have a potential security vulnerability to report.

## qs for enterprise

Available as part of the Tidelift Subscription

The maintainers of qs and thousands of other packages are working with Tidelift to deliver commercial support and maintenance for the open source dependencies you use to build your applications.
Save time, reduce risk, and improve code health, while paying the maintainers of the exact dependencies you use.
[Learn more.](https://tidelift.com/subscription/pkg/npm-qs?utm_source=npm-qs&utm_medium=referral&utm_campaign=enterprise&utm_term=repo)

[package-url]: https://npmjs.org/package/qs
[npm-version-svg]: https://versionbadg.es/ljharb/qs.svg
[deps-svg]: https://david-dm.org/ljharb/qs.svg
[deps-url]: https://david-dm.org/ljharb/qs
[dev-deps-svg]: https://david-dm.org/ljharb/qs/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/qs#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/qs.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/qs.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/qs.svg
[downloads-url]: https://npm-stat.com/charts.html?package=qs
[codecov-image]: https://codecov.io/gh/ljharb/qs/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/qs/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/qs
[actions-url]: https://github.com/ljharb/qs/actions

## Acknowledgements

qs logo by [NUMI](https://github.com/numi-hq/open-design):

[<img src="https://raw.githubusercontent.com/numi-hq/open-design/main/assets/numi-lockup.png" alt="NUMI Logo" style="width: 200px;"/>](https://numi.tech/?ref=qs)
```

### 文件: `node_modules/raw-body/README.md`

```markdown
# raw-body

[![NPM Version][npm-image]][npm-url]
[![NPM Downloads][downloads-image]][downloads-url]
[![Node.js Version][node-version-image]][node-version-url]
[![Build status][github-actions-ci-image]][github-actions-ci-url]
[![Test coverage][coveralls-image]][coveralls-url]

Gets the entire buffer of a stream either as a `Buffer` or a string.
Validates the stream's length against an expected length and maximum limit.
Ideal for parsing request bodies.

## Install

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```sh
$ npm install raw-body
```

### TypeScript

This module includes a [TypeScript](https://www.typescriptlang.org/)
declaration file to enable auto complete in compatible editors and type
information for TypeScript projects. This module depends on the Node.js
types, so install `@types/node`:

```sh
$ npm install @types/node
```

## API

```js
var getRawBody = require('raw-body')
```

### getRawBody(stream, [options], [callback])

**Returns a promise if no callback specified and global `Promise` exists.**

Options:

- `length` - The length of the stream.
  If the contents of the stream do not add up to this length,
  an `400` error code is returned.
- `limit` - The byte limit of the body.
  This is the number of bytes or any string format supported by
  [bytes](https://www.npmjs.com/package/bytes),
  for example `1000`, `'500kb'` or `'3mb'`.
  If the body ends up being larger than this limit,
  a `413` error code is returned.
- `encoding` - The encoding to use to decode the body into a string.
  By default, a `Buffer` instance will be returned when no encoding is specified.
  Most likely, you want `utf-8`, so setting `encoding` to `true` will decode as `utf-8`.
  You can use any type of encoding supported by [iconv-lite](https://www.npmjs.org/package/iconv-lite#readme).

You can also pass a string in place of options to just specify the encoding.

If an error occurs, the stream will be paused, everything unpiped,
and you are responsible for correctly disposing the stream.
For HTTP requests, you may need to finish consuming the stream if
you want to keep the socket open for future requests. For streams
that use file descriptors, you should `stream.destroy()` or
`stream.close()` to prevent leaks.

## Errors

This module creates errors depending on the error condition during reading.
The error may be an error from the underlying Node.js implementation, but is
otherwise an error created by this module, which has the following attributes:

  * `limit` - the limit in bytes
  * `length` and `expected` - the expected length of the stream
  * `received` - the received bytes
  * `encoding` - the invalid encoding
  * `status` and `statusCode` - the corresponding status code for the error
  * `type` - the error type

### Types

The errors from this module have a `type` property which allows for the programmatic
determination of the type of error returned.

#### encoding.unsupported

This error will occur when the `encoding` option is specified, but the value does
not map to an encoding supported by the [iconv-lite](https://www.npmjs.org/package/iconv-lite#readme)
module.

#### entity.too.large

This error will occur when the `limit` option is specified, but the stream has
an entity that is larger.

#### request.aborted

This error will occur when the request stream is aborted by the client before
reading the body has finished.

#### request.size.invalid

This error will occur when the `length` option is specified, but the stream has
emitted more bytes.

#### stream.encoding.set

This error will occur when the given stream has an encoding set on it, making it
a decoded stream. The stream should not have an encoding set and is expected to
emit `Buffer` objects.

#### stream.not.readable

This error will occur when the given stream is not readable.

## Examples

### Simple Express example

```js
var contentType = require('content-type')
var express = require('express')
var getRawBody = require('raw-body')

var app = express()

app.use(function (req, res, next) {
  getRawBody(req, {
    length: req.headers['content-length'],
    limit: '1mb',
    encoding: contentType.parse(req).parameters.charset
  }, function (err, string) {
    if (err) return next(err)
    req.text = string
    next()
  })
})

// now access req.text
```

### Simple Koa example

```js
var contentType = require('content-type')
var getRawBody = require('raw-body')
var koa = require('koa')

var app = koa()

app.use(function * (next) {
  this.text = yield getRawBody(this.req, {
    length: this.req.headers['content-length'],
    limit: '1mb',
    encoding: contentType.parse(this.req).parameters.charset
  })
  yield next
})

// now access this.text
```

### Using as a promise

To use this library as a promise, simply omit the `callback` and a promise is
returned, provided that a global `Promise` is defined.

```js
var getRawBody = require('raw-body')
var http = require('http')

var server = http.createServer(function (req, res) {
  getRawBody(req)
    .then(function (buf) {
      res.statusCode = 200
      res.end(buf.length + ' bytes submitted')
    })
    .catch(function (err) {
      res.statusCode = 500
      res.end(err.message)
    })
})

server.listen(3000)
```

### Using with TypeScript

```ts
import * as getRawBody from 'raw-body';
import * as http from 'http';

const server = http.createServer((req, res) => {
  getRawBody(req)
  .then((buf) => {
    res.statusCode = 200;
    res.end(buf.length + ' bytes submitted');
  })
  .catch((err) => {
    res.statusCode = err.statusCode;
    res.end(err.message);
  });
});

server.listen(3000);
```

## License

[MIT](LICENSE)

[npm-image]: https://img.shields.io/npm/v/raw-body.svg
[npm-url]: https://npmjs.org/package/raw-body
[node-version-image]: https://img.shields.io/node/v/raw-body.svg
[node-version-url]: https://nodejs.org/en/download/
[coveralls-image]: https://img.shields.io/coveralls/stream-utils/raw-body/master.svg
[coveralls-url]: https://coveralls.io/r/stream-utils/raw-body?branch=master
[downloads-image]: https://img.shields.io/npm/dm/raw-body.svg
[downloads-url]: https://npmjs.org/package/raw-body
[github-actions-ci-image]: https://img.shields.io/github/actions/workflow/status/stream-utils/raw-body/ci.yml?branch=master&label=ci
[github-actions-ci-url]: https://github.com/jshttp/stream-utils/raw-body?query=workflow%3Aci
```

### 文件: `node_modules/safer-buffer/Readme.md`

```markdown
# safer-buffer [![travis][travis-image]][travis-url] [![npm][npm-image]][npm-url] [![javascript style guide][standard-image]][standard-url] [![Security Responsible Disclosure][secuirty-image]][secuirty-url]

[travis-image]: https://travis-ci.org/ChALkeR/safer-buffer.svg?branch=master
[travis-url]: https://travis-ci.org/ChALkeR/safer-buffer
[npm-image]: https://img.shields.io/npm/v/safer-buffer.svg
[npm-url]: https://npmjs.org/package/safer-buffer
[standard-image]: https://img.shields.io/badge/code_style-standard-brightgreen.svg
[standard-url]: https://standardjs.com
[secuirty-image]: https://img.shields.io/badge/Security-Responsible%20Disclosure-green.svg
[secuirty-url]: https://github.com/nodejs/security-wg/blob/master/processes/responsible_disclosure_template.md

Modern Buffer API polyfill without footguns, working on Node.js from 0.8 to current.

## How to use?

First, port all `Buffer()` and `new Buffer()` calls to `Buffer.alloc()` and `Buffer.from()` API.

Then, to achieve compatibility with outdated Node.js versions (`<4.5.0` and 5.x `<5.9.0`), use
`const Buffer = require('safer-buffer').Buffer` in all files where you make calls to the new
Buffer API. _Use `var` instead of `const` if you need that for your Node.js version range support._

Also, see the
[porting Buffer](https://github.com/ChALkeR/safer-buffer/blob/master/Porting-Buffer.md) guide.

## Do I need it?

Hopefully, not — dropping support for outdated Node.js versions should be fine nowdays, and that
is the recommended path forward. You _do_ need to port to the `Buffer.alloc()` and `Buffer.from()`
though.

See the [porting guide](https://github.com/ChALkeR/safer-buffer/blob/master/Porting-Buffer.md)
for a better description.

## Why not [safe-buffer](https://npmjs.com/safe-buffer)?

_In short: while `safe-buffer` serves as a polyfill for the new API, it allows old API usage and
itself contains footguns._

`safe-buffer` could be used safely to get the new API while still keeping support for older
Node.js versions (like this module), but while analyzing ecosystem usage of the old Buffer API
I found out that `safe-buffer` is itself causing problems in some cases.

For example, consider the following snippet:

```console
$ cat example.unsafe.js
console.log(Buffer(20))
$ ./node-v6.13.0-linux-x64/bin/node example.unsafe.js
<Buffer 0a 00 00 00 00 00 00 00 28 13 de 02 00 00 00 00 05 00 00 00>
$ standard example.unsafe.js
standard: Use JavaScript Standard Style (https://standardjs.com)
  /home/chalker/repo/safer-buffer/example.unsafe.js:2:13: 'Buffer()' was deprecated since v6. Use 'Buffer.alloc()' or 'Buffer.from()' (use 'https://www.npmjs.com/package/safe-buffer' for '<4.5.0') instead.
```

This is allocates and writes to console an uninitialized chunk of memory.
[standard](https://www.npmjs.com/package/standard) linter (among others) catch that and warn people
to avoid using unsafe API.

Let's now throw in `safe-buffer`!

```console
$ cat example.safe-buffer.js
const Buffer = require('safe-buffer').Buffer
console.log(Buffer(20))
$ standard example.safe-buffer.js
$ ./node-v6.13.0-linux-x64/bin/node example.safe-buffer.js
<Buffer 08 00 00 00 00 00 00 00 28 58 01 82 fe 7f 00 00 00 00 00 00>
```

See the problem? Adding in `safe-buffer` _magically removes the lint warning_, but the behavior
remains identiсal to what we had before, and when launched on Node.js 6.x LTS — this dumps out
chunks of uninitialized memory.
_And this code will still emit runtime warnings on Node.js 10.x and above._

That was done by design. I first considered changing `safe-buffer`, prohibiting old API usage or
emitting warnings on it, but that significantly diverges from `safe-buffer` design. After some
discussion, it was decided to move my approach into a separate package, and _this is that separate
package_.

This footgun is not imaginary — I observed top-downloaded packages doing that kind of thing,
«fixing» the lint warning by blindly including `safe-buffer` without any actual changes.

Also in some cases, even if the API _was_ migrated to use of safe Buffer API — a random pull request
can bring unsafe Buffer API usage back to the codebase by adding new calls — and that could go
unnoticed even if you have a linter prohibiting that (becase of the reason stated above), and even
pass CI. _I also observed that being done in popular packages._

Some examples:
 * [webdriverio](https://github.com/webdriverio/webdriverio/commit/05cbd3167c12e4930f09ef7cf93b127ba4effae4#diff-124380949022817b90b622871837d56cR31)
   (a module with 548 759 downloads/month),
 * [websocket-stream](https://github.com/maxogden/websocket-stream/commit/c9312bd24d08271687d76da0fe3c83493871cf61)
   (218 288 d/m, fix in [maxogden/websocket-stream#142](https://github.com/maxogden/websocket-stream/pull/142)),
 * [node-serialport](https://github.com/node-serialport/node-serialport/commit/e8d9d2b16c664224920ce1c895199b1ce2def48c)
   (113 138 d/m, fix in [node-serialport/node-serialport#1510](https://github.com/node-serialport/node-serialport/pull/1510)),
 * [karma](https://github.com/karma-runner/karma/commit/3d94b8cf18c695104ca195334dc75ff054c74eec)
   (3 973 193 d/m, fix in [karma-runner/karma#2947](https://github.com/karma-runner/karma/pull/2947)),
 * [spdy-transport](https://github.com/spdy-http2/spdy-transport/commit/5375ac33f4a62a4f65bcfc2827447d42a5dbe8b1)
   (5 970 727 d/m, fix in [spdy-http2/spdy-transport#53](https://github.com/spdy-http2/spdy-transport/pull/53)).
 * And there are a lot more over the ecosystem.

I filed a PR at
[mysticatea/eslint-plugin-node#110](https://github.com/mysticatea/eslint-plugin-node/pull/110) to
partially fix that (for cases when that lint rule is used), but it is a semver-major change for
linter rules and presets, so it would take significant time for that to reach actual setups.
_It also hasn't been released yet (2018-03-20)._

Also, `safer-buffer` discourages the usage of `.allocUnsafe()`, which is often done by a mistake.
It still supports it with an explicit concern barier, by placing it under
`require('safer-buffer/dangereous')`.

## But isn't throwing bad?

Not really. It's an error that could be noticed and fixed early, instead of causing havoc later like
unguarded `new Buffer()` calls that end up receiving user input can do.

This package affects only the files where `var Buffer = require('safer-buffer').Buffer` was done, so
it is really simple to keep track of things and make sure that you don't mix old API usage with that.
Also, CI should hint anything that you might have missed.

New commits, if tested, won't land new usage of unsafe Buffer API this way.
_Node.js 10.x also deals with that by printing a runtime depecation warning._

### Would it affect third-party modules?

No, unless you explicitly do an awful thing like monkey-patching or overriding the built-in `Buffer`.
Don't do that.

### But I don't want throwing…

That is also fine!

Also, it could be better in some cases when you don't comprehensive enough test coverage.

In that case — just don't override `Buffer` and use
`var SaferBuffer = require('safer-buffer').Buffer` instead.

That way, everything using `Buffer` natively would still work, but there would be two drawbacks:

* `Buffer.from`/`Buffer.alloc` won't be polyfilled — use `SaferBuffer.from` and
  `SaferBuffer.alloc` instead.
* You are still open to accidentally using the insecure deprecated API — use a linter to catch that.

Note that using a linter to catch accidential `Buffer` constructor usage in this case is strongly
recommended. `Buffer` is not overriden in this usecase, so linters won't get confused.

## «Without footguns»?

Well, it is still possible to do _some_ things with `Buffer` API, e.g. accessing `.buffer` property
on older versions and duping things from there. You shouldn't do that in your code, probabably.

The intention is to remove the most significant footguns that affect lots of packages in the
ecosystem, and to do it in the proper way.

Also, this package doesn't protect against security issues affecting some Node.js versions, so for
usage in your own production code, it is still recommended to update to a Node.js version
[supported by upstream](https://github.com/nodejs/release#release-schedule).
```

### 文件: `node_modules/setprototypeof/README.md`

```markdown
# Polyfill for `Object.setPrototypeOf`

[![NPM Version](https://img.shields.io/npm/v/setprototypeof.svg)](https://npmjs.org/package/setprototypeof)
[![NPM Downloads](https://img.shields.io/npm/dm/setprototypeof.svg)](https://npmjs.org/package/setprototypeof)
[![js-standard-style](https://img.shields.io/badge/code%20style-standard-brightgreen.svg)](https://github.com/standard/standard)

A simple cross platform implementation to set the prototype of an instianted object.  Supports all modern browsers and at least back to IE8.

## Usage:

```
$ npm install --save setprototypeof
```

```javascript
var setPrototypeOf = require('setprototypeof')

var obj = {}
setPrototypeOf(obj, {
  foo: function () {
    return 'bar'
  }
})
obj.foo() // bar
```

TypeScript is also supported:

```typescript
import setPrototypeOf from 'setprototypeof'
```
```

### 文件: `node_modules/side-channel-list/README.md`

```markdown
# side-channel-list <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Store information about any JS value in a side channel, using a linked list.

Warning: this implementation will leak memory until you `delete` the `key`.
Use [`side-channel`](https://npmjs.com/side-channel) for the best available strategy.

## Getting started

```sh
npm install --save side-channel-list
```

## Usage/Examples

```js
const assert = require('assert');
const getSideChannelList = require('side-channel-list');

const channel = getSideChannelList();

const key = {};
assert.equal(channel.has(key), false);
assert.throws(() => channel.assert(key), TypeError);

channel.set(key, 42);

channel.assert(key); // does not throw
assert.equal(channel.has(key), true);
assert.equal(channel.get(key), 42);

channel.delete(key);
assert.equal(channel.has(key), false);
assert.throws(() => channel.assert(key), TypeError);
```

## Tests

Clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/side-channel-list
[npm-version-svg]: https://versionbadg.es/ljharb/side-channel-list.svg
[deps-svg]: https://david-dm.org/ljharb/side-channel-list.svg
[deps-url]: https://david-dm.org/ljharb/side-channel-list
[dev-deps-svg]: https://david-dm.org/ljharb/side-channel-list/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/side-channel-list#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/side-channel-list.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/side-channel-list.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/side-channel-list.svg
[downloads-url]: https://npm-stat.com/charts.html?package=side-channel-list
[codecov-image]: https://codecov.io/gh/ljharb/side-channel-list/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/side-channel-list/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/side-channel-list
[actions-url]: https://github.com/ljharb/side-channel-list/actions
```

### 文件: `node_modules/side-channel-map/README.md`

```markdown
# side-channel-map <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Store information about any JS value in a side channel, using a Map.

Warning: if the `key` is an object, this implementation will leak memory until you `delete` it.
Use [`side-channel`](https://npmjs.com/side-channel) for the best available strategy.

## Getting started

```sh
npm install --save side-channel-map
```

## Usage/Examples

```js
const assert = require('assert');
const getSideChannelMap = require('side-channel-map');

const channel = getSideChannelMap();

const key = {};
assert.equal(channel.has(key), false);
assert.throws(() => channel.assert(key), TypeError);

channel.set(key, 42);

channel.assert(key); // does not throw
assert.equal(channel.has(key), true);
assert.equal(channel.get(key), 42);

channel.delete(key);
assert.equal(channel.has(key), false);
assert.throws(() => channel.assert(key), TypeError);
```

## Tests

Clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/side-channel-map
[npm-version-svg]: https://versionbadg.es/ljharb/side-channel-map.svg
[deps-svg]: https://david-dm.org/ljharb/side-channel-map.svg
[deps-url]: https://david-dm.org/ljharb/side-channel-map
[dev-deps-svg]: https://david-dm.org/ljharb/side-channel-map/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/side-channel-map#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/side-channel-map.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/side-channel-map.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/side-channel-map.svg
[downloads-url]: https://npm-stat.com/charts.html?package=side-channel-map
[codecov-image]: https://codecov.io/gh/ljharb/side-channel-map/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/side-channel-map/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/side-channel-map
[actions-url]: https://github.com/ljharb/side-channel-map/actions
```

### 文件: `node_modules/side-channel-weakmap/README.md`

```markdown
# side-channel-weakmap <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Store information about any JS value in a side channel. Uses WeakMap if available.

Warning: this implementation will leak memory until you `delete` the `key`.
Use [`side-channel`](https://npmjs.com/side-channel) for the best available strategy.

## Getting started

```sh
npm install --save side-channel-weakmap
```

## Usage/Examples

```js
const assert = require('assert');
const getSideChannelList = require('side-channel-weakmap');

const channel = getSideChannelList();

const key = {};
assert.equal(channel.has(key), false);
assert.throws(() => channel.assert(key), TypeError);

channel.set(key, 42);

channel.assert(key); // does not throw
assert.equal(channel.has(key), true);
assert.equal(channel.get(key), 42);

channel.delete(key);
assert.equal(channel.has(key), false);
assert.throws(() => channel.assert(key), TypeError);
```

## Tests

Clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/side-channel-weakmap
[npm-version-svg]: https://versionbadg.es/ljharb/side-channel-weakmap.svg
[deps-svg]: https://david-dm.org/ljharb/side-channel-weakmap.svg
[deps-url]: https://david-dm.org/ljharb/side-channel-weakmap
[dev-deps-svg]: https://david-dm.org/ljharb/side-channel-weakmap/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/side-channel-weakmap#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/side-channel-weakmap.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/side-channel-weakmap.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/side-channel-weakmap.svg
[downloads-url]: https://npm-stat.com/charts.html?package=side-channel-weakmap
[codecov-image]: https://codecov.io/gh/ljharb/side-channel-weakmap/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/side-channel-weakmap/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/side-channel-weakmap
[actions-url]: https://github.com/ljharb/side-channel-weakmap/actions
```

### 文件: `node_modules/side-channel/README.md`

```markdown
# side-channel <sup>[![Version Badge][npm-version-svg]][package-url]</sup>

[![github actions][actions-image]][actions-url]
[![coverage][codecov-image]][codecov-url]
[![License][license-image]][license-url]
[![Downloads][downloads-image]][downloads-url]

[![npm badge][npm-badge-png]][package-url]

Store information about any JS value in a side channel. Uses WeakMap if available.

Warning: in an environment that lacks `WeakMap`, this implementation will leak memory until you `delete` the `key`.

## Getting started

```sh
npm install --save side-channel
```

## Usage/Examples

```js
const assert = require('assert');
const getSideChannel = require('side-channel');

const channel = getSideChannel();

const key = {};
assert.equal(channel.has(key), false);
assert.throws(() => channel.assert(key), TypeError);

channel.set(key, 42);

channel.assert(key); // does not throw
assert.equal(channel.has(key), true);
assert.equal(channel.get(key), 42);

channel.delete(key);
assert.equal(channel.has(key), false);
assert.throws(() => channel.assert(key), TypeError);
```

## Tests

Clone the repo, `npm install`, and run `npm test`

[package-url]: https://npmjs.org/package/side-channel
[npm-version-svg]: https://versionbadg.es/ljharb/side-channel.svg
[deps-svg]: https://david-dm.org/ljharb/side-channel.svg
[deps-url]: https://david-dm.org/ljharb/side-channel
[dev-deps-svg]: https://david-dm.org/ljharb/side-channel/dev-status.svg
[dev-deps-url]: https://david-dm.org/ljharb/side-channel#info=devDependencies
[npm-badge-png]: https://nodei.co/npm/side-channel.png?downloads=true&stars=true
[license-image]: https://img.shields.io/npm/l/side-channel.svg
[license-url]: LICENSE
[downloads-image]: https://img.shields.io/npm/dm/side-channel.svg
[downloads-url]: https://npm-stat.com/charts.html?package=side-channel
[codecov-image]: https://codecov.io/gh/ljharb/side-channel/branch/main/graphs/badge.svg
[codecov-url]: https://app.codecov.io/gh/ljharb/side-channel/
[actions-image]: https://img.shields.io/endpoint?url=https://github-actions-badge-u3jn4tfpocch.runkit.sh/ljharb/side-channel
[actions-url]: https://github.com/ljharb/side-channel/actions
```

### 文件: `node_modules/statuses/README.md`

```markdown
# statuses

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Node.js Version][node-version-image]][node-version-url]
[![Build Status][ci-image]][ci-url]
[![Test Coverage][coveralls-image]][coveralls-url]

HTTP status utility for node.

This module provides a list of status codes and messages sourced from
a few different projects:

  * The [IANA Status Code Registry](https://www.iana.org/assignments/http-status-codes/http-status-codes.xhtml)
  * The [Node.js project](https://nodejs.org/)
  * The [NGINX project](https://www.nginx.com/)
  * The [Apache HTTP Server project](https://httpd.apache.org/)

## Installation

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```sh
$ npm install statuses
```

## API

<!-- eslint-disable no-unused-vars -->

```js
var status = require('statuses')
```

### status(code)

Returns the status message string for a known HTTP status code. The code
may be a number or a string. An error is thrown for an unknown status code.

<!-- eslint-disable no-undef -->

```js
status(403) // => 'Forbidden'
status('403') // => 'Forbidden'
status(306) // throws
```

### status(msg)

Returns the numeric status code for a known HTTP status message. The message
is case-insensitive. An error is thrown for an unknown status message.

<!-- eslint-disable no-undef -->

```js
status('forbidden') // => 403
status('Forbidden') // => 403
status('foo') // throws
```

### status.codes

Returns an array of all the status codes as `Integer`s.

### status.code[msg]

Returns the numeric status code for a known status message (in lower-case),
otherwise `undefined`.

<!-- eslint-disable no-undef, no-unused-expressions -->

```js
status['not found'] // => 404
```

### status.empty[code]

Returns `true` if a status code expects an empty body.

<!-- eslint-disable no-undef, no-unused-expressions -->

```js
status.empty[200] // => undefined
status.empty[204] // => true
status.empty[304] // => true
```

### status.message[code]

Returns the string message for a known numeric status code, otherwise
`undefined`. This object is the same format as the
[Node.js http module `http.STATUS_CODES`](https://nodejs.org/dist/latest/docs/api/http.html#http_http_status_codes).

<!-- eslint-disable no-undef, no-unused-expressions -->

```js
status.message[404] // => 'Not Found'
```

### status.redirect[code]

Returns `true` if a status code is a valid redirect status.

<!-- eslint-disable no-undef, no-unused-expressions -->

```js
status.redirect[200] // => undefined
status.redirect[301] // => true
```

### status.retry[code]

Returns `true` if you should retry the rest.

<!-- eslint-disable no-undef, no-unused-expressions -->

```js
status.retry[501] // => undefined
status.retry[503] // => true
```

## License

[MIT](LICENSE)

[ci-image]: https://badgen.net/github/checks/jshttp/statuses/master?label=ci
[ci-url]: https://github.com/jshttp/statuses/actions?query=workflow%3Aci
[coveralls-image]: https://badgen.net/coveralls/c/github/jshttp/statuses/master
[coveralls-url]: https://coveralls.io/r/jshttp/statuses?branch=master
[node-version-image]: https://badgen.net/npm/node/statuses
[node-version-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/statuses
[npm-url]: https://npmjs.org/package/statuses
[npm-version-image]: https://badgen.net/npm/v/statuses
```

### 文件: `node_modules/toidentifier/README.md`

```markdown
# toidentifier

[![NPM Version][npm-image]][npm-url]
[![NPM Downloads][downloads-image]][downloads-url]
[![Build Status][github-actions-ci-image]][github-actions-ci-url]
[![Test Coverage][codecov-image]][codecov-url]

> Convert a string of words to a JavaScript identifier

## Install

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```bash
$ npm install toidentifier
```

## Example

```js
var toIdentifier = require('toidentifier')

console.log(toIdentifier('Bad Request'))
// => "BadRequest"
```

## API

This CommonJS module exports a single default function: `toIdentifier`.

### toIdentifier(string)

Given a string as the argument, it will be transformed according to
the following rules and the new string will be returned:

1. Split into words separated by space characters (`0x20`).
2. Upper case the first character of each word.
3. Join the words together with no separator.
4. Remove all non-word (`[0-9a-z_]`) characters.

## License

[MIT](LICENSE)

[codecov-image]: https://img.shields.io/codecov/c/github/component/toidentifier.svg
[codecov-url]: https://codecov.io/gh/component/toidentifier
[downloads-image]: https://img.shields.io/npm/dm/toidentifier.svg
[downloads-url]: https://npmjs.org/package/toidentifier
[github-actions-ci-image]: https://img.shields.io/github/workflow/status/component/toidentifier/ci/master?label=ci
[github-actions-ci-url]: https://github.com/component/toidentifier?query=workflow%3Aci
[npm-image]: https://img.shields.io/npm/v/toidentifier.svg
[npm-url]: https://npmjs.org/package/toidentifier


##

[npm]: https://www.npmjs.com/

[yarn]: https://yarnpkg.com/
```

### 文件: `node_modules/type-is/README.md`

```markdown
# type-is

[![NPM Version][npm-version-image]][npm-url]
[![NPM Downloads][npm-downloads-image]][npm-url]
[![Node.js Version][node-version-image]][node-version-url]
[![Build Status][ci-image]][ci-url]
[![Test Coverage][coveralls-image]][coveralls-url]

Infer the content-type of a request.

## Install

This is a [Node.js](https://nodejs.org/en/) module available through the
[npm registry](https://www.npmjs.com/). Installation is done using the
[`npm install` command](https://docs.npmjs.com/getting-started/installing-npm-packages-locally):

```sh
$ npm install type-is
```

## API

```js
var http = require('http')
var typeis = require('type-is')

http.createServer(function (req, res) {
  var istext = typeis(req, ['text/*'])
  res.end('you ' + (istext ? 'sent' : 'did not send') + ' me text')
})
```

### typeis(request, types)

Checks if the `request` is one of the `types`. If the request has no body,
even if there is a `Content-Type` header, then `null` is returned. If the
`Content-Type` header is invalid or does not matches any of the `types`, then
`false` is returned. Otherwise, a string of the type that matched is returned.

The `request` argument is expected to be a Node.js HTTP request. The `types`
argument is an array of type strings.

Each type in the `types` array can be one of the following:

- A file extension name such as `json`. This name will be returned if matched.
- A mime type such as `application/json`.
- A mime type with a wildcard such as `*/*` or `*/json` or `application/*`.
  The full mime type will be returned if matched.
- A suffix such as `+json`. This can be combined with a wildcard such as
  `*/vnd+json` or `application/*+json`. The full mime type will be returned
  if matched.

Some examples to illustrate the inputs and returned value:

```js
// req.headers.content-type = 'application/json'

typeis(req, ['json']) // => 'json'
typeis(req, ['html', 'json']) // => 'json'
typeis(req, ['application/*']) // => 'application/json'
typeis(req, ['application/json']) // => 'application/json'

typeis(req, ['html']) // => false
```

### typeis.hasBody(request)

Returns a Boolean if the given `request` has a body, regardless of the
`Content-Type` header.

Having a body has no relation to how large the body is (it may be 0 bytes).
This is similar to how file existence works. If a body does exist, then this
indicates that there is data to read from the Node.js request stream.

```js
if (typeis.hasBody(req)) {
  // read the body, since there is one

  req.on('data', function (chunk) {
    // ...
  })
}
```

### typeis.is(mediaType, types)

Checks if the `mediaType` is one of the `types`. If the `mediaType` is invalid
or does not matches any of the `types`, then `false` is returned. Otherwise, a
string of the type that matched is returned.

The `mediaType` argument is expected to be a
[media type](https://tools.ietf.org/html/rfc6838) string. The `types` argument
is an array of type strings.

Each type in the `types` array can be one of the following:

- A file extension name such as `json`. This name will be returned if matched.
- A mime type such as `application/json`.
- A mime type with a wildcard such as `*/*` or `*/json` or `application/*`.
  The full mime type will be returned if matched.
- A suffix such as `+json`. This can be combined with a wildcard such as
  `*/vnd+json` or `application/*+json`. The full mime type will be returned
  if matched.

Some examples to illustrate the inputs and returned value:

```js
var mediaType = 'application/json'

typeis.is(mediaType, ['json']) // => 'json'
typeis.is(mediaType, ['html', 'json']) // => 'json'
typeis.is(mediaType, ['application/*']) // => 'application/json'
typeis.is(mediaType, ['application/json']) // => 'application/json'

typeis.is(mediaType, ['html']) // => false
```

### typeis.match(expected, actual)

Match the type string `expected` with `actual`, taking in to account wildcards.
A wildcard can only be in the type of the subtype part of a media type and only
in the `expected` value (as `actual` should be the real media type to match). A
suffix can still be included even with a wildcard subtype. If an input is
malformed, `false` will be returned.

```js
typeis.match('text/html', 'text/html') // => true
typeis.match('*/html', 'text/html') // => true
typeis.match('text/*', 'text/html') // => true
typeis.match('*/*', 'text/html') // => true
typeis.match('*/*+json', 'application/x-custom+json') // => true
```

### typeis.normalize(type)

Normalize a `type` string. This works by performing the following:

- If the `type` is not a string, `false` is returned.
- If the string starts with `+` (so it is a `+suffix` shorthand like `+json`),
  then it is expanded to contain the complete wildcard notation of `*/*+suffix`.
- If the string contains a `/`, then it is returned as the type.
- Else the string is assumed to be a file extension and the mapped media type is
  returned, or `false` is there is no mapping.

This includes two special mappings:

- `'multipart'` -> `'multipart/*'`
- `'urlencoded'` -> `'application/x-www-form-urlencoded'`

## Examples

### Example body parser

```js
var express = require('express')
var typeis = require('type-is')

var app = express()

app.use(function bodyParser (req, res, next) {
  if (!typeis.hasBody(req)) {
    return next()
  }

  switch (typeis(req, ['urlencoded', 'json', 'multipart'])) {
    case 'urlencoded':
      // parse urlencoded body
      throw new Error('implement urlencoded body parsing')
    case 'json':
      // parse json body
      throw new Error('implement json body parsing')
    case 'multipart':
      // parse multipart body
      throw new Error('implement multipart body parsing')
    default:
      // 415 error code
      res.statusCode = 415
      res.end()
      break
  }
})
```

## License

[MIT](LICENSE)

[ci-image]: https://badgen.net/github/checks/jshttp/type-is/master?label=ci
[ci-url]: https://github.com/jshttp/type-is/actions/workflows/ci.yml
[coveralls-image]: https://badgen.net/coveralls/c/github/jshttp/type-is/master
[coveralls-url]: https://coveralls.io/r/jshttp/type-is?branch=master
[node-version-image]: https://badgen.net/npm/node/type-is
[node-version-url]: https://nodejs.org/en/download
[npm-downloads-image]: https://badgen.net/npm/dm/type-is
[npm-url]: https://npmjs.org/package/type-is
[npm-version-image]: https://badgen.net/npm/v/type-is
[travis-image]: https://badgen.net/travis/jshttp/type-is/master
[travis-url]: https://travis-ci.org/jshttp/type-is
```

### 文件: `node_modules/unpipe/README.md`

```markdown
# unpipe

[![NPM Version][npm-image]][npm-url]
[![NPM Downloads][downloads-image]][downloads-url]
[![Node.js Version][node-image]][node-url]
[![Build Status][travis-image]][travis-url]
[![Test Coverage][coveralls-image]][coveralls-url]

Unpipe a stream from all destinations.

## Installation

```sh
$ npm install unpipe
```

## API

```js
var unpipe = require('unpipe')
```

### unpipe(stream)

Unpipes all destinations from a given stream. With stream 2+, this is
equivalent to `stream.unpipe()`. When used with streams 1 style streams
(typically Node.js 0.8 and below), this module attempts to undo the
actions done in `stream.pipe(dest)`.

## License

[MIT](LICENSE)

[npm-image]: https://img.shields.io/npm/v/unpipe.svg
[npm-url]: https://npmjs.org/package/unpipe
[node-image]: https://img.shields.io/node/v/unpipe.svg
[node-url]: http://nodejs.org/download/
[travis-image]: https://img.shields.io/travis/stream-utils/unpipe.svg
[travis-url]: https://travis-ci.org/stream-utils/unpipe
[coveralls-image]: https://img.shields.io/coveralls/stream-utils/unpipe.svg
[coveralls-url]: https://coveralls.io/r/stream-utils/unpipe?branch=master
[downloads-image]: https://img.shields.io/npm/dm/unpipe.svg
[downloads-url]: https://npmjs.org/package/unpipe
```

### 文件: `node_modules/ws/README.md`

```markdown
# ws: a Node.js WebSocket library

[![Version npm](https://img.shields.io/npm/v/ws.svg?logo=npm)](https://www.npmjs.com/package/ws)
[![CI](https://img.shields.io/github/actions/workflow/status/websockets/ws/ci.yml?branch=master&label=CI&logo=github)](https://github.com/websockets/ws/actions?query=workflow%3ACI+branch%3Amaster)
[![Coverage Status](https://img.shields.io/coveralls/websockets/ws/master.svg?logo=coveralls)](https://coveralls.io/github/websockets/ws)

ws is a simple to use, blazing fast, and thoroughly tested WebSocket client and
server implementation.

Passes the quite extensive Autobahn test suite: [server][server-report],
[client][client-report].

**Note**: This module does not work in the browser. The client in the docs is a
reference to a backend with the role of a client in the WebSocket communication.
Browser clients must use the native
[`WebSocket`](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)
object. To make the same code work seamlessly on Node.js and the browser, you
can use one of the many wrappers available on npm, like
[isomorphic-ws](https://github.com/heineiuo/isomorphic-ws).

## Table of Contents

- [Protocol support](#protocol-support)
- [Installing](#installing)
  - [Opt-in for performance](#opt-in-for-performance)
    - [Legacy opt-in for performance](#legacy-opt-in-for-performance)
- [API docs](#api-docs)
- [WebSocket compression](#websocket-compression)
- [Usage examples](#usage-examples)
  - [Sending and receiving text data](#sending-and-receiving-text-data)
  - [Sending binary data](#sending-binary-data)
  - [Simple server](#simple-server)
  - [External HTTP/S server](#external-https-server)
  - [Multiple servers sharing a single HTTP/S server](#multiple-servers-sharing-a-single-https-server)
  - [Client authentication](#client-authentication)
  - [Server broadcast](#server-broadcast)
  - [Round-trip time](#round-trip-time)
  - [Use the Node.js streams API](#use-the-nodejs-streams-api)
  - [Other examples](#other-examples)
- [FAQ](#faq)
  - [How to get the IP address of the client?](#how-to-get-the-ip-address-of-the-client)
  - [How to detect and close broken connections?](#how-to-detect-and-close-broken-connections)
  - [How to connect via a proxy?](#how-to-connect-via-a-proxy)
- [Changelog](#changelog)
- [License](#license)

## Protocol support

- **HyBi drafts 07-12** (Use the option `protocolVersion: 8`)
- **HyBi drafts 13-17** (Current default, alternatively option
  `protocolVersion: 13`)

## Installing

```
npm install ws
```

### Opt-in for performance

[bufferutil][] is an optional module that can be installed alongside the ws
module:

```
npm install --save-optional bufferutil
```

This is a binary addon that improves the performance of certain operations such
as masking and unmasking the data payload of the WebSocket frames. Prebuilt
binaries are available for the most popular platforms, so you don't necessarily
need to have a C++ compiler installed on your machine.

To force ws to not use bufferutil, use the
[`WS_NO_BUFFER_UTIL`](./doc/ws.md#ws_no_buffer_util) environment variable. This
can be useful to enhance security in systems where a user can put a package in
the package search path of an application of another user, due to how the
Node.js resolver algorithm works.

#### Legacy opt-in for performance

If you are running on an old version of Node.js (prior to v18.14.0), ws also
supports the [utf-8-validate][] module:

```
npm install --save-optional utf-8-validate
```

This contains a binary polyfill for [`buffer.isUtf8()`][].

To force ws not to use utf-8-validate, use the
[`WS_NO_UTF_8_VALIDATE`](./doc/ws.md#ws_no_utf_8_validate) environment variable.

## API docs

See [`/doc/ws.md`](./doc/ws.md) for Node.js-like documentation of ws classes and
utility functions.

## WebSocket compression

ws supports the [permessage-deflate extension][permessage-deflate] which enables
the client and server to negotiate a compression algorithm and its parameters,
and then selectively apply it to the data payloads of each WebSocket message.

The extension is disabled by default on the server and enabled by default on the
client. It adds a significant overhead in terms of performance and memory
consumption so we suggest to enable it only if it is really needed.

Note that Node.js has a variety of issues with high-performance compression,
where increased concurrency, especially on Linux, can lead to [catastrophic
memory fragmentation][node-zlib-bug] and slow performance. If you intend to use
permessage-deflate in production, it is worthwhile to set up a test
representative of your workload and ensure Node.js/zlib will handle it with
acceptable performance and memory usage.

Tuning of permessage-deflate can be done via the options defined below. You can
also use `zlibDeflateOptions` and `zlibInflateOptions`, which is passed directly
into the creation of [raw deflate/inflate streams][node-zlib-deflaterawdocs].

See [the docs][ws-server-options] for more options.

```js
import WebSocket, { WebSocketServer } from 'ws';

const wss = new WebSocketServer({
  port: 8080,
  perMessageDeflate: {
    zlibDeflateOptions: {
      // See zlib defaults.
      chunkSize: 1024,
      memLevel: 7,
      level: 3
    },
    zlibInflateOptions: {
      chunkSize: 10 * 1024
    },
    // Other options settable:
    clientNoContextTakeover: true, // Defaults to negotiated value.
    serverNoContextTakeover: true, // Defaults to negotiated value.
    serverMaxWindowBits: 10, // Defaults to negotiated value.
    // Below options specified as default values.
    concurrencyLimit: 10, // Limits zlib concurrency for perf.
    threshold: 1024 // Size (in bytes) below which messages
    // should not be compressed if context takeover is disabled.
  }
});
```

The client will only use the extension if it is supported and enabled on the
server. To always disable the extension on the client, set the
`perMessageDeflate` option to `false`.

```js
import WebSocket from 'ws';

const ws = new WebSocket('ws://www.host.com/path', {
  perMessageDeflate: false
});
```

## Usage examples

### Sending and receiving text data

```js
import WebSocket from 'ws';

const ws = new WebSocket('ws://www.host.com/path');

ws.on('error', console.error);

ws.on('open', function open() {
  ws.send('something');
});

ws.on('message', function message(data) {
  console.log('received: %s', data);
});
```

### Sending binary data

```js
import WebSocket from 'ws';

const ws = new WebSocket('ws://www.host.com/path');

ws.on('error', console.error);

ws.on('open', function open() {
  const array = new Float32Array(5);

  for (var i = 0; i < array.length; ++i) {
    array[i] = i / 2;
  }

  ws.send(array);
});
```

### Simple server

```js
import { WebSocketServer } from 'ws';

const wss = new WebSocketServer({ port: 8080 });

wss.on('connection', function connection(ws) {
  ws.on('error', console.error);

  ws.on('message', function message(data) {
    console.log('received: %s', data);
  });

  ws.send('something');
});
```

### External HTTP/S server

```js
import { createServer } from 'https';
import { readFileSync } from 'fs';
import { WebSocketServer } from 'ws';

const server = createServer({
  cert: readFileSync('/path/to/cert.pem'),
  key: readFileSync('/path/to/key.pem')
});
const wss = new WebSocketServer({ server });

wss.on('connection', function connection(ws) {
  ws.on('error', console.error);

  ws.on('message', function message(data) {
    console.log('received: %s', data);
  });

  ws.send('something');
});

server.listen(8080);
```

### Multiple servers sharing a single HTTP/S server

```js
import { createServer } from 'http';
import { WebSocketServer } from 'ws';

const server = createServer();
const wss1 = new WebSocketServer({ noServer: true });
const wss2 = new WebSocketServer({ noServer: true });

wss1.on('connection', function connection(ws) {
  ws.on('error', console.error);

  // ...
});

wss2.on('connection', function connection(ws) {
  ws.on('error', console.error);

  // ...
});

server.on('upgrade', function upgrade(request, socket, head) {
  const { pathname } = new URL(request.url, 'wss://base.url');

  if (pathname === '/foo') {
    wss1.handleUpgrade(request, socket, head, function done(ws) {
      wss1.emit('connection', ws, request);
    });
  } else if (pathname === '/bar') {
    wss2.handleUpgrade(request, socket, head, function done(ws) {
      wss2.emit('connection', ws, request);
    });
  } else {
    socket.destroy();
  }
});

server.listen(8080);
```

### Client authentication

```js
import { createServer } from 'http';
import { WebSocketServer } from 'ws';

function onSocketError(err) {
  console.error(err);
}

const server = createServer();
const wss = new WebSocketServer({ noServer: true });

wss.on('connection', function connection(ws, request, client) {
  ws.on('error', console.error);

  ws.on('message', function message(data) {
    console.log(`Received message ${data} from user ${client}`);
  });
});

server.on('upgrade', function upgrade(request, socket, head) {
  socket.on('error', onSocketError);

  // This function is not defined on purpose. Implement it with your own logic.
  authenticate(request, function next(err, client) {
    if (err || !client) {
      socket.write('HTTP/1.1 401 Unauthorized\r\n\r\n');
      socket.destroy();
      return;
    }

    socket.removeListener('error', onSocketError);

    wss.handleUpgrade(request, socket, head, function done(ws) {
      wss.emit('connection', ws, request, client);
    });
  });
});

server.listen(8080);
```

Also see the provided [example][session-parse-example] using `express-session`.

### Server broadcast

A client WebSocket broadcasting to all connected WebSocket clients, including
itself.

```js
import WebSocket, { WebSocketServer } from 'ws';

const wss = new WebSocketServer({ port: 8080 });

wss.on('connection', function connection(ws) {
  ws.on('error', console.error);

  ws.on('message', function message(data, isBinary) {
    wss.clients.forEach(function each(client) {
      if (client.readyState === WebSocket.OPEN) {
        client.send(data, { binary: isBinary });
      }
    });
  });
});
```

A client WebSocket broadcasting to every other connected WebSocket clients,
excluding itself.

```js
import WebSocket, { WebSocketServer } from 'ws';

const wss = new WebSocketServer({ port: 8080 });

wss.on('connection', function connection(ws) {
  ws.on('error', console.error);

  ws.on('message', function message(data, isBinary) {
    wss.clients.forEach(function each(client) {
      if (client !== ws && client.readyState === WebSocket.OPEN) {
        client.send(data, { binary: isBinary });
      }
    });
  });
});
```

### Round-trip time

```js
import WebSocket from 'ws';

const ws = new WebSocket('wss://websocket-echo.com/');

ws.on('error', console.error);

ws.on('open', function open() {
  console.log('connected');
  ws.send(Date.now());
});

ws.on('close', function close() {
  console.log('disconnected');
});

ws.on('message', function message(data) {
  console.log(`Round-trip time: ${Date.now() - data} ms`);

  setTimeout(function timeout() {
    ws.send(Date.now());
  }, 500);
});
```

### Use the Node.js streams API

```js
import WebSocket, { createWebSocketStream } from 'ws';

const ws = new WebSocket('wss://websocket-echo.com/');

const duplex = createWebSocketStream(ws, { encoding: 'utf8' });

duplex.on('error', console.error);

duplex.pipe(process.stdout);
process.stdin.pipe(duplex);
```

### Other examples

For a full example with a browser client communicating with a ws server, see the
examples folder.

Otherwise, see the test cases.

## FAQ

### How to get the IP address of the client?

The remote IP address can be obtained from the raw socket.

```js
import { WebSocketServer } from 'ws';

const wss = new WebSocketServer({ port: 8080 });

wss.on('connection', function connection(ws, req) {
  const ip = req.socket.remoteAddress;

  ws.on('error', console.error);
});
```

When the server runs behind a proxy like NGINX, the de-facto standard is to use
the `X-Forwarded-For` header.

```js
wss.on('connection', function connection(ws, req) {
  const ip = req.headers['x-forwarded-for'].split(',')[0].trim();

  ws.on('error', console.error);
});
```

### How to detect and close broken connections?

Sometimes, the link between the server and the client can be interrupted in a
way that keeps both the server and the client unaware of the broken state of the
connection (e.g. when pulling the cord).

In these cases, ping messages can be used as a means to verify that the remote
endpoint is still responsive.

```js
import { WebSocketServer } from 'ws';

function heartbeat() {
  this.isAlive = true;
}

const wss = new WebSocketServer({ port: 8080 });

wss.on('connection', function connection(ws) {
  ws.isAlive = true;
  ws.on('error', console.error);
  ws.on('pong', heartbeat);
});

const interval = setInterval(function ping() {
  wss.clients.forEach(function each(ws) {
    if (ws.isAlive === false) return ws.terminate();

    ws.isAlive = false;
    ws.ping();
  });
}, 30000);

wss.on('close', function close() {
  clearInterval(interval);
});
```

Pong messages are automatically sent in response to ping messages as required by
the spec.

Just like the server example above, your clients might as well lose connection
without knowing it. You might want to add a ping listener on your clients to
prevent that. A simple implementation would be:

```js
import WebSocket from 'ws';

function heartbeat() {
  clearTimeout(this.pingTimeout);

  // Use `WebSocket#terminate()`, which immediately destroys the connection,
  // instead of `WebSocket#close()`, which waits for the close timer.
  // Delay should be equal to the interval at which your server
  // sends out pings plus a conservative assumption of the latency.
  this.pingTimeout = setTimeout(() => {
    this.terminate();
  }, 30000 + 1000);
}

const client = new WebSocket('wss://websocket-echo.com/');

client.on('error', console.error);
client.on('open', heartbeat);
client.on('ping', heartbeat);
client.on('close', function clear() {
  clearTimeout(this.pingTimeout);
});
```

### How to connect via a proxy?

Use a custom `http.Agent` implementation like [https-proxy-agent][] or
[socks-proxy-agent][].

## Changelog

We're using the GitHub [releases][changelog] for changelog entries.

## License

[MIT](LICENSE)

[`buffer.isutf8()`]: https://nodejs.org/api/buffer.html#bufferisutf8input
[bufferutil]: https://github.com/websockets/bufferutil
[changelog]: https://github.com/websockets/ws/releases
[client-report]: http://websockets.github.io/ws/autobahn/clients/
[https-proxy-agent]: https://github.com/TooTallNate/node-https-proxy-agent
[node-zlib-bug]: https://github.com/nodejs/node/issues/8871
[node-zlib-deflaterawdocs]:
  https://nodejs.org/api/zlib.html#zlib_zlib_createdeflateraw_options
[permessage-deflate]: https://tools.ietf.org/html/rfc7692
[server-report]: http://websockets.github.io/ws/autobahn/servers/
[session-parse-example]: ./examples/express-session-parse
[socks-proxy-agent]: https://github.com/TooTallNate/node-socks-proxy-agent
[utf-8-validate]: https://github.com/websockets/utf-8-validate
[ws-server-options]: ./doc/ws.md#new-websocketserveroptions-callback
```

### 文件: `scripts/start.py`

```python
import os
import pathlib
import subprocess
import sys


def pause_and_exit(prompt: str = "", exit_code: int = 1):
    input(prompt)
    exit(exit_code)


def check_python_venv():
    cwd = os.getcwd()

    # check current dir is project root
    if cwd != os.path.dirname(os.path.dirname(os.path.abspath(__file__))):
        pause_and_exit("请在LingChat目录下运行此脚本! 执行 python scripts/start.py")

    venv_dir = os.path.join(cwd, ".venv")
    if not os.path.exists(venv_dir):
        # create virtual environment
        input(f"没有找到虚拟环境, 按任意键创建虚拟环境.")
        if subprocess.check_call(["python", "-m", "venv", ".venv"]) != 0:
            pause_and_exit("创建虚拟环境失败")

        # install dependencies
        install_res = subprocess.check_call(
            ["python", "-m", "pip", "install", "-r", "requirements.log"]
        )

        if install_res != 0:
            pause_and_exit("安装依赖项失败.")

        pause_and_exit("安装完成, 请重新执行此脚本.", 0)

    # Get the current Python executable path
    python_executable = sys.executable

    if not python_executable.startswith(venv_dir):
        pause_and_exit(
            "请在虚拟环境中运行此脚本! 执行 .venv/Scripts/activate 以激活虚拟环境."
        )


def check_node_env():
    try:
        output = subprocess.check_output(["node", "-v"])
        print("Node.js is installed and available")
        print(output.decode("utf-8").strip())
    except FileNotFoundError:
        pause_and_exit("需要安装Node.js")

    assert os.path.exists("frontend"), "找不到 frontend 目录!"

    if not os.path.exists("frontend/node_modules"):
        print("正在安装 node 所需要的包.")
        subprocess.check_call(["npm", "install"], cwd="frontend")


def get_runtime_log_dir():
    return pathlib.Path("runtime_logs")


def check_runtime_logging_env():
    log_dir = get_runtime_log_dir()
    os.makedirs(log_dir, exist_ok=True)


def start_vits():
    if not os.path.exists("third_party/vits-simple-api-windows-cpu-v0.6.16"):
        pause_and_exit("找不到 vits !")

    fout_path = os.path.join(get_runtime_log_dir(), "vits_out.log")
    ferr_path = os.path.join(get_runtime_log_dir(), "vits_err.log")
    with open(fout_path, "w") as fout, open(ferr_path, "w") as ferr:
        subprocess.Popen(
            [
                "third_party/vits-simple-api-windows-cpu-v0.6.16/py310/python.exe",
                "app.py",
            ],
            cwd="third_party/vits-simple-api-windows-cpu-v0.6.16",
            stdout=fout,
            stderr=ferr,
        )


def start_backend():
    fout_path = os.path.join(get_runtime_log_dir(), "backend_out.log")
    ferr_path = os.path.join(get_runtime_log_dir(), "backend_err.log")

    with open(fout_path, "w") as fout, open(ferr_path, "w") as ferr:
        subprocess.Popen(
            [
                "python",
                "backend/webChat.windows.py",  # TODO: remove 'windows' suffix in file.
            ],
            stdout=fout,
            stderr=ferr,
        )


def start_frontend():
    fout_path = os.path.join(get_runtime_log_dir(), "frontend_out.log")
    ferr_path = os.path.join(get_runtime_log_dir(), "frontend_err.log")

    with open(fout_path, "w") as fout, open(ferr_path, "w") as ferr:
        subprocess.Popen(["node", "frontend/server.js"], stdout=fout, stderr=ferr)


def main():
    check_python_venv()
    check_node_env()
    check_runtime_logging_env()
    start_vits()
    start_backend()
    start_frontend()


if __name__ == "__main__":
    main()
```

### 文件: `third_party/logger_new.py`

```python
import logging
import sys
import time
import threading
from datetime import datetime
import os

# 日志配置
ENABLE_FILE_LOGGING = True  # 是否启用文件日志记录
LOG_FILE_DIRECTORY = "run_logs"  # 日志文件存储的相对目录
LOG_FILE_LEVEL = logging.DEBUG # 可以设置为 logging.INFO, logging.WARNING, logging.ERROR

ANIMATION_STYLES = {
    'braille': ['⢿', '⣻', '⣽', '⣾', '⣷', '⣯', '⣟', '⡿'],
    'spinner': ['-', '\\', '|', '/'],
    'dots': ['.  ', '.. ', '...', ' ..', '  .', '   '],
    'arrows': ['←', '↖', '↑', '↗', '→', '↘', '↓', '↙'],
    'moon': ['🌑', '🌒', '🌓', '🌔', '🌕', '🌖', '🌗', '🌘'],
    'clock': ['🕛', '🕐', '🕑', '🕒', '🕓', '🕔', '🕕', '🕖', '🕗', '🕘', '🕙', '🕚'],
    'directional_arrows_unicode': ['⬆️', '↗️', '➡️', '↘️', '⬇️', '↙️', '⬅️', '↖️'],
    'traffic_lights': ['🔴', '🟡', '🟢'],
    'growth_emoji': ['🌱', '🌿', '🌳'],
    'weather_icons': ['☀️', '☁️', '🌧️', '⚡️'],
    'heartbeat': ['♡', '♥'],
}

sys.stderr.flush()
def wcswidth(s):
    """回退 wcswidth, 将非 ASCII 字符视为宽度2。"""
    if not isinstance(s, str):
         return len(s) if s else 0
    length = 0
    for char_ in s:
        if ord(char_) < 128:
            length += 1
        else:
            length += 2
    return length


class TermColors:
    GREY = '\033[90m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    RED = '\033[91m'
    BLUE = '\033[94m'
    RESET = '\033[0m'
    WHITE = '\033[97m'
    CYAN = '\033[96m'
    MAGENTA = '\033[95m'
    LIGHT_BLUE = '\033[94m'
    ORANGE = '\033[38;5;208m'

_logger = None
_animation_thread = None
_stop_animation_event = threading.Event()

_is_animating = False
_current_animation_line_width = 0
_animation_state_lock = threading.Lock()

DEFAULT_ANIMATION_STYLE_KEY = 'braille'
DEFAULT_ANIMATION_COLOR = TermColors.WHITE

class AnimationAwareStreamHandler(logging.StreamHandler):
    def emit(self, record):
        global _is_animating, _current_animation_line_width, _animation_state_lock

        if hasattr(record, 'is_animation_control') and record.is_animation_control:
            super().emit(record)
            return

        with _animation_state_lock:
            is_currently_animating = _is_animating
            animation_width_to_clear = _current_animation_line_width

        if is_currently_animating and animation_width_to_clear > 0:
            self.acquire()
            try:
                self.flush()
                self.stream.write("\r" + " " * animation_width_to_clear + "\r")
                self.stream.flush()
            finally:
                self.release()

        super().emit(record)

class ColoredFormatter(logging.Formatter):
    DATE_FORMAT = "%Y-%m-%d-%H:%M:%S"

    def __init__(self, show_timestamp=True):
        super().__init__(datefmt=self.DATE_FORMAT)
        self.show_timestamp = show_timestamp

    def format(self, record):
        if hasattr(record, 'is_animation_control') and record.is_animation_control:
            return record.getMessage()

        timestamp_part = ""
        if self.show_timestamp:
            timestamp_str = self.formatTime(record, self.DATE_FORMAT)
            timestamp_part = f"{timestamp_str} "

        message_content = record.getMessage()
        level_name = record.levelname
        level_prefix_text = f"[{level_name}]: "

        if record.levelno == logging.DEBUG:
            return f"{TermColors.GREY}{timestamp_part}{level_prefix_text}{message_content}{TermColors.RESET}"

        level_color = ""
        if record.levelno == logging.INFO:
            level_color = TermColors.GREEN
        elif record.levelno == logging.WARNING:
            level_color = TermColors.YELLOW
        elif record.levelno == logging.ERROR:
            level_color = TermColors.RED

        colored_level_prefix = f"{level_color}{level_prefix_text}{TermColors.RESET}"
        return f"{timestamp_part}{colored_level_prefix}{message_content}"

def _animate(message="Loading", animation_chars=None, color_code=DEFAULT_ANIMATION_COLOR):
    global _is_animating, _current_animation_line_width, _animation_state_lock, _stop_animation_event

    if animation_chars is None:
        animation_chars = ANIMATION_STYLES[DEFAULT_ANIMATION_STYLE_KEY]

    idx = 0
    last_char_for_clear = animation_chars[0]

    while not _stop_animation_event.is_set():
        char = animation_chars[idx % len(animation_chars)]
        last_char_for_clear = char

        visible_animation_text = f"{message} {char} "
        current_width = wcswidth(visible_animation_text)

        with _animation_state_lock:
            _current_animation_line_width = current_width

        sys.stdout.write(f"\r{color_code}{message} {char}{TermColors.RESET} ")
        sys.stdout.flush()

        idx += 1
        time.sleep(0.12)

    final_visible_text = f"{message} {last_char_for_clear} "
    width_to_clear = wcswidth(final_visible_text)

    sys.stdout.write("\r" + " " * width_to_clear + "\r")
    sys.stdout.flush()

    with _animation_state_lock:
        _is_animating = False
        _current_animation_line_width = 0

def start_loading_animation(message="Processing",
                            animation_style_key=DEFAULT_ANIMATION_STYLE_KEY,
                            animation_color=DEFAULT_ANIMATION_COLOR):
    global _animation_thread, _stop_animation_event, _is_animating, _animation_state_lock

    with _animation_state_lock:
        if _is_animating:
            return
        _is_animating = True

    _stop_animation_event.clear()

    selected_chars = ANIMATION_STYLES.get(animation_style_key, ANIMATION_STYLES[DEFAULT_ANIMATION_STYLE_KEY])

    _animation_thread = threading.Thread(target=_animate,
                                         args=(message, selected_chars, animation_color),
                                         daemon=True)
    _animation_thread.start()

def stop_loading_animation(success=True, final_message=None):
    global _animation_thread, _stop_animation_event, _is_animating, _animation_state_lock

    acquire_lock = False
    with _animation_state_lock:
        if _is_animating or _animation_thread is not None:
            acquire_lock = True

    if not acquire_lock:
        return

    _stop_animation_event.set()

    current_thread_ref = _animation_thread
    if current_thread_ref and current_thread_ref.is_alive():
        current_thread_ref.join(timeout=2)

    with _animation_state_lock:
        _is_animating = False
        _current_animation_line_width = 0
        _animation_thread = None

    if final_message:
        if success:
            log_info(f"{TermColors.GREEN}✔{TermColors.RESET} {final_message}")
        else:
            log_error(f"{TermColors.RED}✖{TermColors.RESET} {final_message}")

def initialize_logger(app_name="AppLogger", config_debug_mode=True, show_timestamp=True):
    global _logger
    _logger = logging.getLogger(app_name)
    _logger.propagate = False

    if config_debug_mode:
        _logger.setLevel(logging.DEBUG)
    else:
        _logger.setLevel(logging.INFO)

    if _logger.hasHandlers():
        for handler in _logger.handlers[:]:
            _logger.removeHandler(handler)
            handler.close()

    console_handler = AnimationAwareStreamHandler(sys.stdout)
    console_formatter = ColoredFormatter(show_timestamp=show_timestamp)
    console_handler.setFormatter(console_formatter)
    _logger.addHandler(console_handler)

    if ENABLE_FILE_LOGGING:
        try:
            if not os.path.exists(LOG_FILE_DIRECTORY):
                os.makedirs(LOG_FILE_DIRECTORY, exist_ok=True)

            log_filename = datetime.now().strftime("%Y-%m-%d_%H-%M-%S.log")
            log_filepath = os.path.join(LOG_FILE_DIRECTORY, log_filename)

            file_handler = logging.FileHandler(log_filepath, encoding='utf-8')

            file_formatter = logging.Formatter(
                '%(asctime)s - %(levelname)s - %(name)s - %(message)s',
                datefmt=ColoredFormatter.DATE_FORMAT
            )
            file_handler.setFormatter(file_formatter)

            file_handler.setLevel(LOG_FILE_LEVEL)

            _logger.addHandler(file_handler)

        except Exception as e:
            sys.stderr.write(
                f"{TermColors.RED}错误: 初始化文件日志记录失败: {e}{TermColors.RESET}\n"
            )
            sys.stderr.flush()

    return _logger

def get_logger():
    if _logger is None:
        sys.stderr.write(
            f"{TermColors.YELLOW}警告: 日志记录器在显式初始化之前被访问。 "
            f"将使用默认值进行初始化。{TermColors.RESET}\n"
        )
        sys.stderr.flush()
        initialize_logger()
    return _logger

def log_debug(message, *args, **kwargs): get_logger().debug(message, *args, **kwargs)
def log_info(message, *args, **kwargs): get_logger().info(message, *args, **kwargs)
def log_warning(message, *args, **kwargs): get_logger().warning(message, *args, **kwargs)
def log_error(message, *args, **kwargs): get_logger().error(message, *args, **kwargs)

def log_info_color(message, color_code=TermColors.GREEN, *args, **kwargs):
    get_logger().info(f"{color_code}{message}{TermColors.RESET}", *args, **kwargs)

def log_warning_color(message, color_code=TermColors.YELLOW, *args, **kwargs):
    get_logger().warning(f"{color_code}{message}{TermColors.RESET}", *args, **kwargs)

def log_error_color(message, color_code=TermColors.RED, *args, **kwargs):
    get_logger().error(f"{color_code}{message}{TermColors.RESET}", *args, **kwargs)

def log_rag_output(message, *args, **kwargs):
    get_logger().info(f"{TermColors.BLUE}{message}{TermColors.RESET}", *args, **kwargs)

# --- 使用示例 ---
if __name__ == "__main__":

    # 1. 初始化日志记录器
    initialize_logger(app_name="演示应用", config_debug_mode=True, show_timestamp=True)
    log_info("=============== 炫彩日志与加载动画演示开始 ===============")
    log_debug("这是一个调试消息：日志系统已成功初始化。")
    if not ENABLE_FILE_LOGGING:
        log_warning("文件日志记录已禁用。如需启用，请设置 ENABLE_FILE_LOGGING = True")
    else:
        log_info(f"文件日志已启用，日志将存储在 '{LOG_FILE_DIRECTORY}' 目录下。")

    # 2. 基本日志级别演示
    log_info("演示2.1: log_info是一条 INFO 信息。")
    log_warning("演示2.2: log_warning是一条警告 WARNING 信息。")
    log_error("演示2.3: log_error是一条错误 ERROR 信息。")
    log_debug("演示2.4: log_debug是一条调试 DEBUG 信息。DEBUG信息（包括对应时间戳）全部保持灰色")
    log_info_color("演示2.5: log_info_color的 INFO 信息带有醒目的绿色。")
    log_warning_color("演示2.6: log_warning_color的 WARNING 信息带有醒目的黄色。")
    log_error_color("演示2.7: log_error_color的 ERROR 信息带有醒目的红色。")

    # 3. 加载动画演示
    log_info("演示3.1: 默认加载动画 (braille样式, 白色)")
    start_loading_animation(message="任务A处理中")
    time.sleep(2)
    stop_loading_animation(success=True, final_message="任务A成功完成!")

    log_info("演示3.2: 自定义动画样式 (spinner样式, 默认白色)")
    start_loading_animation(message="任务B执行中", animation_style_key='spinner')
    time.sleep(2)
    stop_loading_animation(success=True, final_message="任务B (spinner) 执行完毕!")

    log_info("演示3.3: 自定义动画颜色 (默认braille样式, 青色)")
    start_loading_animation(message="任务C加载中", animation_color=TermColors.CYAN)
    time.sleep(2)
    stop_loading_animation(success=True, final_message="任务C (青色) 加载完成!")

    log_info("演示3.4: 自定义样式与颜色 (arrows样式, 品红色)")
    start_loading_animation(message="任务D进行中", animation_style_key='arrows', animation_color=TermColors.MAGENTA)
    time.sleep(2.5)
    stop_loading_animation(success=True, final_message="任务D (品红箭头) 完成!")

    log_info("演示3.5: 其他动画样式 (moon样式, 浅蓝色)")
    start_loading_animation(message="月相观察", animation_style_key='moon', animation_color=TermColors.LIGHT_BLUE)
    time.sleep(2.5)
    stop_loading_animation(success=True, final_message="月相观察完毕!")

    log_info("演示3.6: 动画期间进行日志记录 (dots样式, 橙色)")
    start_loading_animation(message="橙色点点任务", animation_style_key='dots', animation_color=TermColors.ORANGE)
    log_info("动画已启动，现在记录一条 INFO 消息，动画会自动避让。")
    time.sleep(1)
    log_warning("这是一条警告 WARNING 消息，动画仍在后台继续。")
    time.sleep(1)
    log_debug("一条调试 DEBUG 消息，动画即将停止并模拟失败。")
    time.sleep(1)
    stop_loading_animation(success=False, final_message="橙色点点任务模拟失败。使用success=False 会显示红叉")

    log_info("演示3.7: 停止动画时不显示最终消息")
    start_loading_animation(message="短暂处理")
    time.sleep(1.5)
    stop_loading_animation()
    log_info("动画已停止，不提供 final_message，则 stop_loading_animation 不输出额外消息。")

    # 4. 特殊颜色日志函数
    log_info("演示4.1: 使用 log_info_color 输出自定义颜色 INFO (例如紫红色)")
    log_info_color("这是一条紫红色的 INFO 信息。", color_code=TermColors.MAGENTA)

    log_info("演示4.2: 使用 log_rag_output 输出特定格式 INFO 作为自定义log函数的示范")
    log_rag_output("这是一个使用log_rag_output输出的，模拟的 RAG 模型输出内容")

    # 5. 重新初始化日志记录器：关闭控制台时间戳
    log_info("演示5: 重新初始化日志，关闭控制台时间戳 (文件日志不受影响)。重新初始化会基于当前时间创建新的日志文件（如果文件名是基于时间的）")
    initialize_logger(app_name="演示应用-无时间戳", config_debug_mode=True, show_timestamp=False)
    log_info("这条 INFO 信息在控制台不显示时间戳。")
    log_debug("这条 DEBUG 信息在控制台也不显示时间戳。")
    start_loading_animation(message="无时间戳任务执行")
    time.sleep(1.5)
    stop_loading_animation(final_message="无时间戳任务完成。")
    log_info("控制台时间戳已关闭，文件日志中的时间戳格式依然由 file_formatter 控制。")

    # 6. 恢复时间戳并测试与 print() 的交互
    log_info("演示6: 恢复时间戳并测试动画与普通 print() 语句的交互")
    initialize_logger(app_name="演示应用", config_debug_mode=True, show_timestamp=True) # 恢复默认配置
    log_info("日志时间戳已恢复。")

    print(f"{TermColors.YELLOW}这是一条普通的 print() 语句，在动画开始前。{TermColors.RESET}")
    start_loading_animation(message="错误的动画与print交互写法")
    time.sleep(1)
    print(f"{TermColors.RED}错误示范: 下面这条 print() 语句会打断当前动画行，因为它直接写入stdout并通常会换行。只处理 logging 模块发出的日志，无法拦截 print()，不能正确关闭演示动画。{TermColors.RESET}")
    time.sleep(1)
    log_info("这条日志消息在 print() 之后，会由 AnimationAwareStreamHandler 正确处理，先清空动画行再输出。")
    time.sleep(1)
    stop_loading_animation(final_message="动画与 print() 交互测试结束。")
    print(f"{TermColors.GREEN}动画结束后的另一条 print() 语句，可以正常显示。{TermColors.RESET}")

    # 7. 结束
    if ENABLE_FILE_LOGGING:
        log_info(f"所有演示已完成。请检查 '{LOG_FILE_DIRECTORY}' 目录中的日志文件。")
    else:
        log_info("所有演示已完成。文件日志记录当前已禁用。")
    log_info("=============== 演示结束 ===============")
```
