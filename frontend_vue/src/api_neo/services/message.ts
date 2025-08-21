import axios, { AxiosRequestConfig } from "axios";
export default async function send<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<T> {
    if (data["validateStatus"] === undefined) {
        data["validateStatus"] = (status: number) => (status >= 200 && status < 305) || status === 307;
    }
    return axios.post<T>(url, data, config).catch(error => {
        console.error("Error in API request:", error);
        throw error;
    });
}
