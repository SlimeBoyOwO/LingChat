import eventListener from "./event-bus.js"

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

export const request = {
  fetch: (apiVerison, commandType, command, query, options) => {
    return fetch(`${apiVerison}${commandType}${command}${query}`, options)
    .then(response => {
      return response.json()
    })
    .catch(error => {
      eventListener.emit("notice:error", requestUtils.praseFetchError(error.name))
      throw error
    })
    .then(result => {
      if (result.code === 200) {
        return result.data
      } else {
        throw new Error(result.message);
      };
    })
  },
  GET: (apiVerison, commandType, command, query) => {
    query = query.replace(/[\s\n\r]/g, '')
    return request.fetch(apiVerison, commandType, command, query, {})
  },
  POST: (apiVerison, commandType, command, body) => {
    return request.fetch(apiVerison, commandType, command, '', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    })
  },
  send: (requestType, apiVerison, commandType, command, information) => {
  
    if (requestType === 'GET') {
      return request.GET(apiVerison, commandType, command, information)
    } 
    if (requestType === 'POST') {
      return request.POST(apiVerison, commandType, command, information)
    }
  },
  v1Send: (requestType, commandType, command, information) => {
    return request.send(requestType, '/api/v1/chat', commandType, command, information)
  },

  saveSend: (requestType, command, information) => {
    if (command !== '/list') {
      eventListener.emit("notice:loading", requestUtils.praseSaveLoading(command))
    }
   return request.v1Send(requestType, "/history", command, information)
   .then(data => {
      if (command !== '/list') {
        eventListener.emit("notice:success", requestUtils.praseSaveSuccess(command))
      }
      return data
   })
   .catch(error => {
      if (command !== '/list') {
        eventListener.emit("notice:error", requestUtils.praseSaveError(command))
      }
      throw error
   })
  },
  saveList: (userId, page, pageSize) => {
    return request.saveSend('GET', '/list', `
      ?user_id=${userId}
      &page=${page}
      &page_size=${pageSize}
    `)
  },
  saveLoad: (userId, convoId) => {
    return request.saveSend('GET', '/load', `
      ?user_id=${userId}
      &conversation_id=${convoId}
    `)
  },
  saveSave: (userId, convoId) => {
    return request.saveSend('POST', '/save', {
      user_id: userId,
      conversation_id: convoId,
    })
  },
  saveDelete: (userId, convoId) => {
    return request.saveSend('POST', '/delete', {
      user_id: userId,
      conversation_id: convoId,
    })
  },
  saveCreate: (userId, title) => {
    return request.saveSend('POST', '/create', {
      user_id: userId,
      title: title,
    })
  },
  saveInput: (userId, text, file) => {
    return request.saveSend('POST', '/process-log', {
      filename: file.name,
      content: text,
      user_id: userId,
    })
  },
  // saveOutput: () => {
  //   return request.saveSend()
  // }

  infoSend: (requestType, command, information) => {
   return request.v1Send(requestType, "/info", command, information)
  },
  infoGet: (userId) =>  {
    return request.infoSend('GET', '', `
      ?user_id=${userId}
    `)
  },

  backmusicSend: (requestType, command, information) => {
   return request.v1Send(requestType, "/back-music", command, information)
  },
  backmusicList: () =>  {
    return request.infoSend('GET', 'list', '')
  },
}