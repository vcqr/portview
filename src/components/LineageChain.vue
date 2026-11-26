<template>
  <div class="lineage-chain">
    <div
      v-for="(node, index) in nodes"
      :key="node.pid"
      class="chain-row"
    >
      <!-- 左侧时间线轨道：节点圆点 + 连接线 -->
      <div class="chain-rail">
        <span class="chain-dot" :class="dotClass(node)"></span>
        <span
          v-if="index < nodes.length - 1 || node.parent_exited"
          class="chain-link"
          :class="{ 'chain-link-gap': node.parent_exited }"
        ></span>
      </div>

      <div class="chain-body">
        <div
          class="chain-node"
          :class="{
            'is-target': node.is_target,
            'is-located': highlightPid === node.pid,
          }"
          role="button"
          tabindex="0"
          :title="nodeHint(node)"
          @click="emit('highlight', node.pid)"
          @dblclick="emit('inspect', node.pid)"
          @keydown.enter="emit('highlight', node.pid)"
        >
          <img
            :src="node.icon ? 'data:image/png;base64,' + node.icon : '/exe.svg'"
            :alt="node.name"
            class="chain-icon"
          />
          <span class="chain-name">{{ node.name }}</span>
          <span class="chain-pid">PID {{ node.pid }}</span>
          <span v-if="node.user" class="chain-user">{{ node.user }}</span>
          <span v-if="isRoot(node, index)" class="chain-root-tag">
            {{ t("lineage.rootTag") }}
          </span>
          <span
            v-if="highlightPid === node.pid"
            class="chain-located-tag"
          >
            {{ t("lineage.locatedTag") }}
          </span>
        </div>

        <!-- 链断裂标记：启动该进程的父进程已退出（对齐 witr 的 ParentExited） -->
        <div v-if="node.parent_exited" class="chain-gap">
          <AlertTriangle :size="12" />
          <span>{{ t("lineage.parentExited") }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { AlertTriangle } from "@lucide/vue";
import type { LineageNode } from "@/types/connection";

interface Props {
  nodes: LineageNode[]; // root → target 顺序
  highlightPid?: number | null; // 当前在连接列表中被高亮定位的 PID
}

defineProps<Props>();

const emit = defineEmits<{
  (e: "highlight", pid: number): void;
  (e: "inspect", pid: number): void;
}>();

const { t } = useI18n();

// 根进程判定：PID 1（launchd/systemd/init）或没有父进程
const isRoot = (node: LineageNode, index: number): boolean =>
  index === 0 && (node.pid === 1 || node.ppid === 0 || node.ppid === null);

const dotClass = (node: LineageNode): string => {
  if (node.is_target) return "dot-target";
  if (node.pid === 1 || node.ppid === 0 || node.ppid === null) return "dot-root";
  return "dot-normal";
};

// 悬停提示：完整命令行 + 交互说明
const nodeHint = (node: LineageNode): string => {
  const cmd = node.command_line || node.name;
  return `${cmd}\n${t("lineage.nodeHint")}`;
};
</script>

<style scoped>
.lineage-chain {
  display: flex;
  flex-direction: column;
}

.chain-row {
  display: flex;
  align-items: stretch;
  min-height: 30px;
}

/* ===== 左侧轨道 ===== */
.chain-rail {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 16px;
  flex-shrink: 0;
  padding-top: 10px;
}

.chain-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
  box-sizing: border-box;
}

.dot-root {
  background: var(--bg-subtle);
  border: 2px solid var(--text-3);
}

.dot-normal {
  background: var(--bg-subtle);
  border: 2px solid var(--accent);
}

.dot-target {
  background: var(--accent);
  border: 2px solid var(--accent);
  box-shadow: 0 0 0 3px var(--accent-weak);
}

/* 连接线：从圆点延伸到下一行圆点；断裂时为警告色虚线 */
.chain-link {
  flex: 1;
  width: 2px;
  margin-top: 2px;
  background: var(--border-strong);
}

.chain-link-gap {
  background: transparent;
  border-left: 2px dashed var(--warning);
}

/* ===== 节点 ===== */
.chain-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding-bottom: 4px;
}

.chain-node {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-subtle);
  cursor: pointer;
  min-width: 0;
  transition: border-color 0.15s ease, background-color 0.15s ease;
}

.chain-node:hover {
  border-color: var(--accent);
  background-color: var(--bg-hover);
}

/* 目标进程：描边高亮 */
.chain-node.is-target {
  border-color: var(--accent);
  background: var(--accent-weak);
}

/* 已在连接列表中定位的节点：虚线描边 */
.chain-node.is-located {
  border-style: dashed;
  border-color: var(--accent);
}

.chain-icon {
  width: 18px;
  height: 18px;
  object-fit: contain;
  flex-shrink: 0;
}

.chain-name {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.chain-pid {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-2);
  background: var(--bg-inset);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0 7px;
  flex-shrink: 0;
  white-space: nowrap;
}

.chain-user {
  font-size: 11px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 1;
}

.chain-root-tag,
.chain-located-tag {
  font-size: 10px;
  border-radius: 4px;
  padding: 1px 6px;
  white-space: nowrap;
  flex-shrink: 0;
}

.chain-root-tag {
  color: var(--text-3);
  border: 1px solid var(--border);
  background: var(--bg-inset);
}

.chain-located-tag {
  color: var(--accent);
  border: 1px solid var(--accent);
  background: var(--accent-weak);
}

/* ===== 断裂标记 ===== */
.chain-gap {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 4px 0 4px 8px;
  padding: 4px 8px;
  border-radius: var(--radius-sm);
  background: var(--warning-weak);
  color: var(--warning);
  font-size: 11.5px;
}
</style>
