import { invoke } from '@tauri-apps/api/core'

/** Discussion 分类 */
export interface DiscussionCategory {
  name: string
}

/** Discussion 作者 */
export interface DiscussionAuthor {
  login: string
}

/** GitHub Discussion 条目 */
export interface Discussion {
  /** Discussion 编号 */
  number: number
  /** 标题 */
  title: string
  /** 正文（Markdown，原始完整内容） */
  body: string
  /** GitHub 网页链接 */
  html_url: string
  /** 所属分类 */
  category: DiscussionCategory
  /** 作者信息（可能为 null） */
  author: DiscussionAuthor | null
  /** 创建时间（ISO 8601） */
  created_at: string
  /** 真实 upvote 数（GraphQL 有效，REST 为 0） */
  upvotes: number
  /** 是否有真实 upvote 数据（GraphQL=true，REST=false） */
  has_upvotes: boolean
  /** 👍 emoji 数量（REST 有效，GraphQL 为 0） */
  reactions_upvotes: number
  /** 解析出的头像图片 URL（body 中第一张图片） */
  avatar_url: string | null
  /** 解析出的描述文本 */
  description: string | null
  /** 解析出的标签列表（## 标签 段落内容） */
  tags: string[]
}

/**
 * 获取 GitHub Discussions 列表（创意工坊）
 */
export const fetchDiscussions = async (): Promise<Discussion[]> => {
  return invoke<Discussion[]>('fetch_discussions')
}
