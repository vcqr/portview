<template>
  <div class="connections-table-container">
    <div class="table-wrapper">
      <!-- 首次加载骨架屏 -->
      <div v-if="isLoading && connections.length === 0" class="table-skeleton">
        <div
          v-for="(width, i) in skeletonWidths"
          :key="i"
          class="skeleton-row"
        >
          <span class="skeleton-bar" :style="{ width: width + '%' }"></span>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-else-if="connections.length === 0" class="table-empty">
        <Globe class="empty-icon" :size="40" :stroke-width="1.5" />
        <p class="empty-title">{{ t("table.emptyTitle") }}</p>
        <p class="empty-hint">{{ t("table.emptyHint") }}</p>
      </div>

      <table v-else class="connections-table">
        <thead>
          <tr>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 0)"
            >
              <div class="column-header" @click="toggleSort('process_name')">
                <span class="sortable-header">
                  {{ t("tableHeaders.processName") }}
                  <span
                    v-if="sortColumn === 'process_name'"
                    class="sort-indicator"
                  >
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 1)"
            >
              <div class="column-header" @click="toggleSort('pid')">
                <span class="sortable-header">
                  {{ t("tableHeaders.pid") }}
                  <span v-if="sortColumn === 'pid'" class="sort-indicator">
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 2)"
            >
              <div class="column-header" @click="toggleSort('protocol')">
                <span class="sortable-header">
                  {{ t("tableHeaders.protocol") }}
                  <span
                    v-if="sortColumn === 'protocol'"
                    class="sort-indicator"
                  >
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 3)"
            >
              <div class="column-header" @click="toggleSort('local_addr')">
                <span class="sortable-header">
                  {{ t("tableHeaders.localAddr") }}
                  <span
                    v-if="sortColumn === 'local_addr'"
                    class="sort-indicator"
                  >
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 4)"
            >
              <div class="column-header" @click="toggleSort('local_port')">
                <span class="sortable-header">
                  {{ t("tableHeaders.localPort") }}
                  <span
                    v-if="sortColumn === 'local_port'"
                    class="sort-indicator"
                  >
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 5)"
            >
              <div class="column-header" @click="toggleSort('remote_addr')">
                <span class="sortable-header">
                  {{ t("tableHeaders.remoteAddr") }}
                  <span
                    v-if="sortColumn === 'remote_addr'"
                    class="sort-indicator"
                  >
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 6)"
            >
              <div class="column-header" @click="toggleSort('remote_port')">
                <span class="sortable-header">
                  {{ t("tableHeaders.remotePort") }}
                  <span
                    v-if="sortColumn === 'remote_port'"
                    class="sort-indicator"
                  >
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 7)"
            >
              <div class="column-header" @click="toggleSort('state')">
                <span class="sortable-header">
                  {{ t("tableHeaders.state") }}
                  <span v-if="sortColumn === 'state'" class="sort-indicator">
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th
              class="resizable-th"
              @mousedown="startColumnResize($event, 8)"
            >
              <div class="column-header" @click="toggleSort('start_time')">
                <span class="sortable-header">
                  {{ t("tableHeaders.startTime") }}
                  <span
                    v-if="sortColumn === 'start_time'"
                    class="sort-indicator"
                  >
                    {{ sortDirection === "asc" ? " ▲" : " ▼" }}
                  </span>
                </span>
              </div>
            </th>
            <th class="filler-column">
              <!-- 冗余列，用于填充剩余空间 -->
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="conn in connections"
            :key="conn.id"
            @contextmenu="showContextMenuHandler(conn, $event)"
            @click="emit('update:clickedConnection', conn)"
            @dblclick="showProcessDetailsDialog(conn)"
            :class="{
              'selected-row': clickedConnection?.id === conn.id,
              'changed-connection': conn.hasChanged,
              'new-connection': conn.isNew,
              'deleted-connection': conn.isDeleted,
              'lineage-highlight':
                highlightPid != null && conn.pid === highlightPid,
            }"
            :data-pid="conn.pid ?? ''"
          >
            <td class="process-name-cell">
              <div class="process-with-icon" :title="conn.process_name || '-'">
                <img
                  :src="conn.icon ? 'data:image/png;base64,' + conn.icon : '/exe.svg'"
                  :alt="conn.process_name || 'Process Icon'"
                  class="process-icon"
                />
                <span
                  :class="{
                    'kernel-process': isKernelProcess(conn.process_name),
                  }"
                  >{{ conn.process_name || "-" }}</span
                >
              </div>
            </td>
            <td :title="conn.pid ? conn.pid.toString() : '-'">{{ conn.pid || "-" }}</td>
            <td :title="conn.protocol">{{ conn.protocol }}</td>
            <td :title="conn.local_addr">{{ conn.local_addr }}</td>
            <td :title="conn.local_port.toString()">{{ conn.local_port }}</td>
            <td :title="conn.remote_addr">{{ conn.remote_addr }}</td>
            <td :title="conn.remote_port.toString()">{{ conn.remote_port }}</td>
            <td :title="conn.state">{{ conn.state }}</td>
            <td :title="formatDate(conn.start_time)">{{ formatDate(conn.start_time) }}</td>
            <td class="filler-cell"></td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, watch, nextTick, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { Globe } from "@lucide/vue";
import type { TcpConnection, SortColumn, SortDirection } from "@/types/connection";

// 定义组件属性
interface Props {
  connections: TcpConnection[];
  clickedConnection: TcpConnection | null;
  sortColumn: string | null;
  sortDirection: SortDirection;
  customColumnWidths: Record<string, number>;
  isLoading?: boolean;
  highlightPid?: number | null; // 调用链联动的 PID 高亮（null = 未启用）
}

const props = defineProps<Props>();

// 定义事件发射器
interface Emits {
  (e: "update:clickedConnection", value: TcpConnection): void;
  (e: "toggleSort", column: SortColumn | 'start_time'): void;
  (e: "showContextMenuHandler", conn: TcpConnection, event: MouseEvent): void;
  (e: "showProcessDetailsDialog", conn: TcpConnection): void;
  (e: "update:customColumnWidths", value: Record<string, number>): void;
}

const emit = defineEmits<Emits>();

// 使用国际化
const { t } = useI18n();

// 从props获取数据
const connections = computed(() => props.connections);
const clickedConnection = computed(() => props.clickedConnection);

// 排序相关
const sortColumn = computed(() => props.sortColumn);
const sortDirection = computed(() => props.sortDirection);

// 列宽相关
const customColumnWidths = computed(() => props.customColumnWidths);

// 滚动定位到指定 PID 的第一行（调用链节点单击联动），返回是否找到
const scrollToPid = (pid: number): boolean => {
  const el = document.querySelector(
    `.connections-table tbody tr[data-pid="${pid}"]`,
  );
  if (!el) return false;
  el.scrollIntoView({ block: "center", behavior: "smooth" });
  return true;
};

defineExpose({ scrollToPid });

// 骨架屏每行的占位宽度（百分比）
const skeletonWidths = [92, 38, 52, 68, 30, 60, 42, 78, 34, 64, 46, 72];

// 在组件挂载后应用列宽
onMounted(() => {
  nextTick(() => {
    applyCustomColumnWidths();
  });
});

// 监听列宽变化并应用
watch(customColumnWidths, () => {
  nextTick(() => {
    applyCustomColumnWidths();
  });
}, { deep: true });

// 监听连接数据变化并应用列宽
watch(connections, () => {
  nextTick(() => {
    applyCustomColumnWidths();
  });
}, { deep: true });

// 方法
const toggleSort = (column: SortColumn) => {
  emit('toggleSort', column);
};

// 列宽拖拽相关变量
let isDragging = false;
let dragStartX = 0;
let dragStartWidth = 0;
let currentColumnIndex = -1;

const startColumnResize = (event: MouseEvent, columnIndex: number) => {
  // 检查是否在右边框区域（10像素范围内）
  const thElement = event.target as HTMLElement;
  const rect = thElement.getBoundingClientRect();
  const rightEdgeThreshold = 10; // 10像素的边框区域（包括margin）

  // 计算鼠标相对于元素右边的距离
  const distanceFromRight = rect.right - event.clientX;

  // 只有当鼠标在右边框区域内才允许拖拽
  if (distanceFromRight > rightEdgeThreshold) {
    return; // 不在边框区域，不执行拖拽
  }

  isDragging = true;
  dragStartX = event.clientX;
  currentColumnIndex = columnIndex;

  // 获取当前列的宽度
  dragStartWidth = thElement.offsetWidth;

  // 添加resizing类到表格
  const table = document.querySelector(".connections-table") as HTMLElement;
  if (table) {
    table.classList.add("resizing");
  }

  // 添加current-resizing类到当前列
  thElement.classList.add("current-resizing");

  // 添加鼠标移动和释放事件监听器
  document.addEventListener("mousemove", handleColumnResize);
  document.addEventListener("mouseup", stopColumnResize);

  // 阻止默认行为，防止选中文本
  event.preventDefault();
};

// 处理列宽调整
const handleColumnResize = (event: MouseEvent) => {
  if (!isDragging) return;

  const deltaX = event.clientX - dragStartX;
  const newWidth = Math.max(dragStartWidth + deltaX, 50); // 最小宽度50px

  // 获取所有表头元素
  const thElements = document.querySelectorAll(".connections-table th");
  if (thElements[currentColumnIndex]) {
    const th = thElements[currentColumnIndex] as HTMLElement;

    // 设置新的宽度
    th.style.width = `${newWidth}px`;
    th.style.maxWidth = `${newWidth}px`;
    th.style.minWidth = `${newWidth}px`;

    th.style.setProperty("width", `${newWidth}px`, "important");
    th.style.setProperty("max-width", `${newWidth}px`, "important");
    th.style.setProperty("min-width", `${newWidth}px`, "important");

    // 同时设置对应列的td元素以确保列宽一致
    const tdElements = document.querySelectorAll(
      `.connections-table td:nth-child(${currentColumnIndex + 1})`,
    );
    tdElements.forEach((td) => {
      const tdElement = td as HTMLElement;
      tdElement.style.width = `${newWidth}px`;
      tdElement.style.maxWidth = `${newWidth}px`;
      tdElement.style.minWidth = `${newWidth}px`;

      tdElement.style.setProperty("width", `${newWidth}px`, "important");
      tdElement.style.setProperty("max-width", `${newWidth}px`, "important");
      tdElement.style.setProperty("min-width", `${newWidth}px`, "important");
    });

    // 更新自定义列宽存储
    const columnOrder: (keyof TcpConnection)[] = [
      "process_name",
      "pid",
      "protocol",
      "local_addr",
      "local_port",
      "remote_addr",
      "remote_port",
      "state",
      "start_time",
    ];
    if (columnOrder[currentColumnIndex]) {
      const updatedWidths = { ...props.customColumnWidths };
      updatedWidths[columnOrder[currentColumnIndex]] = newWidth;
      emit('update:customColumnWidths', updatedWidths);
    }
  }
};

// 结束列宽调整
const stopColumnResize = () => {
  isDragging = false;

  // 移除resizing类
  const table = document.querySelector(".connections-table") as HTMLElement;
  if (table) {
    table.classList.remove("resizing");
  }

  // 移除current-resizing类
  const thElements = document.querySelectorAll(".connections-table th");
  thElements.forEach((th) => {
    th.classList.remove("current-resizing");
  });

  // 恢复鼠标指针样式
  thElements.forEach((th) => {
    (th as HTMLElement).style.cursor = "";
  });

  // 移除事件监听器
  document.removeEventListener("mousemove", handleColumnResize);
  document.removeEventListener("mouseup", stopColumnResize);
};

const showContextMenuHandler = (conn: TcpConnection, event: MouseEvent) => {
  emit('showContextMenuHandler', conn, event);
};

const showProcessDetailsDialog = (conn: TcpConnection) => {
  emit('showProcessDetailsDialog', conn);
};

const applyCustomColumnWidths = () => {
  // 使用 setTimeout 确保在 DOM 更新后执行
  setTimeout(() => {
    const thElements = document.querySelectorAll(".connections-table th");
    const columnOrder: (keyof TcpConnection)[] = [
      "process_name",
      "pid",
      "protocol",
      "local_addr",
      "local_port",
      "remote_addr",
      "remote_port",
      "state",
      "start_time",
    ];

    columnOrder.forEach((col, index) => {
      if (thElements[index]) {
        const th = thElements[index] as HTMLElement;
        let colWidth;

        if (props.customColumnWidths[col]) {
          // 使用自定义宽度
          colWidth = props.customColumnWidths[col];
        } else {
          // 使用默认宽度
          switch (col) {
            case "process_name":
              colWidth = 200; // 进程名称列更宽，以容纳较长的进程名
              break;
            case "pid":
              colWidth = 100; // PID列稍宽一些
              break;
            case "protocol":
              colWidth = 80; // 协议列稍宽一些
              break;
            case "local_addr":
              colWidth = 150; // 本地地址列更宽，以容纳完整的IP地址
              break;
            case "local_port":
              colWidth = 100; // 本地端口列稍宽一些
              break;
            case "remote_addr":
              colWidth = 150; // 远程地址列更宽，以容纳完整的IP地址
              break;
            case "remote_port":
              colWidth = 100; // 远程端口列稍宽一些
              break;
            case "state":
              colWidth = 120; // 状态列稍宽一些
              break;
            case "start_time":
              colWidth = 180; // 启动时间列更宽，以容纳完整的时间戳
              break;
            default:
              colWidth = 120; // 默认宽度稍宽一些
          }
        }

        // 设置列宽并使用 !important 确保优先级
        th.style.width = `${colWidth}px`;
        th.style.maxWidth = `${colWidth}px`;
        th.style.minWidth = `${colWidth}px`;

        th.style.setProperty("width", `${colWidth}px`, "important");
        th.style.setProperty("max-width", `${colWidth}px`, "important");
        th.style.setProperty("min-width", `${colWidth}px`, "important");

        // 同时设置td元素以确保列宽一致
        const tdElements = document.querySelectorAll(
          `.connections-table td:nth-child(${index + 1})`,
        );
        tdElements.forEach((td) => {
          const tdElement = td as HTMLElement;
          tdElement.style.width = `${colWidth}px`;
          tdElement.style.maxWidth = `${colWidth}px`;
          tdElement.style.minWidth = `${colWidth}px`;

          tdElement.style.setProperty("width", `${colWidth}px`, "important");
          tdElement.style.setProperty(
            "max-width",
            `${colWidth}px`,
            "important",
          );
          tdElement.style.setProperty(
            "min-width",
            `${colWidth}px`,
            "important",
          );
        });
      }
    });

    // 确保表格使用固定布局
    const table = document.querySelector(".connections-table") as HTMLElement;
    if (table) {
      table.style.tableLayout = "fixed";
    }

    // 设置冗余列填充剩余空间
    const fillerColumns = document.querySelectorAll(
      ".connections-table th.filler-column, .connections-table td.filler-cell",
    );
    fillerColumns.forEach((filler) => {
      const fillerElement = filler as HTMLElement;
      fillerElement.style.width = "100%";
      fillerElement.style.minWidth = "0";
      fillerElement.style.maxWidth = "none";
    });
  }, 0);
};

// 判断是否为内核进程
const isKernelProcess = (processName: string | null): boolean => {
  return (
    processName === "[KERNEL]" ||
    (processName !== null && processName.includes("[KERNEL]"))
  );
};

// 格式化日期时间显示
const formatDate = (timestamp: number | null): string => {
  if (timestamp === null || timestamp === 0) {
    return "-";
  }

  // 将秒级时间戳转换为毫秒级时间戳
  const date = new Date(timestamp * 1000);

  // 获取年、月、日、小时、分钟和秒
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0"); // 月份从0开始，需要+1
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");

  // 返回格式化的日期时间字符串
  return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
};
</script>

<style scoped>
.connections-table-container {
  width: 100%;
  overflow-x: auto;
  overflow-y: auto;
  flex: 1 1 auto;
  margin-top: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: var(--bg-panel);
}

.table-wrapper {
  width: 100%;
  display: block;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  min-width: 100%;
  overflow-x: auto;
  position: relative;
}

.connections-table {
  width: fit-content;
  min-width: 100%;
  border-collapse: collapse;
  font-size: 13px;
  border: none;
  border-radius: 0;
  table-layout: fixed;
  flex-shrink: 0;
  margin-bottom: 0;
  display: table;
  max-width: none;
  color: var(--text-1);
}

.connections-table thead tr {
  background-color: var(--bg-subtle);
  color: var(--text-2);
  text-align: left;
  font-weight: 600;
  font-size: 12px;
  border-bottom: 1px solid var(--border-strong);
  height: 24px;
  position: sticky;
  top: 0;
  z-index: 10;
  display: table-row;
}

.connections-table tbody {
  display: table-row-group;
}

.connections-table th,
.connections-table td {
  padding: 2px 3px;
  text-align: left;
  border-bottom: 1px solid var(--border);
  color: inherit;
  line-height: 1.2;
  height: 24px;
  vertical-align: middle;
  white-space: nowrap;
  overflow: hidden;
  word-break: keep-all;
  text-overflow: ellipsis;
}

/* 列分隔线（末列/填充列除外） */
.connections-table th:not(:last-child),
.connections-table td:not(:last-child) {
  border-right: 1px solid var(--border-strong);
}

.connections-table tbody tr {
  background-color: var(--bg-panel);
  transition: background-color 0.25s ease;
}

.connections-table tbody tr:nth-of-type(even) {
  background-color: var(--bg-subtle);
}

.connections-table tbody tr:hover {
  background-color: var(--bg-hover);
}

/* ===== 行状态：左侧色条 + 低饱和底色 ===== */
.connections-table tbody tr.new-connection {
  background-color: var(--success-weak);
}

.connections-table tbody tr.new-connection td:first-child {
  box-shadow: inset 3px 0 0 var(--success);
}

.connections-table tbody tr.changed-connection {
  background-color: var(--warning-weak);
}

.connections-table tbody tr.changed-connection td:first-child {
  box-shadow: inset 3px 0 0 var(--warning);
}

.connections-table tbody tr.deleted-connection {
  background-color: var(--danger-weak);
}

.connections-table tbody tr.deleted-connection td:first-child {
  box-shadow: inset 3px 0 0 var(--danger);
}

.connections-table tbody tr.deleted-connection td {
  color: var(--text-2);
  text-decoration: line-through;
}

.connections-table tbody tr.selected-row {
  background-color: var(--accent-weak);
}

.connections-table tbody tr.selected-row td:first-child {
  box-shadow: inset 3px 0 0 var(--accent);
}

.connections-table tbody tr.selected-row td {
  color: var(--text-1);
}

/* 调用链联动高亮：虚线描边圈出该进程的所有连接行（与选中行区分） */
.connections-table tbody tr.lineage-highlight {
  outline: 2px dashed var(--accent);
  outline-offset: -2px;
  background-color: var(--accent-weak);
}

.connections-table tbody tr.lineage-highlight td:first-child {
  box-shadow: inset 3px 0 0 var(--accent);
}

/* 冗余列样式 - 填充剩余空间 */
.filler-column,
.filler-cell {
  width: 100%;
  min-width: 0;
  max-width: none;
  border: none;
  border-right: none !important;
}

.column-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

/* 列宽拖拽手柄 */
.resizable-th {
  position: relative;
  cursor: default;
}

.resizable-th::after {
  content: "";
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: 10px;
  cursor: col-resize;
  background: transparent;
  z-index: 10;
  margin-right: -5px;
}

.resizable-th:hover::after {
  background: var(--border-strong);
  opacity: 0.7;
}

.connections-table.resizing {
  user-select: none;
}

.connections-table.resizing .resizable-th.current-resizing::after {
  background: var(--accent);
  opacity: 0.8;
}

.sortable-header {
  cursor: pointer;
  user-select: none;
  position: relative;
  padding-right: 18px;
}

.sort-indicator {
  position: absolute;
  right: 4px;
  font-size: 0.8em;
  color: var(--accent);
}

/* 首列与窗口边界保留间距（表头与数据行对齐） */
.connections-table th:first-child,
.connections-table td:first-child {
  padding-left: 14px;
}

/* 进程名称单元格 */
.process-name-cell {
  padding: 2px 8px;
}

.process-with-icon {
  display: flex;
  align-items: center;
  gap: 4px;
}

.process-icon {
  width: 16px;
  height: 16px;
  object-fit: contain;
  flex-shrink: 0;
}

/* 内核进程徽章 */
.kernel-process {
  font-weight: 600;
  color: var(--danger);
  background-color: var(--danger-weak);
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 12px;
}

/* ===== 空状态 ===== */
.table-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 80px 16px;
  text-align: center;
}

.empty-icon {
  color: var(--text-3);
  opacity: 0.55;
  margin-bottom: 8px;
}

.empty-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-2);
}

.empty-hint {
  margin: 0;
  font-size: 12.5px;
  color: var(--text-3);
}

/* ===== 骨架屏 ===== */
.table-skeleton {
  padding: 6px 0;
}

.skeleton-row {
  height: 24px;
  display: flex;
  align-items: center;
  padding: 0 8px;
}

.skeleton-bar {
  display: block;
  height: 12px;
  border-radius: 6px;
  background: linear-gradient(
    90deg,
    var(--bg-subtle) 25%,
    var(--bg-hover) 50%,
    var(--bg-subtle) 75%
  );
  background-size: 200% 100%;
  animation: pv-shimmer 1.4s ease-in-out infinite;
}

.skeleton-row:nth-child(2n) .skeleton-bar {
  animation-delay: 0.2s;
}

.skeleton-row:nth-child(3n) .skeleton-bar {
  animation-delay: 0.4s;
}
</style>
