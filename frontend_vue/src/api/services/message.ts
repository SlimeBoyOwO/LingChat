import axios, { AxiosRequestConfig, AxiosResponse } from "axios";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export async function send<T = any>(
    url: string,
    data?: unknown,
    config?: AxiosRequestConfig
): Promise<AxiosResponse<T>> {
    return axios.post<T>(url, data, config).catch(error => {
        console.error("Error in API request:", error);
        throw error;
    });
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export async function get<T = any>(url: string, params?: AxiosRequestConfig): Promise<AxiosResponse<T>> {
    return axios.get<T>(url, params).catch(error => {
        console.error("Error in API request:", error);
        throw error;
    });
}
