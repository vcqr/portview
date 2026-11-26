import { createI18n } from 'vue-i18n';

// 导入语言资源文件
import zhCN from './locales/zh-CN.json';
import enUS from './locales/en-US.json';

// 类型定义，用于TypeScript支持
type MessageSchema = typeof zhCN;

const i18n = createI18n<[MessageSchema], 'zh' | 'en'>({
  legacy: false, // 使用Composition API风格
  locale: localStorage.getItem('locale') || 'zh', // 从localStorage获取语言设置，默认为中文
  fallbackLocale: 'zh', // 备用语言
  messages: {
    zh: zhCN,
    en: enUS
  }
});

export default i18n;