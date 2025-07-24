interface RequestOptions extends RequestInit {
  headers?: Record<string, string>;
}

interface RequestData {
  [key: string]: any;
}

type HttpMethod = "GET" | "POST" | "DELETE";

class Request {
  private async fetch(url: string, options: RequestOptions = {}): Promise<any> {
    try {
      const response = await fetch(url, options);
      if (response.ok) {
        return await response.json();
      } else {
        throw await response.json();
      }
    } catch (error) {
      console.error(error);
      throw error;
    }
  }

  private send(
    method: HttpMethod,
    url: string,
    information: RequestData = {}
  ): Promise<any> {
    if (method === "GET") {
      const urlObj = new URL(url, window.location.origin);
      const search = urlObj.searchParams;
      for (const key in information) {
        search.append(key, String(information[key]));
      }
      return this.fetch(urlObj.toString());
    }

    if (method === "POST") {
      return this.fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(information),
      });
    }

    if (method === "DELETE") {
      const urlObj = new URL(url, window.location.origin);
      const search = urlObj.searchParams;
      for (const key in information) {
        search.append(key, String(information[key]));
      }
      return this.fetch(urlObj.toString(), { method: "DELETE" });
    }

    throw new Error(`Unsupported HTTP method: ${method}`);
  }

  // 配置相关
  async configGet(): Promise<any> {
    try {
      return await this.send("GET", "/api/settings/config", {});
    } catch (result) {
      throw new Error("无法加载配置");
    }
  }

  async configSet(formData: RequestData): Promise<void> {
    try {
      await this.send("POST", "/api/settings/config", formData);
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  // 角色相关
  async characterGetAll(): Promise<any[]> {
    try {
      const result = await this.send(
        "GET",
        "/api/v1/chat/character/get_all_characters",
        {}
      );
      return result.data;
    } catch (result: any) {
      throw new Error(result.message);
    }
  }

  async characterSelect(userId: string, characterId: string): Promise<any> {
    try {
      const result = await this.send(
        "POST",
        "/api/v1/chat/character/select_character",
        {
          user_id: userId,
          character_id: characterId,
        }
      );
      return result.character;
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  // 历史记录相关
  async historyList(
    userId: string,
    page: number,
    pageSize: number
  ): Promise<any[]> {
    try {
      const result = await this.send("GET", "/api/v1/chat/history/list", {
        user_id: userId,
        page: page,
        page_size: pageSize,
      });
      return result.data.conversations;
    } catch (result: any) {
      throw new Error(`${result.msg}:${result.error}`);
    }
  }

  async historyLoad(userId: string, convoId: string): Promise<any> {
    try {
      const result = await this.send("GET", "/api/v1/chat/history/load", {
        user_id: userId,
        conversation_id: convoId,
      });
      return result.data;
    } catch (result: any) {
      throw new Error(`${result.msg}:${result.error}`);
    }
  }

  async historySave(userId: string, convoId: string): Promise<string> {
    try {
      const result = await this.send("POST", "/api/v1/chat/history/save", {
        user_id: userId,
        conversation_id: convoId,
      });
      return result.data.conversation_id;
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  async historyDelete(userId: string, convoId: string): Promise<string> {
    try {
      const result = await this.send("POST", "/api/v1/chat/history/delete", {
        user_id: userId,
        conversation_id: convoId,
      });
      return result.data.conversation_id;
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  async historyCreate(userId: string, title: string): Promise<string> {
    try {
      const result = await this.send("POST", "/api/v1/chat/history/create", {
        user_id: userId,
        title: title,
      });
      return result.data.conversation_id;
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  async historyInput(
    userId: string,
    text: string,
    fileName: string
  ): Promise<any> {
    try {
      const result = await this.send(
        "POST",
        "/api/v1/chat/history/process-log",
        {
          user_id: userId,
          content: text,
          filename: fileName,
        }
      );
      return result.data;
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  // 信息获取
  async informationGet(userId: string): Promise<any> {
    try {
      const result = await this.send("GET", "/api/v1/chat/info/init", {
        user_id: userId,
      });
      return result.data;
    } catch (result: any) {
      throw new Error(`${result.msg}:${result.error}`);
    }
  }

  // 背景音乐相关
  async backmusicList(): Promise<any> {
    try {
      return await this.send("GET", "/api/v1/chat/back-music/list", {});
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  async backmusicUpload(formData: RequestData): Promise<void> {
    try {
      await this.send("POST", "/api/v1/chat/back-music/upload", formData);
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  async backmusicDelete(url: string): Promise<void> {
    try {
      await this.send("DELETE", "/api/v1/chat/back-music/delete", {
        url: encodeURI(url),
      });
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  // 背景相关
  async backgroundList(): Promise<any> {
    try {
      return await this.send("GET", "/api/v1/chat/background/list", {});
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  async backgroundUpload(formData: RequestData): Promise<void> {
    try {
      await this.send("POST", "/api/v1/chat/background/upload", formData);
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }

  async backgroundDelete(id: string): Promise<void> {
    try {
      await this.send("DELETE", "/api/v1/chat/background/delete", {
        id: id,
      });
    } catch (result: any) {
      throw new Error(result.detail);
    }
  }
}

const request = new Request();
export default request;

// 请求工具函数
export const requestUtils = {
  parseFetchError: (error: string): string => {
    switch (error) {
      case "TypeError":
        return "网络无法连接";
      case "DOMException":
        return "请求已被取消";
      case "SyntaxError":
        return "响应错误解析";
      default:
        return "未知请求错误";
    }
  },

  parseSaveError: (error: string): string => {
    switch (error) {
      case "/list":
        return "存档列表读取失败";
      case "/load":
        return "存档加载失败";
      case "/save":
        return "存档保存失败";
      case "/delete":
        return "存档删除失败";
      case "/create":
        return "存档创建失败";
      case "/process-log":
        return "存档处理失败";
      default:
        return "未知存档错误";
    }
  },

  parseSaveLoading: (error: string): string => {
    switch (error) {
      case "/list":
        return "正在存档列表读取";
      case "/load":
        return "正在加载存档";
      case "/save":
        return "正在保存存档";
      case "/delete":
        return "正在删除存档";
      case "/create":
        return "正在创建存档";
      case "/process-log":
        return "正在处理存档";
      default:
        return "未知存档事件";
    }
  },

  parseSaveSuccess: (error: string): string => {
    switch (error) {
      case "/list":
        return "存档列表读取完成";
      case "/load":
        return "加载存档完成";
      case "/save":
        return "保存存档完成";
      case "/delete":
        return "删除存档完成";
      case "/create":
        return "创建存档完成";
      case "/process-log":
        return "处理存档完成";
      default:
        return "未知存档事件";
    }
  },
};
