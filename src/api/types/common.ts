export interface ApiResponse<T = any> {
  code: number;
  data: T;
  message: string;
}

export interface PaginationParams {
  page?: number;
  page_size?: number;
}

export interface PaginationResult<T> {
  data: T[];
  total: number;
  page: number;
  page_size: number;
}
