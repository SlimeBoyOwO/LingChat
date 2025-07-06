# 模块导入
```
import request from '...../js/core/request.js'
```

此模块中所有函数均为异步函数返回一个异步对象Promise

此模块中所有异步函数正确运行会返回一个回调数据

此模块中所有异步函数错误运行会返回一个错误对象

## 角色
### request.characterGetAll()
获取所有角色信息列表

#### 参数：无

#### 回调：Promise异步对象

list：角色信息列表


### request.characterSelect(userId, characterId)
获取用户选择角色信息

#### 参数：
userId：当前用户的id

characterId：目标角色的id

#### 回调：Promise异步对象

character：角色信息对象

## 存档

### request.historyList(userId, page, pageSize)
获取全部存档列表

#### 参数：
userId：当前用户的id

page：获取存档的页码

pageSize：每一页的存档数量

#### 回调：Promise异步对象

list：获取到的存档列表

### request.historyLoad(userId, convoId)
根据id加载存档

#### 参数：
userId：当前用户的id

convoId：存档的id

#### 回调：Promise异步对象

data；存档数据

### request.historySave(userId, convoId)
保存数据到指定的存档

### request.historyDelete(userId, convoId)
删除指定的存档

### request.historyCreate(userId, title)
创建一个存档

### request.histortyInput(userId, text, fileName)
导入一个存档

## 个人信息

### request.informationGet(userId)
获取当前用户的所有信息

## 背景音乐

### backmusicList()
获取背景音乐列表

### request.backmusicUpload()
上传背景音乐

### request.backmusicDelete(url)
删除指定背景音乐