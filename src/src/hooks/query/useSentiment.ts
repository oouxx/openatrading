import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { sentimentKeys } from "./queryKeys"

export interface SentimentHotArticle {
  article_id: string
  title: string
  nickname: string
  like_count: number
  comment_count: number
  forward_count: number
  heat_score: number
  content: string
}

export interface SentimentTaogubaArticle {
  topic_id: number
  new_topic_id: number
  subject: string
  user_name: string
  view_count: number
  reply_count: number
  date_time: string
  content: string
}

/** 舆情聚合（/api/sentiment）。 */
export interface SentimentData {
  trend?: string
  hot_stocks?: Array<[string, string, number]>
  hot_articles?: SentimentHotArticle[]
  taoguba_articles?: SentimentTaogubaArticle[]
}

export function useSentiment() {
  return useQuery({
    queryKey: sentimentKeys.all,
    queryFn: () => fetchJson<SentimentData>("/api/sentiment"),
    staleTime: 2 * 60 * 1000,
  })
}
