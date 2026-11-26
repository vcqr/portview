<template>
  <div v-if="show" class="message-box-overlay" @click="close">
    <div class="message-box" @click.stop>
      <div class="message-header">
        <span class="message-title">{{ title }}</span>
        <button class="icon-close" @click="close">
          <X :size="16" />
        </button>
      </div>
      <div class="message-content">
        <div v-if="icon" class="message-icon">
          <img :src="icon" alt="icon" />
        </div>
        <div class="message-text">{{ message }}</div>
      </div>
      <div class="message-actions">
        <button v-if="showCancel" class="cancel-btn" @click="cancel">{{ cancelText }}</button>
        <button class="confirm-btn" @click="confirm">{{ confirmText }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { X } from '@lucide/vue';

interface Props {
  show: boolean;
  type?: 'info' | 'success' | 'warning' | 'error';
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  showCancel?: boolean;
  icon?: string;
}

const props = withDefaults(defineProps<Props>(), {
  type: 'info',
  title: '',
  confirmText: '确定',
  cancelText: '取消',
  showCancel: false,
  icon: undefined,
});

// 定义事件
interface Emits {
  (e: 'confirm'): void;
  (e: 'cancel'): void;
  (e: 'close'): void;
}

const emit = defineEmits<Emits>();

// 根据类型设置标题
const defaultTitle = computed(() => {
  switch (props.type) {
    case 'success':
      return '成功';
    case 'warning':
      return '警告';
    case 'error':
      return '错误';
    default:
      return '提示';
  }
});

// 确认操作
const confirm = () => {
  emit('confirm');
  emit('close');
};

// 取消操作
const cancel = () => {
  emit('cancel');
  emit('close');
};

// 关闭弹窗
const close = () => {
  emit('close');
};
</script>

<style scoped>
.message-box-overlay {
  position: fixed;
  inset: 0;
  background-color: rgba(9, 14, 22, 0.45);
  backdrop-filter: blur(2px);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 10000;
}

.message-box {
  background-color: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  min-width: 320px;
  max-width: 500px;
  overflow: hidden;
  color: var(--text-1);
}

.message-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 20px;
  border-bottom: 1px solid var(--border);
}

.message-title {
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

.message-content {
  display: flex;
  padding: 20px;
  align-items: flex-start;
}

.message-icon {
  margin-right: 12px;
  width: 24px;
  height: 24px;
}

.message-icon img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.message-text {
  flex: 1;
  color: var(--text-1);
  line-height: 1.55;
  font-size: 13.5px;
}

.message-actions {
  display: flex;
  justify-content: flex-end;
  padding: 14px 20px;
  gap: 8px;
}

.confirm-btn,
.cancel-btn {
  height: 32px;
  padding: 0 16px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  transition: background-color 0.15s ease, border-color 0.15s ease;
}

.confirm-btn {
  background-color: var(--accent);
  color: #ffffff;
  border: 1px solid transparent;
}

.confirm-btn:hover {
  background-color: var(--accent-hover);
}

.cancel-btn {
  background-color: var(--bg-panel);
  color: var(--text-2);
  border: 1px solid var(--border-strong);
}

.cancel-btn:hover {
  color: var(--text-1);
  border-color: var(--accent);
}
</style>
