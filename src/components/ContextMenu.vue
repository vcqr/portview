<template>
  <div v-if="showContextMenu" class="context-menu" :style="contextMenuStyle">
    <ul>
      <li @click="handleShowProcessDetails">
        <span class="menu-item-content">
          <span class="menu-item-icon">
            <Info :size="14" />
          </span>
          {{ t("contextMenu.processDetails") }}
        </span>
      </li>
      <li
        @click="handleOpenContainingFolder"
        v-if="props.selectedConnection && props.selectedConnection.pid"
      >
        <span class="menu-item-content">
          <span class="menu-item-icon">
            <FolderOpen :size="14" />
          </span>
          {{ t("contextMenu.openContainingFolder") }}
        </span>
      </li>
      <li @click="handleKillProcess" class="danger-item">
        <span class="menu-item-content">
          <span class="menu-item-icon">
            <XCircle :size="14" />
          </span>
          {{ t("contextMenu.killProcess") }}
        </span>
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Info, FolderOpen, XCircle } from "@lucide/vue";
import type { TcpConnection } from "@/types/connection";

// 定义组件属性
interface Props {
  showContextMenu: boolean;
  contextMenuPosition: { x: number; y: number };
  selectedConnection: TcpConnection | null;
}

const props = defineProps<Props>();

// 定义事件发射器
interface Emits {
  (e: "update:showContextMenu", value: boolean): void;
  (e: "showProcessDetailsDialog", conn: TcpConnection): void;
  (e: "killProcess", conn: TcpConnection): void;
  (e: "openContainingFolder", conn: TcpConnection): void;
}

const emit = defineEmits<Emits>();

// 使用国际化
const { t } = useI18n();

// 计算右键菜单样式
const contextMenuStyle = computed(() => {
  return {
    top: `${props.contextMenuPosition.y}px`,
    left: `${props.contextMenuPosition.x}px`,
    position: "fixed" as const,
    zIndex: 1000,
  };
});

// 方法
const handleShowProcessDetails = () => {
  if (props.selectedConnection) {
    emit("showProcessDetailsDialog", props.selectedConnection);
  }
};

const handleKillProcess = () => {
  if (props.selectedConnection) {
    emit("killProcess", props.selectedConnection);
  }
};

const handleOpenContainingFolder = () => {
  if (props.selectedConnection) {
    emit("openContainingFolder", props.selectedConnection);
  }
};
</script>

<style scoped>
/* 右键菜单 */
.context-menu {
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  z-index: 10000;
  min-width: 180px;
  padding: 4px;
}

.context-menu ul {
  list-style: none;
  margin: 0;
  padding: 0;
}

.context-menu li {
  padding: 7px 10px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background-color 0.12s ease;
  font-size: 13px;
  color: var(--text-1);
  display: flex;
  align-items: center;
  line-height: 1.2;
}

.context-menu li:hover {
  background-color: var(--bg-hover);
}

.context-menu li:active {
  background-color: var(--accent-weak);
}

/* 危险操作（杀死进程） */
.context-menu li.danger-item {
  color: var(--danger);
}

.context-menu li.danger-item:hover {
  background-color: var(--danger-weak);
}

.menu-item-content {
  display: flex;
  align-items: center;
  gap: 10px;
}

.menu-item-icon {
  width: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-2);
  flex-shrink: 0;
}

.context-menu li.danger-item .menu-item-icon {
  color: var(--danger);
}
</style>
