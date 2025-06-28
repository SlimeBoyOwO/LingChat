import eventListener from "./event-bus.js"
class Request {
  constructor() {
    this.fetch = (url, options = {}) => {
      return fetch(url, options)
      .catch(error => {
        console.error(error)
      })
      .then(message => {
        if (message.ok) {
          return message.json()
        } else {
          throw message.json()
        }
      })
    }
    this.send = (method, url, information) => {
      if (method === 'GET') {
        url = new URL(url, window.location)
        const search = url.searchParams
        for (const key in information) {
          search.append(key, information[key])
        }
        return this.fetch(url)
      }
      if (method === 'POST') {
        return this.fetch(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(information),
        })
      }
      if (method === 'DELETE') {
        url = new URL(url, window.location)
        const search = url.searchParams
        for (const key in information) {
          search.append(key, information[key])
        }
        return this.fetch(url, {method: 'DELETE'})
      }
    }
  }
  
  configGet() {
    return this.send(
      'GET',
      '/api/settings/config',
      {}
    )
    .then(result => {
      return result
    })
    .catch(result => {
      throw new Error('无法加载配置')
    })
  }
  configSet(formData) {
    return this.send(
      'POST',
      '/api/settings/config',
      formData
    )
    .then(result => {
      return
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }
  characterGetAll() {
    return this.send(
      'GET',
      '/api/v1/chat/character/get_all_characters',
      {}
    )
    .then(result => {
      return result.data
    })
    .catch(result => {
      throw new Error(result.message)
    })
  }
  characterSelect(userId, characterId) {
    return this.send(
      'POST',
      '/api/v1/chat/character/select_character',
      {
        user_id: userId,
        character_id: characterId,
      }
    )
    .then(result => {
      return result.character
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }


  historyList(userId, page, pageSize) {
    return this.send(
      'GET',
      '/api/v1/chat/history/list',
      {
        user_id: userId,
        page: page,
        page_size: pageSize,
      }
    )
    .then(result => {
      return result.data.conversations
    })
    .catch(result => {
      throw new Error(`${result.msg}:${result.error}`)
    })
  }

  historyLoad(userId, convoId) {
    return this.send(
      'GET',
      '/api/v1/chat/history/load',
      {
        user_id: userId,
        conversation_id: convoId,
      }
    )
    .then(result => {
      return result.data
    })
    .catch(result => {
      throw new Error(`${result.msg}:${result.error}`)
    })
  }

  historySave(userId, convoId) {
    return this.send(
      'POST',
      '/api/v1/chat/history/save',
      {
        user_id: userId,
        conversation_id: convoId,
      }
    )
    .then(result => {
      return result.data.conversation_id
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }
  
  historyDelete(userId, convoId) {
    return this.send(
      'POST',
      '/api/v1/chat/history/delete',
      {
        user_id: userId,
        conversation_id: convoId,
      }
    )
    .then(result => {
      return result.data.conversation_id
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }

  historyCreate(userId, title) {
    return this.send(
      'POST',
      '/api/v1/chat/history/create',
      {
        user_id: userId,
        title: title,
      }
    )
    .then(result => {
      return result.data.conversation_id
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }

  histortyInput(userId, text, fileName) {
    return this.send(
      'POST',
      '/api/v1/chat/history/process-log',
      {
        user_id: userId,
        content: text,
        filename: fileName,
      }
    )
    .catch(result => {
      throw new Error(result.detail)
    })
  }

  histortyOutput() {}

  informationGet(userId) {
    return this.send(
      'GET',
      '/api/v1/chat/info/init',
      {
        user_id: userId,
      }
    )
    .then(result => {
      return result.data
    })
    .catch(result => {
      throw new Error(`${result.msg}:${result.error}`)
    })
  }

  backmusicList() {
    return this.send(
      'GET',
      '/api/v1/chat/back-music/list',
      {}
    )
    .catch(result => {
      throw new Error(result.detail)
    })
  }

  backmusicUpload(formData) {
    return this.send(
      'POST',
      '/api/v1/chat/back-music/upload',
      formData
    )
    .then(result => {
      return
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }
  backmusicDelete(url) {
    return this.send(
      'DELETE',
      '/api/v1/chat/back-music/delete',
      {
        url: encodeURI(url),
      }
    )
    .then(result => {
      return
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }

  backgroundList() {
    return this.send(
      'GET',
      '/api/v1/chat/background/list',
      {}
    )
    .catch(result => {
      throw new Error(result.detail)
    })
  }
  backgroundUpload(formData) {
    return this.send(
      'POST',
      '/api/v1/chat/background/upload',
      formData
    )
    .then(result => {
      return
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }
  backgroundDelete(id) {
    return this.send(
      'DELETE',
      '/api/v1/chat/background/delete',
      {
        id: id,
      }
    )
    .then(result => {
      return
    })
    .catch(result => {
      throw new Error(result.detail)
    })
  }


}
const request = new Request()
export default request



const requestUtils = {
  praseFetchError: (error) => {
    switch(error) {
      case 'TypeError': return '网络无法连接'
      case 'DOMException': return '请求已被取消'
      case 'SyntaxError': return '响应错误解析'
      default: '未知请求错误'
    }
  },
  praseSaveError: (error) => {
    switch(error) {
      case '/list': return '存档列表读取失败'
      case '/load': return '存档加载失败'
      case '/save': return '存档保存失败'
      case '/delete': return '存档删除失败'
      case '/create': return '存档创建失败'
      case '/porcess-log': return '存档处理失败'
      default: '未知存档错误'
    }
  },
  praseSaveLoading: (error) => {
    switch(error) {
      case '/list': return '正在存档列表读取'
      case '/load': return '正在加载存档'
      case '/save': return '正在保存存档'
      case '/delete': return '正在删除存档'
      case '/create': return '正在创建存档'
      case '/porcess-log': return '正在处理存档'
      default: '未知存档事件'
    }
  },
  praseSaveSuccess: (error) => {
    switch(error) {
      case '/list': return '存档列表读取完成'
      case '/load': return '加载存档完成'
      case '/save': return '保存存档完成'
      case '/delete': return '删除存档完成'
      case '/create': return '创建存档完成'
      case '/porcess-log': return '处理存档完成'
      default: '未知存档事件'
    }
  },
}
