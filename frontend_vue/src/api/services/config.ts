import http from '../http'

export type StructuredConfig = Record<string, any>

export async function fetchEnvConfig(): Promise<StructuredConfig> {
  return http.get('/settings/config')
}

export async function saveEnvConfig(
  values: Record<string, string>,
): Promise<{ status: string; message: string }> {
  return http.post('/settings/config', values)
}
