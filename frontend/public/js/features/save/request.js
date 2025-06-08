import EventBus from "../../core/event-bus.js";

function praseFetchError(errorName) {
  switch (errorName) {
    case "TypeError":
      return "网络无法连接";
    case "DOMException":
      return "请求已被取消";
    case "SyntaxError":
      return "响应错误解析";
  }
}

function praseSaveError(command) {
  switch (command) {
    case "list":
      return "存档列表读取失败";
    case "load":
      return "存档加载失败";
    case "save":
      return "存档保存失败";
    case "delete":
      return "存档删除失败";
    case "create":
      return "存档创建失败";
    case "process-log":
      return "存档处理失败";
  }
}

function praseSaveSuccess(command) {
  switch (command) {
    case "list":
      return "存档列表读取成功";
    case "load":
      return "存档加载成功";
    case "save":
      return "存档保存成功";
    case "delete":
      return "存档删除成功";
    case "create":
      return "存档创建成功";
    case "process-log":
      return "存档处理成功";
  }
}

function praseSaveLoading(command) {
  switch (command) {
    case "list":
      return "正在读取存档列表";
    case "load":
      return "正在加载存档";
    case "save":
      return "正在保存存档";
    case "delete":
      return "正在删除存档";
    case "create":
      return "正在创建存档";
    case "process-log":
      return "正在处理存档";
  }
}

export const saveRequest = {
  send: (command, query, options) => {
    if (command !== "list") {
      EventBus.emit("notice:loading", praseSaveLoading(command));
    }
    return fetch(`/api/v1/chat/history/${command}${query}`, options)
      .then((response) => {
        return response.json();
      })
      .catch((error) => {
        EventBus.emit("notice:error", praseFetchError(error.name));
        throw error;
      })
      .then((result) => {
        if (result.code === 200) {
          if (command !== "list") {
            EventBus.emit("notice:success", praseSaveSuccess(command));
          }
          return result.data;
        } else {
          throw new Error(result.message || "错误");
        }
      })
      .catch((error) => {
        EventBus.emit("notice:error", praseSaveError(command));
        throw error;
      });
  },
  sendGET: (command, query) => {
    query = query.replace(/[\s\n\r\t]/g, "");
    return saveRequest.send(command, query, {});
  },
  sendPOST: (command, body) => {
    return saveRequest.send(command, "", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  },
  list: (userId, page, pageSize) => {
    return saveRequest.sendGET(
      "list",
      `
      ?user_id=${userId}
      &page=${page}
      &page_size=${pageSize}
    `
    );
  },
  load: (userId, convoId) => {
    return saveRequest.sendGET(
      "load",
      `
      ?user_id=${userId}
      &conversation_id=${convoId}
    `
    );
  },
  save: (userId, convoId) => {
    return saveRequest.sendPOST("save", {
      user_id: userId,
      conversation_id: convoId,
    });
  },
  delete: (userId, convoId) => {
    return saveRequest.sendPOST("delete", {
      user_id: userId,
      conversation_id: convoId,
    });
  },
  create: (userId, title) => {
    return saveRequest.sendPOST("create", {
      user_id: userId,
      title: title,
    });
  },
  processLog: (userId, text, file) => {
    return saveRequest.sendPOST("process-log", {
      filename: file.name,
      content: text,
      user_id: userId,
    });
  },
};
