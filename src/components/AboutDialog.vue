<template>
  <div class="about-modal-overlay" v-if="showAbout">
    <div class="about-modal">
      <div class="about-header">
        <h2>{{ t("about.title") }}</h2>
        <button class="icon-close" @click="closeAbout">
          <X :size="16" />
        </button>
      </div>
      <div class="about-content">
        <div class="app-logo-container">
          <img src="/icon.svg" alt="PortView Logo" class="app-logo-large" />
        </div>
        <div class="app-info">
          <p><strong>{{ t("about.appName") }}:</strong> {{ t("appTitle") }}</p>
          <p><strong>{{ t("about.version") }}:</strong> {{ appVersion }}</p>
          <p><strong>{{ t("about.description") }}:</strong> {{ t("about.descriptionText") }}</p>
          <p><strong>{{ t("about.author") }}:</strong> vcyang</p>
          <p><strong>{{ t("about.github") }}:</strong> 
            <a href="#" @click.prevent="openGitHub"> {{ t("about.githubLink") }}</a>
          </p>
        </div>
      </div>
      <div class="about-footer">
        <button class="ok-btn" @click="closeAbout">{{ t("about.okButton") }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { openUrl } from '@tauri-apps/plugin-opener';
import { X } from "@lucide/vue";

interface Props {
  showAbout: boolean;
  appVersion: string;
}

const props = defineProps<Props>();

interface Emits {
  (e: "update:showAbout", value: boolean): void;
}

const emit = defineEmits<Emits>();

const { t } = useI18n();

const closeAbout = () => {
  emit('update:showAbout', false);
};

const openGitHub = async () => {
  // 使用系统默认浏览器打开链接
  await openUrl('https://github.com/vcqr/portview');
};
</script>

<style scoped>
.about-modal-overlay {
  position: fixed;
  inset: 0;
  background-color: rgba(9, 14, 22, 0.45);
  backdrop-filter: blur(2px);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 10000;
}

.about-modal {
  background-color: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  width: 90%;
  max-width: 460px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  position: relative;
  color: var(--text-1);
}

.about-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 20px;
  border-bottom: 1px solid var(--border);
}

.about-header h2 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-1);
}

/* 统一的幽灵关闭按钮 */
.icon-close {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-3);
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease;
}

.icon-close:hover {
  background-color: var(--bg-hover);
  color: var(--text-1);
}

.about-content {
  padding: 24px 20px;
  flex-grow: 1;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.app-logo-container {
  display: flex;
  justify-content: center;
  align-items: center;
}

.app-logo-large {
  width: 88px;
  height: 88px;
  object-fit: contain;
}

.app-info p {
  margin: 0 0 10px 0;
  color: var(--text-2);
  line-height: 1.55;
  font-size: 13.5px;
}

.app-info strong {
  color: var(--text-1);
  font-weight: 600;
}

.about-content a {
  color: var(--accent);
  text-decoration: none;
}

.about-content a:hover {
  text-decoration: underline;
}

.about-footer {
  padding: 14px 20px;
  border-top: 1px solid var(--border);
  display: flex;
  justify-content: flex-end;
}

.ok-btn {
  height: 32px;
  padding: 0 18px;
  background-color: var(--accent);
  color: #ffffff;
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  transition: background-color 0.15s ease;
}

.ok-btn:hover {
  background-color: var(--accent-hover);
}
</style>
