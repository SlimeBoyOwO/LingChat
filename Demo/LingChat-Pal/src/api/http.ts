import axios, {
  AxiosInstance,
  AxiosError,
  AxiosResponse,
  InternalAxiosRequestConfig,
  AxiosRequestConfig,
} from "axios";

// 定义响应数据的通用结构
interface ApiResponse<T = any> {
  code: number;
  data: T;
  message: string;
}

// 自定义错误类型
interface AppError extends Error {
  code?: number;
  status?: number;
  response?: AxiosResponse;
}

// 正确的模块扩展方式
declare module "axios" {
  export interface AxiosRequestConfig {
    // 可以添加自定义配置项
    silent?: boolean; // 例如是否静默处理错误
  }

  export interface AxiosInstance {
    // 重写方法签名以支持直接返回数据
    get<T = any>(url: string, config?: AxiosRequestConfig): Promise<T>;
    post<T = any>(
      url: string,
      data?: any,
      config?: AxiosRequestConfig
    ): Promise<T>;
    put<T = any>(
      url: string,
      data?: any,
      config?: AxiosRequestConfig
    ): Promise<T>;
    delete<T = any>(url: string, config?: AxiosRequestConfig): Promise<T>;
    patch<T = any>(
      url: string,
      data?: any,
      config?: AxiosRequestConfig
    ): Promise<T>;
  }
}

const http: AxiosInstance = axios.create({
  baseURL: "/api",
  timeout: 30000,
  headers: {
    "Content-Type": "application/json",
    "X-Requested-With": "XMLHttpRequest",
  },
  withCredentials: true,
});

// 请求拦截器
http.interceptors.request.use(
  (config: InternalAxiosRequestConfig) => {
    config.headers = config.headers || {};

    const token = localStorage.getItem("token");
    if (token) {
      config.headers.Authorization = `Bearer ${token}`;
    }

    if (config.data instanceof FormData) {
      config.headers["Content-Type"] = "multipart/form-data";
    }

    return config;
  },
  (error: AxiosError) => {
    return Promise.reject(error);
  }
);

// 响应拦截器
http.interceptors.response.use(
  (response: AxiosResponse<ApiResponse>) => {
    // 检查是否有 code 字段
    if (response.data && response.data.code !== undefined) {
      if (response.data.code === 200) {
        return response.data.data;
      } else {
        const error: AppError = new Error(response.data.message || "请求失败");
        error.code = response.data.code;
        return Promise.reject(error);
      }
    } else if (response.data && response.data.data !== undefined) {
      // 处理没有 code 但有 data 字段的情况（您的后端结构）
      return response.data.data;
    } else {
      // 其他情况直接返回数据
      return response.data;
    }
  },
  (error: AxiosError) => {
    // 安全访问响应数据
    const responseData = (error.response?.data as ApiResponse) || {};
    const errorMessage = responseData.message || error.message || "网络错误";

    const enhancedError: AppError = new Error(errorMessage);
    enhancedError.status = error.response?.status;
    enhancedError.code = responseData.code;
    enhancedError.response = error.response;

    if (error.response) {
      switch (error.response.status) {
        case 401:
          window.location.href = "/login";
          break;
        case 403:
          // 权限相关处理
          break;
      }
    }

    return Promise.reject(enhancedError);
  }
);

export default http;
