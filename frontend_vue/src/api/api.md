## 自定义类型

#### ModelInfo

```json
{
    "chat": {
        "llm_provider": "string",
        "api_key": "string",
        "base_url": "string",
        "model_type": "string",
        "model_info": {
            //Only Ollama
            "base_url": "string",
            "model": "string",
            //Only LMStudio
            "model_type": "string",
            "base_url": "string",
            "api_key": "string|null",
            //Only Gemini
            "model_type": "string",
            "base_url": "string"
        }
    }, //webllm的model_info为null即可
    "visual": {
        "api_key": "string",
        "base_url": "string",
        "model": "string"
    },
    "translate": {
        "llm_provider": "string",
        "api_key": "string"
    }
}
```

#### CharacterCardCover

```json
{
    "id": "int",
    "cover": "string", //封面图片的url
    "title": "string",
    "description": "string"
}
```

#### CharacterAvatars

这部分具体都有什么差分我还不知道，总之是所有差分的 url 地址

```json
{
    "happy": "string|null",
    "sad": "string|null",
    ...
}
```

#### CharacterCard

```json
{
    "cover": "CharacterCardCover",
    "player_name": "string",
    "player_subtitle": "string",
    "ai_name": "string",
    "ai_subtitle": "string",
    "ai_model": "ModelInfo",
    "ai_avatars": "CharacterAvatars"
}
```

## 消息

> &lt;PLACEHOLDER&gt;是占位符,如"&lt;ID&gt;"应返回"1"或2

#### /api/user/info

POST:

```json
{
    "username": "string",
    "password": "string" //加密密码
}
```

RECV:

```json
{
    "id": "int",
    "name": "string",
    "auth_token": "string" //用户认证令牌
}
```

#### /api/user/settings/defaults

POST:

```json
{
    "id": "int",
    "auth_token": "string"
}
```

RECV:

```json
{
    "model": "ModelInfo"
}
```

#### /api/user/settings/settings

POST:

```json
{
    "id": "int",
    "auth_token": "string"
}
```

RECV:

```json
{
    "characterCard": "CharacterCard",
    "characterCards": "CharacterCardCover[]"
}
```

#### /api/card/character/cover

POST:

```json
{
    "id": "int[]"
}
```

RECV:

```json
{
    "<ID>": {
        "cover":"string",//封面图片的Url
        "title":"string",
        "description": "string"
    },
    "<ID>":...
}
```

#### /api/card/character/extend

POST:

```json
{
    "id": "int[]"
}
```

RECV:

```json
{
    "<ID>":{
        "player_name": "string",
        "player_subtitle": "string",
        "ai_name": "string",
        "ai_subtitle": "string",
        "ai_model": "ModelInfo|null", //null的时候会使用Defaults里的默认model设置
        "ai_avatars": "CharacterAvatars"
    },
    "<ID>":...
}
```

#### /api/card/character/full

POST:

```json
{
    "id": "int[]"
}
```

RECV:

```json
{
    "<ID>":{
        "cover":"CharacterCardCover",
        "player_name": "string",
        "player_subtitle": "string",
        "ai_name": "string",
        "ai_subtitle": "string",
        "ai_model": "ModelInfo|null", //null的时候会使用Defaults里的默认model设置
        "ai_avatars": "CharacterAvatars"
    },
    "<ID>":...
}
```

#### /api/card/character/search

POST:

```json
{
    "name": "string"
}
```

RECV:

```json
{
    "<ID>":"CharacterCardCover",
    ...
}
```
