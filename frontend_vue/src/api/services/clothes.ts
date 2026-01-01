import http from '../http'
import type { Clothes } from '../../types'

export const getAllClothes = async (): Promise<Clothes[]> => {
  try {
    const data = await http.get('/v1/chat/clothes/list', {})
    return data
  } catch (error: any) {
    console.error('获取游戏信息错误:', error.message)
    throw error // 直接抛出拦截器处理过的错误
  }
}
