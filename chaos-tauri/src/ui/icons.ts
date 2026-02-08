import home from '@fluentui/svg-icons/icons/home_20_regular.svg?raw'
import subtitle from '@fluentui/svg-icons/icons/arrow_download_20_regular.svg?raw'
import live from '@fluentui/svg-icons/icons/live_20_regular.svg?raw'
import danmaku from '@fluentui/svg-icons/icons/comment_20_regular.svg?raw'
import settings from '@fluentui/svg-icons/icons/apps_settings_20_regular.svg?raw'
import about from '@fluentui/svg-icons/icons/info_20_regular.svg?raw'

export const ICONS = {
  home,
  subtitle,
  live,
  danmaku,
  settings,
  about
} as const

export type AppIconName = keyof typeof ICONS

