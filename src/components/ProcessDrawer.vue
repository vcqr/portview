<template>
  <aside v-if="show" class="process-drawer" :style="{ width: width + 'px' }">
    <!-- 左缘拖拽手柄：调整抽屉宽度 -->
    <div class="drawer-resize-handle" @mousedown="startResize"></div>

    <div class="drawer-inner">
      <!-- 头部：进程身份 -->
      <div class="drawer-header">
        <img
          :src="targetIcon ? 'data:image/png;base64,' + targetIcon : '/exe.svg'"
          :alt="processDetails?.name || 'Process Icon'"
          class="process-icon-large"
        />
        <div class="header-info">
          <h3 class="process-name">
            {{ processDetails?.name || t("modal.processDetails") }}
          </h3>
          <span v-if="processDetails" class="pid-chip">
            PID {{ processDetails.pid }}
          </span>
        </div>
        <div class="header-actions">
          <!-- 返回上一进程：双击链上/子进程节点进入后可逐级回退 -->
          <button
            v-if="canGoBack"
            class="icon-btn"
            @click="emit('back')"
            :title="t('lineage.back')"
          >
            <ArrowLeft :size="15" />
          </button>
          <button
            class="icon-btn"
            :class="{ spinning: refreshing }"
            @click="emit('refresh')"
            :title="t('modal.refresh')"
          >
            <RefreshCw :size="15" />
          </button>
          <button
            class="icon-btn"
            @click="emit('update:show', false)"
            :title="t('lineage.close')"
          >
            <X :size="16" />
          </button>
        </div>
      </div>

      <div v-if="processDetails" class="drawer-body">
        <!-- 进程已退出提示 -->
        <div v-if="processGone" class="gone-banner">
          <AlertTriangle :size="14" />
          <span>{{ t("modal.processGone") }}</span>
        </div>

        <!-- 概览指标卡片 -->
        <div class="stats-grid">
          <div class="stat-card">
            <span class="stat-icon"><MemoryStick :size="15" /></span>
            <div class="stat-info">
              <span class="stat-label">{{ t("modal.memoryUsage") }}</span>
              <span class="stat-value">{{
                formatMemoryUsage(processDetails.memory_usage)
              }}</span>
            </div>
          </div>
          <div class="stat-card">
            <span class="stat-icon"><Cpu :size="15" /></span>
            <div class="stat-info">
              <span class="stat-label">{{ t("modal.cpuUsage") }}</span>
              <span class="stat-value">{{ processDetails.cpu_usage.toFixed(1) }}%</span>
            </div>
          </div>
          <div class="stat-card">
            <span class="stat-icon"><Clock :size="15" /></span>
            <div class="stat-info">
              <span class="stat-label">{{ t("modal.startTime") }}</span>
              <span class="stat-value">{{ formatDate(processDetails.start_time) }}</span>
            </div>
          </div>
          <div class="stat-card">
            <span class="stat-icon"><GitFork :size="15" /></span>
            <div class="stat-info">
              <span class="stat-label">{{ t("modal.parentPid") }}</span>
              <span class="stat-value">{{ processDetails.parent_pid ?? "-" }}</span>
            </div>
          </div>
        </div>

        <!-- 命令行 -->
        <div class="detail-section">
          <div class="detail-label-row">
            <span class="detail-label">{{ t("modal.commandLine") }}</span>
            <button
              v-if="processDetails.command_line"
              class="mini-btn"
              :class="{ copied: copiedKey === 'cmd' }"
              @click="copyText(processDetails.command_line, 'cmd')"
              :title="copiedKey === 'cmd' ? t('modal.copied') : t('modal.copy')"
            >
              <Check v-if="copiedKey === 'cmd'" :size="13" />
              <Copy v-else :size="13" />
            </button>
          </div>
          <div class="code-block">{{ processDetails.command_line || "-" }}</div>
        </div>

        <!-- 执行路径 -->
        <div class="detail-section">
          <div class="detail-label-row">
            <span class="detail-label">{{ t("modal.executablePath") }}</span>
            <span class="label-actions">
              <button
                v-if="processDetails.executable_path"
                class="mini-btn"
                :class="{ copied: copiedKey === 'path' }"
                @click="copyText(processDetails.executable_path, 'path')"
                :title="
                  copiedKey === 'path' ? t('modal.copied') : t('modal.copy')
                "
              >
                <Check v-if="copiedKey === 'path'" :size="13" />
                <Copy v-else :size="13" />
              </button>
              <button
                v-if="
                  processDetails.executable_path &&
                  processDetails.executable_path !== ''
                "
                class="mini-btn"
                @click="openContainingFolder"
                :title="t('modal.openFolder')"
              >
                <FolderOpen :size="13" />
              </button>
            </span>
          </div>
          <div class="code-block">{{ processDetails.executable_path || "-" }}</div>
        </div>

        <!-- 调用链：沿父进程追溯到系统根（置于底部，与子进程相邻构成完整链路） -->
        <div class="detail-section">
          <div class="detail-label-row">
            <span class="detail-label lineage-label">
              <GitBranch :size="13" />
              {{ t("lineage.title") }}
            </span>
            <span class="label-actions">
              <button
                v-if="chainText"
                class="mini-btn"
                :class="{ copied: copiedKey === 'chain' }"
                @click="copyText(chainText, 'chain')"
                :title="
                  copiedKey === 'chain'
                    ? t('modal.copied')
                    : t('lineage.copyChain')
                "
              >
                <Check v-if="copiedKey === 'chain'" :size="13" />
                <Copy v-else :size="13" />
              </button>
              <button
                class="mini-btn"
                :class="{ spinning: lineageLoading }"
                @click="emit('refreshLineage')"
                :title="t('lineage.refresh')"
              >
                <RefreshCw :size="12" />
              </button>
            </span>
          </div>
          <LineageChain
            v-if="lineage && lineage.nodes.length > 0"
            :nodes="lineage.nodes"
            :highlight-pid="highlightPid"
            @highlight="emit('highlight', $event)"
            @inspect="emit('inspect', $event)"
          />
          <div v-else-if="lineageLoading" class="chain-placeholder">
            {{ t("lineage.loading") }}
          </div>
          <div v-else class="chain-placeholder">
            {{ t("lineage.empty") }}
          </div>
        </div>

        <!-- 直接子进程：置于调用链下方，完整展开由抽屉整体滚动 -->
        <div
          v-if="lineage && lineage.children.length > 0"
          class="detail-section"
        >
          <div class="children-header" @click="showChildren = !showChildren">
            <ChevronDown
              :size="14"
              class="chevron"
              :class="{ collapsed: !showChildren }"
            />
            <span>
              {{ t("lineage.children") }} ({{ lineage.children.length }})
            </span>
          </div>
          <div v-show="showChildren" class="children-list">
            <div
              v-for="child in lineage.children"
              :key="child.pid"
              class="child-row"
              :title="childHint(child)"
              @click="emit('highlight', child.pid)"
              @dblclick="emit('inspect', child.pid)"
            >
              <img
                :src="
                  child.icon
                    ? 'data:image/png;base64,' + child.icon
                    : '/exe.svg'
                "
                :alt="child.name"
                class="chain-icon"
              />
              <span class="child-name">{{ child.name }}</span>
              <span class="pid-chip small">PID {{ child.pid }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 详情尚未加载时的占位 -->
      <div v-else class="drawer-body drawer-loading">
        {{ t("lineage.loading") }}
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  ArrowLeft,
  Check,
  ChevronDown,
  Clock,
  Copy,
  Cpu,
  FolderOpen,
  GitBranch,
  GitFork,
  MemoryStick,
  RefreshCw,
  X,
} from "@lucide/vue";
import LineageChain from "./LineageChain.vue";
import type { ProcessDetails, ProcessLineage, LineageNode } from "@/types/connection";

interface Props {
  show: boolean;
  processDetails: ProcessDetails | null;
  lineage: ProcessLineage | null; // 调用链（打开时拉取一次）
  lineageLoading?: boolean;
  processIcon: string | null; // Base64 encoded icon data（连接行带来的图标）
  processGone?: boolean; // 进程是否已退出（轮询时发现）
  refreshing?: boolean; // 手动刷新进行中
  highlightPid?: number | null; // 连接列表中当前高亮的 PID
  width?: number; // 抽屉宽度（px），由父组件持久化
  canGoBack?: boolean; // 是否存在可返回的上一进程（导航历史非空）
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (e: "update:show", value: boolean): void;
  (e: "refresh"): void;
  (e: "refreshLineage"): void;
  (e: "highlight", pid: number): void;
  (e: "inspect", pid: number): void;
  (e: "update:width", value: number): void;
  (e: "back"): void;
}>();

const { t } = useI18n();

// 头部图标优先取调用链中当前进程自己的图标（双击链上节点切换目标后，
// 连接行的图标不再对应当前进程）
const targetIcon = computed(() => {
  const pid = props.processDetails?.pid;
  const node = props.lineage?.nodes.find((n) => n.pid === pid);
  return node?.icon || props.processIcon;
});

// 单行调用链文本：name (pid) → name (pid) → ...
const chainText = computed(() => {
  if (!props.lineage || props.lineage.nodes.length === 0) return "";
  return props.lineage.nodes
    .map((n) => `${n.name} (${n.pid})`)
    .join(" → ");
});

// 子进程列表默认展开，切换查看的进程时重置
const showChildren = ref(true);
watch(
  () => props.processDetails?.pid,
  () => {
    showChildren.value = true;
  },
);

// 浮层打开期间支持 Esc 关闭
const onKeydown = (e: KeyboardEvent) => {
  if (e.key === "Escape") emit("update:show", false);
};
watch(
  () => props.show,
  (show) => {
    if (show) {
      window.addEventListener("keydown", onKeydown);
    } else {
      window.removeEventListener("keydown", onKeydown);
    }
  },
);
onUnmounted(() => window.removeEventListener("keydown", onKeydown));

// 挂载时收敛持久化宽度：窗口缩小过后，存储值可能已超过 80vw 上限
onMounted(() => {
  const max = Math.max(
    DRAWER_MIN_WIDTH,
    window.innerWidth * DRAWER_MAX_RATIO,
  );
  if ((props.width ?? 0) > max) {
    emit("update:width", max);
  }
});

// ===== 拖拽调宽 =====
const DRAWER_MIN_WIDTH = 320;
// 最大宽度为视口的 80%，拖拽时按当前视口动态计算
const DRAWER_MAX_RATIO = 0.8;

const startResize = (e: MouseEvent) => {
  e.preventDefault();
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  const onMove = (ev: MouseEvent) => {
    const max = Math.max(
      DRAWER_MIN_WIDTH,
      window.innerWidth * DRAWER_MAX_RATIO,
    );
    const w = Math.min(
      max,
      Math.max(DRAWER_MIN_WIDTH, window.innerWidth - ev.clientX),
    );
    emit("update:width", w);
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
};

// ===== 复制反馈 =====
const copiedKey = ref<string | null>(null);
let copiedTimer: number | null = null;

const copyText = async (text: string, key: string) => {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // 剪贴板 API 不可用时退化为 execCommand
    const textarea = document.createElement("textarea");
    textarea.value = text;
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand("copy");
    document.body.removeChild(textarea);
  }
  copiedKey.value = key;
  if (copiedTimer !== null) {
    window.clearTimeout(copiedTimer);
  }
  copiedTimer = window.setTimeout(() => {
    copiedKey.value = null;
  }, 1500);
};

// 打开文件所在目录
const openContainingFolder = async () => {
  if (props.processDetails && props.processDetails.executable_path) {
    try {
      await invoke("open_folder", {
        path: props.processDetails.executable_path,
      });
    } catch (error) {
      console.error("Failed to open folder:", error);
    }
  }
};

// 子进程行悬停提示
const childHint = (child: LineageNode): string =>
  `${child.command_line || child.name}\n${t("lineage.nodeHint")}`;

// 格式化内存使用量显示
const formatMemoryUsage = (memoryInBytes: number): string => {
  if (memoryInBytes < 1024) {
    return `${memoryInBytes} B`;
  } else if (memoryInBytes < 1024 * 1024) {
    const memoryInKB = (memoryInBytes / 1024).toFixed(2);
    return `${memoryInKB} KB`;
  } else if (memoryInBytes < 1024 * 1024 * 1024) {
    const memoryInMB = (memoryInBytes / (1024 * 1024)).toFixed(2);
    return `${memoryInMB} MB`;
  } else {
    const memoryInGB = (memoryInBytes / (1024 * 1024 * 1024)).toFixed(2);
    return `${memoryInGB} GB`;
  }
};

// 格式化日期时间显示
const formatDate = (timestamp: number | null): string => {
  if (timestamp === null || timestamp === 0) {
    return "-";
  }

  const date = new Date(timestamp * 1000);

  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");

  return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
};
</script>

<style scoped>
/* 浮层抽屉：悬停在连接表格上方，不挤压表格布局。
   定位在 main-row（position: relative）内，只覆盖表格区域、
   不遮挡顶部工具栏与底部状态栏；层级低于 MenuBar(100) 与右键菜单(10000) */
.process-drawer {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 50;
  min-width: 0;
  background: var(--bg-panel);
  border-left: 1px solid var(--border);
  box-shadow: var(--shadow-lg);
  display: flex;
  flex-direction: column;
  color: var(--text-1);
  animation: drawer-in 0.2s ease-out;
}

@keyframes drawer-in {
  from {
    transform: translateX(24px);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

/* 左缘拖拽手柄 */
.drawer-resize-handle {
  position: absolute;
  left: -3px;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: col-resize;
  z-index: 10;
}

.drawer-resize-handle:hover {
  background: var(--accent-weak);
}

.drawer-inner {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

/* ===== 头部 ===== */
.drawer-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}

.process-icon-large {
  width: 34px;
  height: 34px;
  object-fit: contain;
  flex-shrink: 0;
}

.header-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  flex: 1;
}

.process-name {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pid-chip {
  width: fit-content;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-2);
  background: var(--bg-subtle);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 1px 8px;
}

.pid-chip.small {
  font-size: 10.5px;
  padding: 0 7px;
  flex-shrink: 0;
  white-space: nowrap;
}

.header-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.icon-btn {
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

.icon-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-1);
}

.icon-btn.spinning {
  color: var(--accent);
  animation: drawer-spin 0.8s linear infinite;
}

@keyframes drawer-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

/* ===== 主体 ===== */
.drawer-body {
  padding: 14px 16px 16px;
  overflow-y: auto;
  flex-grow: 1;
  min-height: 0;
}

.drawer-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-3);
  font-size: 12.5px;
}

/* 进程已退出提示条 */
.gone-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  margin-bottom: 14px;
  border-radius: var(--radius-sm);
  background: var(--warning-weak);
  border: 1px solid var(--warning);
  color: var(--warning);
  font-size: 12.5px;
}

/* ===== 概览指标卡片 ===== */
.stats-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  margin-bottom: 16px;
}

.stat-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  background: var(--bg-subtle);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  min-width: 0;
}

.stat-icon {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-sm);
  background: var(--accent-weak);
  color: var(--accent);
  flex-shrink: 0;
}

.stat-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.stat-label {
  font-size: 11px;
  color: var(--text-3);
}

.stat-value {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-variant-numeric: tabular-nums;
}

/* ===== 区块 ===== */
.detail-section {
  margin-bottom: 14px;
}

.detail-section:last-child {
  margin-bottom: 0;
}

.detail-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.detail-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-3);
}

/* 调用链标题带图标 */
.lineage-label {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.label-actions {
  display: inline-flex;
  gap: 4px;
}

.mini-btn {
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--bg-panel);
  color: var(--text-2);
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease,
    background-color 0.15s ease;
}

.mini-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
  background-color: var(--accent-weak);
}

.mini-btn.copied {
  border-color: var(--success);
  color: var(--success);
  background-color: var(--success-weak);
}

.mini-btn.spinning {
  color: var(--accent);
  animation: drawer-spin 0.8s linear infinite;
}

/* 调用链加载/空占位 */
.chain-placeholder {
  padding: 12px 10px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-md);
  color: var(--text-3);
  font-size: 12px;
  text-align: center;
}

/* ===== 子进程列表 ===== */
.children-header {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-3);
  cursor: pointer;
  user-select: none;
  margin-bottom: 6px;
}

.children-header:hover {
  color: var(--text-1);
}

.chevron {
  transition: transform 0.15s ease;
}

.chevron.collapsed {
  transform: rotate(-90deg);
}

/* 子进程列表不限制高度：完整展开，随抽屉主体一起滚动 */
.children-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.child-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-subtle);
  cursor: pointer;
  min-width: 0;
  transition: border-color 0.15s ease, background-color 0.15s ease;
}

.child-row:hover {
  border-color: var(--accent);
  background-color: var(--bg-hover);
}

.chain-icon {
  width: 16px;
  height: 16px;
  object-fit: contain;
  flex-shrink: 0;
}

.child-name {
  font-size: 12px;
  color: var(--text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
  min-width: 0;
}

/* ===== 命令行 / 路径区块 ===== */
.code-block {
  background: var(--bg-inset);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 8px 10px;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-1);
  white-space: pre-wrap;
  word-break: break-all;
  overflow-wrap: anywhere;
  max-height: 96px;
  overflow-y: auto;
}
</style>
