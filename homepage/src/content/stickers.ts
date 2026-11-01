import type { Locale } from '../lib/locale'

export type StickerTone =
  | 'blue'
  | 'amber'
  | 'lilac'
  | 'pink'
  | 'mint'
  | 'cream'
  | 'stone'
  | 'peach'
  | 'red'
  | 'ink'
  | 'yellow'
  | 'white'

export type StickerDef = {
  id: string
  tone: StickerTone
  tags: string[]
  starred: boolean
  zh: { text: string; sub: string }
  en: { text: string; sub: string }
}

export const stickers: StickerDef[] = [
  {
    id: 'received',
    tone: 'blue',
    tags: ['work'],
    starred: true,
    zh: { text: '收到', sub: '立刻办' },
    en: { text: 'ROGER', sub: 'on it' },
  },
  {
    id: 'nice',
    tone: 'amber',
    tags: ['daily'],
    starred: true,
    zh: { text: '好耶', sub: '下班' },
    en: { text: 'NICE', sub: 'off work' },
  },
  {
    id: 'what',
    tone: 'lilac',
    tags: ['daily'],
    starred: false,
    zh: { text: '？', sub: '发生什么事了' },
    en: { text: 'HUH?', sub: 'what happened' },
  },
  {
    id: 'laugh',
    tone: 'pink',
    tags: ['daily', 'cats'],
    starred: false,
    zh: { text: '笑死', sub: '真绷不住' },
    en: { text: 'LOL', sub: "can't breathe" },
  },
  {
    id: 'online',
    tone: 'mint',
    tags: ['work'],
    starred: false,
    zh: { text: '在吗', sub: '试探一下' },
    en: { text: 'THERE?', sub: 'just checking' },
  },
  {
    id: 'thanks',
    tone: 'cream',
    tags: ['work'],
    starred: false,
    zh: { text: '谢谢老板', sub: '下次还敢' },
    en: { text: 'THANKS', sub: 'boss' },
  },
  {
    id: 'wait',
    tone: 'stone',
    tags: ['work'],
    starred: false,
    zh: { text: '稍等', sub: '正在加载' },
    en: { text: 'WAIT', sub: 'loading' },
  },
  {
    id: 'hug',
    tone: 'peach',
    tags: ['cats'],
    starred: true,
    zh: { text: '贴贴', sub: '猫猫专用' },
    en: { text: 'HUG', sub: 'for cats' },
  },
  {
    id: 'broken',
    tone: 'red',
    tags: ['daily'],
    starred: false,
    zh: { text: '坏了', sub: '这下真坏了' },
    en: { text: 'BROKEN', sub: 'oh no' },
  },
  {
    id: 'ok',
    tone: 'white',
    tags: ['work'],
    starred: false,
    zh: { text: 'OK', sub: '收到收到' },
    en: { text: 'OKAY', sub: 'got it' },
  },
  {
    id: 'cat',
    tone: 'yellow',
    tags: ['cats'],
    starred: false,
    zh: { text: '喵', sub: '猫猫' },
    en: { text: 'MEOW', sub: 'cat' },
  },
  {
    id: 'gg',
    tone: 'ink',
    tags: ['daily'],
    starred: false,
    zh: { text: 'GG', sub: '结束了' },
    en: { text: 'GG', sub: 'over' },
  },
]

export function stickerText(sticker: StickerDef, locale: Locale) {
  return sticker[locale]
}
